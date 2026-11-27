// Release builds on Windows are GUI apps: no console window behind them.
#![cfg_attr(all(windows, not(debug_assertions)), windows_subsystem = "windows")]
//! Desktop shell: a winit window painted with vello (wgpu). All drawing
//! logic lives in `littledraw-core`; this file translates input events, manages
//! local workspace persistence, and turns display lists into GPU draw calls.

use std::error::Error;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use base64::Engine;
use littledraw_core::kurbo::{Affine, BezPath, Point, Rect, RoundedRect, Shape, Size, Vec2};
use littledraw_core::manager_ui::{ManagerAction, ManagerState, Selection};
use littledraw_core::{
    export, Button, Color, Cursor, Document, DrawOp, Editor, Frame, Item, Key, Modifiers, Request, Theme,
};
use serde_json::Value;
use vello::util::{RenderContext, RenderSurface};
use vello::wgpu;
use vello::{AaConfig, RenderParams, Renderer, Scene};
use winit::application::ApplicationHandler;
use winit::cursor::CursorIcon;
use winit::dpi::LogicalSize;
use winit::event::{ButtonSource, ElementState, Ime, MouseButton, MouseScrollDelta, PointerSource, WindowEvent};
use winit::event_loop::{ActiveEventLoop, ControlFlow, EventLoop};
use winit::icon::{Icon, RgbaIcon};
use winit::keyboard::{Key as WinitKey, NamedKey};
use winit::window::{
    ImeCapabilities, ImeEnableRequest, ImeRequest, ImeRequestData, Window, WindowAttributes, WindowId,
};

mod render;
mod store;
mod update;

use render::{encode, render_to_png_bytes, renderer_options, to_peniko, Images};
use store::WorkspaceStore;

const APP_NAME: &str = "LittleDraw";
/// Desktop entry id (`littledraw.desktop`), also used as the Wayland app id.
#[cfg(target_os = "linux")]
const APP_ID: &str = "littledraw";
/// Pixels per logical pixel of an exported PNG.
const EXPORT_SCALE: f64 = 2.0;
const IMAGE_EXTENSIONS: [&str; 6] = ["png", "jpg", "jpeg", "gif", "webp", "bmp"];
const DRAWING_EXTENSION: &str = "excalidraw";
const AUTOSAVE_DEBOUNCE: Duration = Duration::from_millis(700);
const THUMBNAIL_INTERVAL: Duration = Duration::from_secs(5);
const THUMBNAIL_MAX_SIDE: u32 = 640;
const BACK_BTN_RECT: Rect = Rect::new(14.0, 14.0, 128.0, 44.0);
const APP_ICON_PNG: &[u8] = include_bytes!("../../../resources/icon.png");

fn load_app_icon() -> Option<Icon> {
    let img = image::load_from_memory(APP_ICON_PNG).ok()?.into_rgba8();
    let (width, height) = img.dimensions();
    RgbaIcon::new(img.into_raw(), width, height).ok().map(Icon::from)
}

fn load_document(path: &Path) -> Result<Document, String> {
    let json = std::fs::read_to_string(path).map_err(|e| e.to_string())?;
    Document::from_json(&json).map_err(|e| e.to_string())
}

fn now_ms() -> f64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0.0, |d| d.as_millis() as f64)
}

fn now_ms_u64() -> u64 {
    SystemTime::now().duration_since(UNIX_EPOCH).map_or(0, |d| d.as_millis() as u64)
}

/// Logical pixels scrolled per mouse-wheel notch.
const WHEEL_LINE: f64 = 40.0;
/// Two presses this close in time and space make a double click.
const DOUBLE_CLICK_TIME: Duration = Duration::from_millis(400);
const DOUBLE_CLICK_DISTANCE: f64 = 4.0;

#[derive(Clone, Debug, PartialEq, Eq)]
enum Screen {
    Manager,
    Editor { board_id: Option<String> },
}

struct App {
    /// The editor's theme as last seen, to notice when the user switches it.
    theme: Theme,
    context: RenderContext,
    renderers: Vec<Option<Renderer>>,
    surface: Option<(RenderSurface<'static>, Arc<dyn Window>)>,
    scene: Scene,
    screen: Screen,
    manager: Option<ManagerState>,
    store: Option<WorkspaceStore>,
    editor: Editor,
    path: PathBuf,
    mods: Modifiers,
    /// Last cursor position, in logical pixels.
    cursor: Point,
    /// When and where the primary button was last pressed.
    last_press: Option<(Instant, Point)>,
    /// System clipboard, opened on first use.
    clipboard: Option<arboard::Clipboard>,
    images: Images,
    last_edit: Option<Instant>,
    last_thumb: Option<Instant>,
    needs_thumb: bool,
    /// Background self-update; `None` when not installed by Velopack.
    updates: Option<update::Updates>,
}

impl App {
    fn is_double_click(&self) -> bool {
        self.last_press.is_some_and(|(at, pos)| {
            at.elapsed() <= DOUBLE_CLICK_TIME && pos.distance(self.cursor) <= DOUBLE_CLICK_DISTANCE
        })
    }

    fn clipboard(&mut self) -> Option<&mut arboard::Clipboard> {
        if self.clipboard.is_none() {
            self.clipboard = arboard::Clipboard::new().map_err(|err| eprintln!("clipboard unavailable: {err}")).ok();
        }
        self.clipboard.as_mut()
    }

    fn copy(&mut self, cut: bool) {
        let copied = if cut { self.editor.cut() } else { self.editor.copy() };
        if let (Some(text), Some(clipboard)) = (copied, self.clipboard()) {
            if let Err(err) = clipboard.set_text(text) {
                eprintln!("could not copy: {err}");
            }
        }
    }

    fn paste(&mut self) {
        let Some(clipboard) = self.clipboard() else {
            return;
        };
        if let Ok(text) = clipboard.get_text() {
            return self.editor.paste(&text);
        }
        let Ok(bitmap) = clipboard.get_image() else {
            return;
        };
        let (width, height) = (bitmap.width as u32, bitmap.height as u32);
        let mut png = Vec::new();
        let encoded = image::RgbaImage::from_raw(width, height, bitmap.bytes.into_owned())
            .ok_or_else(|| "unexpected bitmap size".to_owned())
            .and_then(|pixels| {
                let format = image::ImageFormat::Png;
                pixels.write_to(&mut std::io::Cursor::new(&mut png), format).map_err(|e| e.to_string())
            });
        match encoded {
            Ok(()) => self.insert_image(&png),
            Err(err) => eprintln!("could not paste the image: {err}"),
        }
    }

    fn insert_image(&mut self, bytes: &[u8]) {
        let decoded = image::guess_format(bytes).and_then(|format| {
            let pixels = image::load_from_memory_with_format(bytes, format)?;
            Ok((format.to_mime_type(), pixels.width(), pixels.height()))
        });
        match decoded {
            Ok((mime, width, height)) => {
                let encoded = base64::engine::general_purpose::STANDARD.encode(bytes);
                let data_url = format!("data:{mime};base64,{encoded}");
                self.editor.insert_image(&data_url, mime, f64::from(width), f64::from(height), now_ms());
            }
            Err(err) => eprintln!("could not read the image: {err}"),
        }
    }

    fn pick_image(&mut self) {
        let picked = rfd::FileDialog::new().add_filter("Imágenes", &IMAGE_EXTENSIONS).pick_file();
        if let Some(path) = picked {
            match std::fs::read(&path) {
                Ok(bytes) => self.insert_image(&bytes),
                Err(err) => eprintln!("could not read {}: {err}", path.display()),
            }
        }
    }

    /// Asks where to save `doc` as an image and writes it there: SVG when
    /// the chosen name ends in `.svg`, PNG otherwise.
    fn export_image(&mut self, doc: &Document, name: &str) -> Result<(), String> {
        let picked = rfd::FileDialog::new()
            .add_filter("Imagen PNG", &["png"])
            .add_filter("Imagen SVG", &["svg"])
            .set_file_name(format!("{name}.png"))
            .save_file();
        let Some(mut dest) = picked else {
            return Ok(());
        };
        if dest.extension().is_none() {
            dest.set_extension("png");
        }
        if dest.extension().is_some_and(|ext| ext.eq_ignore_ascii_case("svg")) {
            return std::fs::write(&dest, export::to_svg(doc)).map_err(|e| e.to_string());
        }
        let Some((surface, _)) = &self.surface else {
            return Err("no hay ventana".into());
        };
        let dev_id = surface.dev_id;
        let device = &self.context.devices[dev_id].device;
        let queue = &self.context.devices[dev_id].queue;
        let Some(renderer) = self.renderers[dev_id].as_mut() else {
            return Err("no hay renderizador".into());
        };
        // Twice the size for a sharp picture, unless that exceeds the GPU.
        let (_, w, h) = export::to_frame(doc, 1.0);
        let limit = f64::from(device.limits().max_texture_dimension_2d);
        let scale = EXPORT_SCALE.min(limit / f64::from(w.max(h).max(1)));
        let (frame, w, h) = export::to_frame(doc, scale);
        let png = render_to_png_bytes(device, queue, renderer, &frame, w, h, &doc.files).map_err(|e| e.to_string())?;
        std::fs::write(&dest, png).map_err(|e| e.to_string())
    }

    /// Exports the drawing open in the editor, named after its board or file.
    fn export_open_drawing(&mut self) {
        let name = match (&self.screen, &self.store) {
            (Screen::Editor { board_id: Some(id) }, Some(store)) => {
                store.workspace().find_board(id).map(|e| e.board.name.clone())
            }
            _ => self.path.file_stem().map(|s| s.to_string_lossy().into_owned()),
        };
        let doc = self.editor.document().clone();
        if let Err(err) = self.export_image(&doc, name.as_deref().unwrap_or("dibujo")) {
            eprintln!("could not export the image: {err}");
        }
    }

    fn open_standalone(&mut self) {
        let picked = rfd::FileDialog::new().add_filter("Excalidraw", &[DRAWING_EXTENSION, "json"]).pick_file();
        let Some(path) = picked else {
            return;
        };
        match load_document(&path) {
            Ok(doc) if !self.editor.is_modified() || self.save_standalone() => {
                self.editor.load(doc);
                self.path = path;
                self.images = Images::default();
            }
            Ok(_) => {}
            Err(err) => eprintln!("could not open {}: {err}", path.display()),
        }
    }

    fn save_as_standalone(&mut self) {
        let name = self.path.file_name().unwrap_or_default().to_string_lossy().into_owned();
        let picked = rfd::FileDialog::new().add_filter("Excalidraw", &[DRAWING_EXTENSION]).set_file_name(name).save_file();
        if let Some(mut path) = picked {
            if path.extension().is_none() {
                path.set_extension(DRAWING_EXTENSION);
            }
            self.path = path;
            self.save_standalone();
        }
    }

    fn save_standalone(&mut self) -> bool {
        match std::fs::write(&self.path, self.editor.document().to_json()) {
            Ok(()) => self.editor.mark_saved(),
            Err(err) => eprintln!("could not save {}: {err}", self.path.display()),
        }
        !self.editor.is_modified()
    }

    fn preload_thumbnails(&mut self) {
        let Some(store) = &self.store else {
            return;
        };
        let ids: Vec<String> = store
            .workspace()
            .projects
            .iter()
            .flat_map(|p| p.boards.iter().map(|b| b.id.clone()))
            .collect();
        for id in ids {
            if let Ok(bytes) = std::fs::read(store.thumbnail_path(&id)) {
                self.images.insert_png(format!("thumb:{id}"), &bytes);
            }
        }
    }

    fn open_managed_board(&mut self, board_id: &str) {
        let Some(store) = &self.store else {
            return;
        };
        let doc = match store.load_board(board_id) {
            Ok(Some(json)) => match Document::from_json(&json) {
                Ok(d) => d,
                Err(err) => {
                    if let Some(mgr) = &mut self.manager {
                        mgr.notice = Some(format!("No se pudo leer la pizarra: {err}"));
                    }
                    return;
                }
            },
            Ok(None) => Document::default(),
            Err(err) => {
                if let Some(mgr) = &mut self.manager {
                    mgr.notice = Some(err);
                }
                return;
            }
        };
        self.editor.load(doc);
        if let Some((surface, window)) = &self.surface {
            let size = window.surface_size().to_logical::<f64>(window.scale_factor());
            let _ = surface;
            self.editor.resize(size.width, size.height);
        }
        self.editor.zoom_to_fit();
        self.screen = Screen::Editor { board_id: Some(board_id.to_owned()) };
        self.last_edit = None;
        self.last_thumb = None;
        self.needs_thumb = false;
        self.update_title();
    }

    fn save_managed_board(&mut self, force_thumbnail: bool) {
        let Screen::Editor { board_id: Some(board_id) } = &self.screen else {
            return;
        };
        let board_id = board_id.clone();

        if self.editor.is_modified() {
            let json = self.editor.document().to_json();
            if let Some(store) = &mut self.store {
                if let Err(err) = store.save_board(&board_id, &json) {
                    eprintln!("autosave failed: {err}");
                } else {
                    self.editor.mark_saved();
                    self.last_edit = None;
                }
            }
        }

        let should_thumb = force_thumbnail
            || (self.needs_thumb && self.last_thumb.is_none_or(|t| t.elapsed() >= THUMBNAIL_INTERVAL));
        if should_thumb {
            self.update_board_thumbnail(&board_id);
        }
    }

    fn update_board_thumbnail(&mut self, board_id: &str) {
        self.last_thumb = Some(Instant::now());
        self.needs_thumb = false;
        let doc = self.editor.document();
        let has_live = doc.elements.iter().any(|e| !e.is_deleted);
        if !has_live {
            if let Some(store) = &self.store {
                let _ = store.save_thumbnail(board_id, None);
            }
            self.images.remove(&format!("thumb:{board_id}"));
            return;
        }
        let Some((surface, _)) = &self.surface else {
            return;
        };
        let dev_id = surface.dev_id;
        let device = &self.context.devices[dev_id].device;
        let queue = &self.context.devices[dev_id].queue;
        let Some(renderer) = self.renderers[dev_id].as_mut() else {
            return;
        };
        let (frame, w, h) = export::to_frame(doc, 1.0);
        let max_dim = w.max(h);
        let (frame, w, h) = if max_dim > THUMBNAIL_MAX_SIDE {
            export::to_frame(doc, f64::from(THUMBNAIL_MAX_SIDE) / f64::from(max_dim))
        } else {
            (frame, w, h)
        };
        if let Ok(png) = render_to_png_bytes(device, queue, renderer, &frame, w, h, &doc.files) {
            if let Some(store) = &self.store {
                let _ = store.save_thumbnail(board_id, Some(&png));
            }
            self.images.insert_png(format!("thumb:{board_id}"), &png);
        }
    }

    fn back_to_manager(&mut self) {
        self.save_managed_board(true);
        if let (Some(mgr), Some(store)) = (&mut self.manager, &self.store) {
            mgr.now_ms = now_ms_u64();
            mgr.set_workspace(store.workspace().clone());
        }
        self.screen = Screen::Manager;
        self.update_title();
    }

    fn handle_manager_action(&mut self, action: ManagerAction) {
        match action {
            ManagerAction::OpenBoard(id) => {
                self.open_managed_board(&id);
            }
            ManagerAction::CreateProject(name) => {
                let res = self.store.as_mut().map(|s| s.create_project(&name));
                if let Some(Ok(proj)) = res {
                    if let (Some(mgr), Some(store)) = (&mut self.manager, &self.store) {
                        mgr.selection = Selection::Project(proj.id);
                        mgr.set_workspace(store.workspace().clone());
                    }
                } else if let Some(Err(e)) = res {
                    if let Some(mgr) = &mut self.manager {
                        mgr.notice = Some(e);
                    }
                }
            }
            ManagerAction::RenameProject { project_id, name } => {
                self.run_store_op(|s| s.rename_project(&project_id, &name));
            }
            ManagerAction::RecolorProject { project_id, color } => {
                self.run_store_op(|s| s.recolor_project(&project_id, &color));
            }
            ManagerAction::DeleteProject(project_id) => {
                self.run_store_op(|s| s.delete_project(&project_id));
            }
            ManagerAction::CreateBoard { project_id, name } => {
                let res = self.store.as_mut().map(|s| s.create_board(&project_id, &name));
                match res {
                    Some(Ok(board)) => {
                        if let (Some(mgr), Some(store)) = (&mut self.manager, &self.store) {
                            mgr.set_workspace(store.workspace().clone());
                        }
                        self.open_managed_board(&board.id);
                    }
                    Some(Err(e)) => {
                        if let Some(mgr) = &mut self.manager {
                            mgr.notice = Some(e);
                        }
                    }
                    None => {}
                }
            }
            ManagerAction::RenameBoard { board_id, name } => {
                self.run_store_op(|s| s.rename_board(&board_id, &name));
            }
            ManagerAction::DuplicateBoard(board_id) => {
                let res = self.store.as_mut().map(|s| s.duplicate_board(&board_id));
                match res {
                    Some(Ok(copy)) => {
                        if let Some(store) = &self.store {
                            if let Ok(bytes) = std::fs::read(store.thumbnail_path(&copy.id)) {
                                self.images.insert_png(format!("thumb:{}", copy.id), &bytes);
                            }
                        }
                        if let (Some(mgr), Some(store)) = (&mut self.manager, &self.store) {
                            mgr.set_workspace(store.workspace().clone());
                        }
                    }
                    Some(Err(e)) => {
                        if let Some(mgr) = &mut self.manager {
                            mgr.notice = Some(e);
                        }
                    }
                    None => {}
                }
            }
            ManagerAction::MoveBoard { board_id, target_project_id } => {
                self.run_store_op(|s| s.move_board(&board_id, &target_project_id));
            }
            ManagerAction::DeleteBoard(board_id) => {
                self.images.remove(&format!("thumb:{board_id}"));
                self.run_store_op(|s| s.delete_board(&board_id));
            }
            ManagerAction::ExportBoard(board_id) => {
                let Some(store) = &self.store else {
                    return;
                };
                let name = store
                    .workspace()
                    .find_board(&board_id)
                    .map(|e| format!("{}.excalidraw", e.board.name))
                    .unwrap_or_else(|| "pizarra.excalidraw".into());
                let picked = rfd::FileDialog::new()
                    .add_filter("Excalidraw", &[DRAWING_EXTENSION])
                    .set_file_name(name)
                    .save_file();
                if let Some(mut dest) = picked {
                    if dest.extension().is_none() {
                        dest.set_extension(DRAWING_EXTENSION);
                    }
                    let content = store
                        .load_board(&board_id)
                        .ok()
                        .flatten()
                        .unwrap_or_else(|| Document::default().to_json());
                    if let Err(err) = std::fs::write(&dest, content) {
                        if let Some(mgr) = &mut self.manager {
                            mgr.notice = Some(format!("Error al exportar: {err}"));
                        }
                    }
                }
            }
            ManagerAction::ExportBoardImage(board_id) => {
                let Some(store) = &self.store else {
                    return;
                };
                let name = store.workspace().find_board(&board_id).map(|e| e.board.name.clone());
                let loaded = store.load_board(&board_id).and_then(|json| match json {
                    Some(json) => Document::from_json(&json).map_err(|e| e.to_string()),
                    None => Ok(Document::default()),
                });
                let result = loaded.and_then(|doc| self.export_image(&doc, name.as_deref().unwrap_or("pizarra")));
                if let (Err(err), Some(mgr)) = (result, &mut self.manager) {
                    mgr.notice = Some(format!("Error al exportar la imagen: {err}"));
                }
            }
            ManagerAction::ImportBoards(project_id) => {
                let Some(paths) = rfd::FileDialog::new()
                    .add_filter("Excalidraw", &[DRAWING_EXTENSION, "json"])
                    .pick_files()
                else {
                    return;
                };
                self.run_store_op(|s| {
                    for p in &paths {
                        s.import_board_file(&project_id, p)?;
                    }
                    Ok(())
                });
            }
            ManagerAction::OpenWorkspaceFolder => {
                if let Some(store) = &self.store {
                    open_folder(store.root());
                }
            }
            ManagerAction::ToggleTheme(theme) => {
                // The editor follows; the end of the event loop remembers it.
                self.editor.set_theme(theme);
            }
        }
    }

    fn run_store_op<F>(&mut self, op: F)
    where
        F: FnOnce(&mut WorkspaceStore) -> Result<(), String>,
    {
        let Some(store) = &mut self.store else {
            return;
        };
        match op(store) {
            Ok(()) => {
                if let Some(mgr) = &mut self.manager {
                    mgr.now_ms = now_ms_u64();
                    mgr.set_workspace(store.workspace().clone());
                }
            }
            Err(err) => {
                if let Some(mgr) = &mut self.manager {
                    mgr.notice = Some(err);
                }
            }
        }
    }

    fn shortcut(&mut self, key: Key, shift: bool) -> bool {
        let managed = matches!(self.screen, Screen::Editor { board_id: Some(_) });
        match (key, shift) {
            (Key::Char('s'), false) => {
                if managed {
                    self.save_managed_board(true);
                } else {
                    self.save_standalone();
                }
            }
            (Key::Char('s'), true) if !managed => self.save_as_standalone(),
            (Key::Char('o'), false) if !managed => self.open_standalone(),
            (Key::Char('b'), false) if managed => self.back_to_manager(),
            (Key::Char('c'), false) => self.copy(false),
            (Key::Char('x'), false) => self.copy(true),
            (Key::Char('v'), false) => self.paste(),
            _ => return false,
        }
        true
    }

    fn update_title(&self) {
        let Some((_, window)) = &self.surface else {
            return;
        };
        match &self.screen {
            Screen::Manager => window.set_title(&format!("{APP_NAME} — Gestor de proyectos")),
            Screen::Editor { board_id: Some(id) } => {
                let name = self
                    .store
                    .as_ref()
                    .and_then(|s| s.workspace().find_board(id))
                    .map_or("Pizarra", |e| e.board.name.as_str());
                let mark = if self.editor.is_modified() { " •" } else { "" };
                window.set_title(&format!("{name}{mark} — {APP_NAME}"));
            }
            Screen::Editor { board_id: None } => {
                let name = self.path.file_name().unwrap_or_default().to_string_lossy();
                let mark = if self.editor.is_modified() { " •" } else { "" };
                window.set_title(&format!("{name}{mark} — {APP_NAME}"));
            }
        }
    }

    fn render(&mut self) {
        let Some((surface, window)) = &mut self.surface else {
            return;
        };
        let (width, height) = (surface.config.width, surface.config.height);
        let viewport = Rect::new(0.0, 0.0, f64::from(width), f64::from(height));
        let scale = window.scale_factor();

        let (frame, files_owned);
        let files: &Value = match &self.screen {
            Screen::Manager => {
                let mgr = self.manager.as_mut().expect("manager state exists");
                mgr.now_ms = now_ms_u64();
                frame = mgr.frame();
                files_owned = Value::Null;
                &files_owned
            }
            Screen::Editor { board_id } => {
                let mut f = self.editor.frame();
                if board_id.is_some() {
                    append_back_button(&mut f, self.cursor, self.editor.theme());
                }
                frame = f;
                &self.editor.document().files
            }
        };

        encode(&mut self.scene, &frame, scale, viewport, &mut self.images, files);

        let device = &self.context.devices[surface.dev_id];
        let renderer = self.renderers[surface.dev_id].as_mut().expect("renderer exists for the surface's device");
        let params = RenderParams {
            base_color: to_peniko(frame.background),
            width,
            height,
            antialiasing_method: AaConfig::Area,
        };
        if let Err(err) =
            renderer.render_to_texture(&device.device, &device.queue, &self.scene, &surface.target_view, &params)
        {
            eprintln!("render failed: {err}");
            return;
        }

        use wgpu::CurrentSurfaceTexture as Current;
        let target = match surface.surface.get_current_texture() {
            Current::Success(t) | Current::Suboptimal(t) => t,
            Current::Outdated | Current::Lost => {
                self.context.configure_surface(surface);
                window.request_redraw();
                return;
            }
            _ => return,
        };
        let mut encoder = device.device.create_command_encoder(&wgpu::CommandEncoderDescriptor::default());
        surface.blitter.copy(
            &device.device,
            &mut encoder,
            &surface.target_view,
            &target.texture.create_view(&wgpu::TextureViewDescriptor::default()),
        );
        device.queue.submit([encoder.finish()]);
        window.pre_present_notify();
        device.queue.present(target);
    }
}

fn append_back_button(frame: &mut Frame, cursor: Point, theme: Theme) {
    let hovered = BACK_BTN_RECT.contains(cursor);
    // (idle, hovered) backdrop, (idle, hovered) border, text.
    let (bg, border, text) = match theme {
        Theme::Light => (
            (Color::rgba(255, 255, 255, 235), Color::rgba(241, 240, 246, 245)),
            (Color::rgba(210, 208, 200, 255), Color::rgba(105, 101, 219, 255)),
            Color::rgba(30, 30, 36, 255),
        ),
        Theme::Dark => (
            (Color::rgba(35, 35, 41, 235), Color::rgba(49, 48, 59, 245)),
            (Color::rgba(62, 62, 74, 255), Color::rgba(168, 165, 255, 255)),
            Color::rgba(227, 227, 232, 255),
        ),
    };
    let bg = if hovered { bg.1 } else { bg.0 };
    let border = if hovered { border.1 } else { border.0 };
    let rr = RoundedRect::from_rect(BACK_BTN_RECT, 8.0).to_path(0.1);
    let mut ops = vec![
        DrawOp::fill(rr.clone(), bg),
        DrawOp::stroke(rr, border, 1.0),
    ];
    let mut text_path = BezPath::new();
    littledraw_core::text::append_line("<- Proyectos", 2, 13.0, Point::new(26.0, 33.5), &mut text_path);
    if !text_path.is_empty() {
        ops.push(DrawOp::fill(text_path, text));
    }
    frame.items.push(Item { transform: Affine::IDENTITY, opacity: 1.0, ops: Rc::new(ops) });
}

fn tablet_pressure(source: &PointerSource) -> Option<f64> {
    match source {
        PointerSource::TabletTool { data, .. } => data.force.map(|f| f.normalized(None)),
        PointerSource::Touch { force, .. } => force.map(|f| f.normalized(None)),
        _ => None,
    }
}

fn button_pressure(source: &ButtonSource) -> Option<f64> {
    match source {
        ButtonSource::TabletTool { data, .. } => data.force.map(|f| f.normalized(None)),
        ButtonSource::Touch { force, .. } => force.map(|f| f.normalized(None)),
        _ => None,
    }
}

fn map_button(button: MouseButton) -> Option<Button> {
    match button {
        MouseButton::Left => Some(Button::Primary),
        MouseButton::Middle => Some(Button::Middle),
        MouseButton::Right => Some(Button::Secondary),
        _ => None,
    }
}

fn map_key(key: &WinitKey) -> Option<Key> {
    Some(match key {
        WinitKey::Named(NamedKey::Delete) => Key::Delete,
        WinitKey::Named(NamedKey::Backspace) => Key::Backspace,
        WinitKey::Named(NamedKey::Enter) => Key::Enter,
        WinitKey::Named(NamedKey::Home) => Key::Home,
        WinitKey::Named(NamedKey::End) => Key::End,
        WinitKey::Named(NamedKey::Escape) => Key::Escape,
        WinitKey::Character(s) if s.as_str() == " " => Key::Space,
        WinitKey::Named(NamedKey::ArrowLeft) => Key::Left,
        WinitKey::Named(NamedKey::ArrowRight) => Key::Right,
        WinitKey::Named(NamedKey::ArrowUp) => Key::Up,
        WinitKey::Named(NamedKey::ArrowDown) => Key::Down,
        WinitKey::Character(s) => Key::Char(s.chars().next()?.to_ascii_lowercase()),
        _ => return None,
    })
}

impl ApplicationHandler for App {
    fn can_create_surfaces(&mut self, event_loop: &dyn ActiveEventLoop) {
        if self.surface.is_some() {
            return;
        }
        let attributes = WindowAttributes::default()
            .with_surface_size(LogicalSize::new(1320, 840))
            .with_window_icon(load_app_icon());
        // The Wayland app id must match `littledraw.desktop` so launchers
        // and taskbars group the window with its icon.
        #[cfg(target_os = "linux")]
        let attributes = attributes.with_platform_attributes(Box::new(
            winit::platform::wayland::WindowAttributesWayland::default().with_name(APP_ID, APP_ID),
        ));
        let window: Arc<dyn Window> =
            Arc::from(event_loop.create_window(attributes).expect("failed to create window"));
        let size = window.surface_size();
        let surface = pollster::block_on(self.context.create_surface(
            window.clone(),
            size.width.max(1),
            size.height.max(1),
            wgpu::PresentMode::AutoVsync,
        ))
        .expect("failed to create a GPU surface");

        self.renderers.resize_with(self.context.devices.len(), || None);
        self.renderers[surface.dev_id].get_or_insert_with(|| {
            Renderer::new(&self.context.devices[surface.dev_id].device, renderer_options())
                .expect("failed to create the vello renderer")
        });

        let logical = size.to_logical::<f64>(window.scale_factor());
        self.editor.resize(logical.width, logical.height);
        self.editor.zoom_to_fit();
        if let Some(mgr) = &mut self.manager {
            mgr.resize(Size::new(logical.width, logical.height));
        }
        if let Some(req) = ImeEnableRequest::new(ImeCapabilities::default(), ImeRequestData::default()) {
            let _ = window.request_ime_update(ImeRequest::Enable(req));
        }
        window.request_redraw();
        self.surface = Some((surface, window));
        self.update_title();
    }

    /// An update finished downloading in the background.
    fn proxy_wake_up(&mut self, _event_loop: &dyn ActiveEventLoop) {
        let Some(version) = self.updates.as_mut().and_then(update::Updates::poll) else {
            return;
        };
        if let Some(mgr) = &mut self.manager {
            mgr.notice = Some(format!("LittleDraw {version} se instalará al cerrar la aplicación."));
        }
        if let Some((_, window)) = &self.surface {
            window.request_redraw();
        }
    }

    fn about_to_wait(&mut self, event_loop: &dyn ActiveEventLoop) {
        if matches!(self.screen, Screen::Editor { board_id: Some(_) }) {
            let due_save = self.last_edit.is_some_and(|t| t.elapsed() >= AUTOSAVE_DEBOUNCE);
            let due_thumb =
                self.needs_thumb && self.last_thumb.is_none_or(|t| t.elapsed() >= THUMBNAIL_INTERVAL);
            if due_save || due_thumb {
                self.save_managed_board(false);
                self.update_title();
            }
            if self.last_edit.is_some() || self.needs_thumb {
                event_loop.set_control_flow(ControlFlow::WaitUntil(Instant::now() + Duration::from_millis(250)));
                return;
            }
        }
        event_loop.set_control_flow(ControlFlow::Wait);
    }

    fn window_event(&mut self, event_loop: &dyn ActiveEventLoop, _id: WindowId, event: WindowEvent) {
        let Some((surface, window)) = &mut self.surface else {
            return;
        };
        let scale = window.scale_factor();
        match event {
            WindowEvent::CloseRequested => {
                self.save_managed_board(true);
                if let Some(updates) = &self.updates {
                    updates.install_on_exit();
                }
                event_loop.exit();
                return;
            }
            WindowEvent::RedrawRequested => {
                self.render();
                return;
            }
            WindowEvent::SurfaceResized(size) => {
                if size.width > 0 && size.height > 0 {
                    self.context.resize_surface(surface, size.width, size.height);
                    let logical = size.to_logical::<f64>(scale);
                    self.editor.resize(logical.width, logical.height);
                    if let Some(mgr) = &mut self.manager {
                        mgr.resize(Size::new(logical.width, logical.height));
                    }
                }
            }
            WindowEvent::ModifiersChanged(mods) => {
                let state = mods.state();
                self.mods =
                    Modifiers { shift: state.shift_key(), ctrl: state.control_key(), alt: state.alt_key() };
            }
            WindowEvent::PointerMoved { position, source, .. } => {
                let p = position.to_logical::<f64>(scale);
                self.cursor = Point::new(p.x, p.y);
                match self.screen {
                    Screen::Manager => {
                        if let Some(mgr) = &mut self.manager {
                            mgr.pointer_move(self.cursor);
                        }
                    }
                    Screen::Editor { .. } => {
                        self.editor.pointer_move_with_pressure(self.cursor, self.mods, tablet_pressure(&source));
                    }
                }
            }
            WindowEvent::PointerButton { state, position, button, .. } => {
                let p = position.to_logical::<f64>(scale);
                self.cursor = Point::new(p.x, p.y);
                let pressure = button_pressure(&button);
                let Some(button) = button.mouse_button().and_then(map_button) else {
                    return;
                };
                match &self.screen {
                    Screen::Manager => {
                        if state == ElementState::Pressed && button == Button::Primary {
                            let action = self.manager.as_mut().and_then(|m| m.click(self.cursor));
                            if let Some(act) = action {
                                self.handle_manager_action(act);
                            }
                        }
                    }
                    Screen::Editor { board_id } => {
                        if board_id.is_some()
                            && state == ElementState::Pressed
                            && button == Button::Primary
                            && BACK_BTN_RECT.contains(self.cursor)
                        {
                            self.back_to_manager();
                            if let Some((_, window)) = &self.surface {
                                window.request_redraw();
                            }
                            return;
                        }
                        if state == ElementState::Released {
                            self.editor.pointer_up(self.cursor, button, self.mods);
                        } else if button == Button::Primary && self.is_double_click() {
                            self.last_press = None;
                            self.editor.double_click(self.cursor);
                        } else {
                            if button == Button::Primary {
                                self.last_press = Some((Instant::now(), self.cursor));
                            }
                            self.editor.pointer_down_with_pressure(self.cursor, button, self.mods, pressure);
                        }
                    }
                }
            }
            WindowEvent::MouseWheel { delta, .. } => {
                let (dx, dy) = match delta {
                    MouseScrollDelta::LineDelta(x, y) => (f64::from(x) * WHEEL_LINE, f64::from(y) * WHEEL_LINE),
                    MouseScrollDelta::PixelDelta(pos) => {
                        let p = pos.to_logical::<f64>(scale);
                        (p.x, p.y)
                    }
                    _ => (0.0, 0.0),
                };
                match self.screen {
                    Screen::Manager => {
                        if let Some(mgr) = &mut self.manager {
                            mgr.scroll(dy);
                        }
                    }
                    Screen::Editor { .. } => {
                        self.editor.wheel(Vec2::new(dx, dy), self.cursor, self.mods);
                    }
                }
            }
            WindowEvent::PinchGesture { delta, .. } => {
                if matches!(self.screen, Screen::Editor { .. }) {
                    self.editor.zoom_at(self.cursor, 1.0 + delta);
                }
            }
            WindowEvent::KeyboardInput { event, .. } => {
                let key = map_key(&event.logical_key);
                let pressed = event.state == ElementState::Pressed;
                match self.screen {
                    Screen::Manager => {
                        if pressed {
                            if let Some(k) = key {
                                let action = self.manager.as_mut().and_then(|m| m.key_down(k));
                                if let Some(act) = action {
                                    self.handle_manager_action(act);
                                }
                            }
                            if let Some(text) = event.text.filter(|_| !self.mods.ctrl && !self.mods.alt) {
                                if let Some(mgr) = &mut self.manager {
                                    mgr.text_input(&text);
                                }
                            }
                        }
                    }
                    Screen::Editor { .. } => {
                        let shortcut = pressed && self.mods.ctrl;
                        if shortcut && key.is_some_and(|key| self.shortcut(key, self.mods.shift)) {
                            // Handled by application.
                        } else if let Some(key) = key {
                            if pressed {
                                self.editor.key_down(key, self.mods);
                            } else {
                                self.editor.key_up(key);
                            }
                        }
                        if let Some(text) = event.text.filter(|_| pressed && !self.mods.ctrl && !self.mods.alt) {
                            self.editor.text_input(&text);
                        }
                    }
                }
            }
            WindowEvent::Ime(Ime::Commit(text)) => match self.screen {
                Screen::Manager => {
                    if let Some(mgr) = &mut self.manager {
                        mgr.text_input(&text);
                    }
                }
                Screen::Editor { .. } => self.editor.text_input(&text),
            },
            _ => return,
        }

        if matches!(self.screen, Screen::Editor { .. }) {
            match self.editor.take_request() {
                Some(Request::PickImage) => self.pick_image(),
                Some(Request::ExportImage) => self.export_open_drawing(),
                None => {}
            }
            if self.editor.is_modified() {
                self.last_edit = Some(Instant::now());
                self.needs_thumb = true;
            }
        }
        if self.editor.theme() != self.theme {
            self.theme = self.editor.theme();
            if let Some(manager) = &mut self.manager {
                manager.theme = self.theme;
            }
            remember_theme(self.theme);
        }
        self.update_title();
        if let Some((_, window)) = &self.surface {
            let icon = match self.screen {
                Screen::Manager => CursorIcon::Default,
                Screen::Editor { .. } => match self.editor.cursor() {
                    Cursor::Default => CursorIcon::Default,
                    Cursor::Crosshair => CursorIcon::Crosshair,
                    Cursor::Text => CursorIcon::Text,
                    Cursor::Grab => CursorIcon::Grab,
                },
            };
            window.set_cursor(icon.into());
            window.request_redraw();
        }
    }
}

fn untitled_path() -> PathBuf {
    (0u32..)
        .map(|n| match n {
            0 => PathBuf::from("untitled.excalidraw"),
            n => PathBuf::from(format!("untitled-{n}.excalidraw")),
        })
        .find(|path| !path.exists())
        .expect("some untitled name is free")
}

/// Shows `dir` in the system's file manager.
fn open_folder(dir: &Path) {
    let opener = if cfg!(windows) {
        "explorer"
    } else if cfg!(target_os = "macos") {
        "open"
    } else {
        "xdg-open"
    };
    if let Err(err) = std::process::Command::new(opener).arg(dir).spawn() {
        eprintln!("could not open {}: {err}", dir.display());
    }
}

/// Where the chosen theme is remembered between runs, inside
/// [`store::config_dir`].
fn theme_file() -> Option<PathBuf> {
    Some(store::config_dir()?.join("theme"))
}

fn remembered_theme() -> Option<Theme> {
    // Earlier versions kept the choice in `~/.config/board/theme`.
    let legacy = || {
        let config = store::config_dir()?;
        std::fs::read_to_string(config.parent()?.join("board").join("theme")).ok()
    };
    let saved = std::fs::read_to_string(theme_file()?).ok().or_else(legacy)?;
    match saved.trim() {
        "dark" => Some(Theme::Dark),
        "light" => Some(Theme::Light),
        _ => None,
    }
}

fn remember_theme(theme: Theme) {
    let name = match theme {
        Theme::Light => "light",
        Theme::Dark => "dark",
    };
    let saved = theme_file().ok_or_else(|| "no home directory".to_owned()).and_then(|file| {
        let dir = file.parent().expect("the theme file is inside a directory");
        std::fs::create_dir_all(dir).and_then(|()| std::fs::write(&file, name)).map_err(|e| e.to_string())
    });
    if let Err(err) = saved {
        eprintln!("could not remember the theme: {err}");
    }
}

struct Args {
    file: Option<PathBuf>,
    workspace: Option<PathBuf>,
    svg: Option<PathBuf>,
    png: Option<PathBuf>,
    scale: f64,
    /// Theme forced for this run, overriding the remembered one.
    theme: Option<Theme>,
}

fn parse_args() -> Result<Args, String> {
    let mut args = Args { file: None, workspace: None, svg: None, png: None, scale: 1.0, theme: None };
    let mut it = std::env::args().skip(1);
    while let Some(arg) = it.next() {
        match arg.as_str() {
            "--workspace" => args.workspace = Some(it.next().ok_or("--workspace needs a directory path")?.into()),
            "--svg" => args.svg = Some(it.next().ok_or("--svg needs an output path")?.into()),
            "--png" => args.png = Some(it.next().ok_or("--png needs an output path")?.into()),
            "--scale" => {
                let value = it.next().ok_or("--scale needs a number")?;
                args.scale = value.parse().ok().filter(|s| *s > 0.0).ok_or("--scale must be a positive number")?;
            }
            "--dark" => args.theme = Some(Theme::Dark),
            "--light" => args.theme = Some(Theme::Light),
            "-h" | "--help" => return Err(String::new()),
            flag if flag.starts_with('-') => return Err(format!("unknown option {flag}")),
            _ if args.file.is_none() => args.file = Some(arg.into()),
            _ => return Err("only one file can be opened".to_owned()),
        }
    }
    Ok(args)
}

fn main() {
    update::run_hooks();
    let args = parse_args().unwrap_or_else(|err| {
        if !err.is_empty() {
            eprintln!("error: {err}");
        }
        eprintln!(
            "usage: littledraw [FILE.excalidraw] [--workspace DIR] [--dark | --light] [--svg OUT.svg] [--png OUT.png [--scale N]]"
        );
        std::process::exit(2);
    });

    let mut document = None;
    if let Some(path) = args.file.as_ref().filter(|p| p.exists()) {
        match load_document(path) {
            Ok(doc) => document = Some(doc),
            Err(err) => {
                eprintln!("could not open {}: {err}", path.display());
                std::process::exit(1);
            }
        }
    }

    if args.svg.is_some() || args.png.is_some() {
        let Some(doc) = document else {
            eprintln!("error: exporting needs an existing input file");
            std::process::exit(2);
        };
        let mut failed = false;
        let mut report = |out: &Path, result: Result<(), Box<dyn Error>>| {
            if let Err(err) = result {
                eprintln!("could not export {}: {err}", out.display());
                failed = true;
            }
        };
        if let Some(out) = &args.svg {
            report(out, std::fs::write(out, export::to_svg(&doc)).map_err(Into::into));
        }
        if let Some(out) = &args.png {
            let (frame, width, height) = export::to_frame(&doc, args.scale);
            report(out, render::write_png(&frame, width, height, &doc.files, out));
        }
        std::process::exit(i32::from(failed));
    }

    let seed = SystemTime::now().duration_since(UNIX_EPOCH).map_or(1, |d| d.as_nanos() as u64);
    let mut editor = Editor::new(seed);
    // One theme for the manager and the editor.
    let theme = args.theme.or_else(remembered_theme).unwrap_or_default();
    editor.set_theme(theme);
    let (screen, store, manager, path) = if let Some(file) = args.file {
        if let Some(doc) = document {
            editor.load(doc);
        }
        (Screen::Editor { board_id: None }, None, None, file)
    } else {
        let root = args.workspace.unwrap_or_else(WorkspaceStore::default_root);
        let store = WorkspaceStore::open(root)
            .unwrap_or_else(|err| panic!("failed to open workspace: {err}"));
        let mgr = ManagerState::new(
            store.workspace().clone(),
            Size::new(1320.0, 840.0),
            theme,
            now_ms_u64(),
        );
        (Screen::Manager, Some(store), Some(mgr), untitled_path())
    };

    let mut app = App {
        theme,
        context: RenderContext::new(),
        renderers: Vec::new(),
        surface: None,
        scene: Scene::new(),
        screen,
        manager,
        store,
        editor,
        path,
        mods: Modifiers::default(),
        cursor: Point::ZERO,
        last_press: None,
        clipboard: None,
        images: Images::default(),
        last_edit: None,
        last_thumb: None,
        needs_thumb: false,
        updates: None,
    };
    app.preload_thumbnails();

    let event_loop = EventLoop::new().expect("failed to create event loop");
    app.updates = update::Updates::start(event_loop.create_proxy());
    event_loop.run_app(app).expect("event loop failed");
}
