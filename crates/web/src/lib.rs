//! LittleDraw in the browser: the same editor as the desktop app, compiled
//! to WebAssembly and painted on a `<canvas>` with the Canvas 2D API, so it
//! runs in any current browser without WebGPU.
//!
//! The page's script (`site/demo/main.js`) turns DOM events into calls on
//! [`Demo`] and does what only the page can: files, clipboard, storage.

use std::collections::{HashMap, HashSet};
use std::rc::Rc;

use littledraw_core::kurbo::{Affine, Point, Rect, Shape, Vec2};
use littledraw_core::{export, Button, Color, Cursor, Document, DrawOp, Editor, Frame, Key, Modifiers, Request, Style, Theme};
use serde_json::Value;
use wasm_bindgen::prelude::*;
use wasm_bindgen::JsCast;
use web_sys::{CanvasRenderingContext2d as Context, CanvasWindingRule, HtmlCanvasElement, HtmlImageElement, Path2d};

/// The scene shown on a first visit.
const WELCOME: &str = include_str!("../../../samples/matematicas.excalidraw");
/// Largest side of an exported PNG, in pixels; browsers cap canvas sizes.
const EXPORT_MAX_SIDE: f64 = 8192.0;

#[wasm_bindgen]
pub struct Demo {
    editor: Editor,
    canvas: HtmlCanvasElement,
    context: Context,
    /// Off-screen canvas for elements drawn with group opacity.
    layer: Option<(HtmlCanvasElement, Context)>,
    dpr: f64,
    paths: PathCache,
    images: Images,
}

#[wasm_bindgen]
impl Demo {
    /// Opens `saved` (a `.excalidraw` JSON string), or the welcome scene.
    #[wasm_bindgen(constructor)]
    pub fn new(canvas: HtmlCanvasElement, saved: Option<String>, dark: bool) -> Result<Demo, JsValue> {
        let context = context_2d(&canvas)?;
        let seed = (js_sys::Math::random() * 9_007_199_254_740_991.0) as u64;
        let mut editor = Editor::new(seed);
        let doc = saved
            .as_deref()
            .and_then(|json| Document::from_json(json).ok())
            .or_else(|| Document::from_json(WELCOME).ok());
        if let Some(doc) = doc {
            editor.load(doc);
        }
        editor.set_theme(if dark { Theme::Dark } else { Theme::Light });
        editor.mark_saved();
        Ok(Demo {
            editor,
            canvas,
            context,
            layer: None,
            dpr: 1.0,
            paths: PathCache::default(),
            images: Images::default(),
        })
    }

    /// The canvas now measures `width`×`height` CSS pixels on a screen with
    /// `dpr` device pixels per CSS pixel.
    pub fn resize(&mut self, width: f64, height: f64, dpr: f64) {
        self.dpr = dpr;
        self.canvas.set_width((width * dpr).round() as u32);
        self.canvas.set_height((height * dpr).round() as u32);
        self.editor.resize(width, height);
    }

    pub fn zoom_to_fit(&mut self) {
        self.editor.zoom_to_fit();
    }

    /// `button`: 0 primary, 1 middle, 2 secondary (as in `PointerEvent`).
    /// `pressure` is given only for pens.
    #[allow(clippy::too_many_arguments)]
    pub fn pointer_down(&mut self, x: f64, y: f64, button: u8, shift: bool, ctrl: bool, alt: bool, pressure: Option<f64>) {
        if let Some(button) = map_button(button) {
            let mods = Modifiers { shift, ctrl, alt };
            self.editor.pointer_down_with_pressure(Point::new(x, y), button, mods, pressure);
        }
    }

    pub fn pointer_move(&mut self, x: f64, y: f64, shift: bool, ctrl: bool, alt: bool, pressure: Option<f64>) {
        let mods = Modifiers { shift, ctrl, alt };
        self.editor.pointer_move_with_pressure(Point::new(x, y), mods, pressure);
    }

    pub fn pointer_up(&mut self, x: f64, y: f64, button: u8, shift: bool, ctrl: bool, alt: bool) {
        if let Some(button) = map_button(button) {
            self.editor.pointer_up(Point::new(x, y), button, Modifiers { shift, ctrl, alt });
        }
    }

    pub fn double_click(&mut self, x: f64, y: f64) {
        self.editor.double_click(Point::new(x, y));
    }

    /// `dx`/`dy` in CSS pixels, positive when scrolling up or left.
    #[allow(clippy::too_many_arguments)]
    pub fn wheel(&mut self, dx: f64, dy: f64, x: f64, y: f64, shift: bool, ctrl: bool, alt: bool) {
        self.editor.wheel(Vec2::new(dx, dy), Point::new(x, y), Modifiers { shift, ctrl, alt });
    }

    /// `key` is `KeyboardEvent.key`. Returns whether the editor knows it.
    pub fn key_down(&mut self, key: &str, shift: bool, ctrl: bool, alt: bool) -> bool {
        let Some(key) = map_key(key) else {
            return false;
        };
        self.editor.key_down(key, Modifiers { shift, ctrl, alt });
        true
    }

    pub fn key_up(&mut self, key: &str) {
        if let Some(key) = map_key(key) {
            self.editor.key_up(key);
        }
    }

    /// Typed or composed text; ignored unless a text is being edited.
    pub fn text_input(&mut self, text: &str) {
        self.editor.text_input(text);
    }

    pub fn is_editing_text(&self) -> bool {
        self.editor.is_editing_text()
    }

    pub fn copy(&self) -> Option<String> {
        self.editor.copy()
    }

    pub fn cut(&mut self) -> Option<String> {
        self.editor.cut()
    }

    pub fn paste(&mut self, text: &str) {
        self.editor.paste(text);
    }

    /// `"image"` when the user wants to insert an image, `"export"` when they
    /// want to export the drawing.
    pub fn take_request(&mut self) -> Option<String> {
        self.editor.take_request().map(|request| {
            match request {
                Request::PickImage => "image",
                Request::ExportImage => "export",
            }
            .to_owned()
        })
    }

    pub fn insert_image(&mut self, data_url: &str, mime: &str, width: f64, height: f64) {
        self.editor.insert_image(data_url, mime, width, height, js_sys::Date::now());
    }

    /// The CSS cursor for the canvas.
    pub fn cursor(&self) -> String {
        match self.editor.cursor() {
            Cursor::Default => "default",
            Cursor::Crosshair => "crosshair",
            Cursor::Text => "text",
            Cursor::Grab => "grab",
        }
        .to_owned()
    }

    pub fn is_dark(&self) -> bool {
        self.editor.theme() == Theme::Dark
    }

    pub fn is_modified(&self) -> bool {
        self.editor.is_modified()
    }

    pub fn mark_saved(&mut self) {
        self.editor.mark_saved();
    }

    pub fn to_json(&self) -> String {
        self.editor.document().to_json()
    }

    /// Replaces the drawing with a `.excalidraw` JSON string.
    pub fn load_json(&mut self, json: &str) -> Result<(), JsValue> {
        let doc = Document::from_json(json).map_err(|err| JsValue::from_str(&err.to_string()))?;
        self.editor.load(doc);
        self.editor.mark_saved();
        self.editor.zoom_to_fit();
        Ok(())
    }

    /// Starts an empty drawing.
    pub fn clear(&mut self) {
        self.editor.load(Document::default());
        self.editor.mark_saved();
    }

    pub fn to_svg(&self) -> String {
        export::to_svg(self.editor.document())
    }

    /// Paints the editor. Returns `true` while images are still loading,
    /// so the page should paint again on the next frame.
    pub fn render(&mut self) -> Result<bool, JsValue> {
        let frame = self.editor.frame();
        let size = (self.canvas.width(), self.canvas.height());
        let files = &self.editor.document().files;
        let pending = paint(&self.context, &mut self.layer, &frame, self.dpr, size, &mut self.paths, &mut self.images, files)?;
        Ok(pending)
    }

    /// Paints the whole drawing, without the editor's interface, at twice
    /// its size (less for very large drawings) onto `canvas`, for a PNG.
    /// Returns `true` while images are still loading.
    pub fn render_export(&mut self, canvas: &HtmlCanvasElement) -> Result<bool, JsValue> {
        let doc = self.editor.document();
        let (_, w, h) = export::to_frame(doc, 1.0);
        let scale = 2.0_f64.min(EXPORT_MAX_SIDE / f64::from(w.max(h).max(1)));
        let (frame, w, h) = export::to_frame(doc, scale);
        canvas.set_width(w);
        canvas.set_height(h);
        let context = context_2d(canvas)?;
        let mut layer = None;
        paint(&context, &mut layer, &frame, 1.0, (w, h), &mut PathCache::default(), &mut self.images, &doc.files)
    }
}

fn context_2d(canvas: &HtmlCanvasElement) -> Result<Context, JsValue> {
    canvas
        .get_context("2d")?
        .ok_or_else(|| JsValue::from_str("this browser has no 2D canvas"))?
        .dyn_into::<Context>()
        .map_err(|_| JsValue::from_str("unexpected canvas context"))
}

fn map_button(button: u8) -> Option<Button> {
    match button {
        0 => Some(Button::Primary),
        1 => Some(Button::Middle),
        2 => Some(Button::Secondary),
        _ => None,
    }
}

/// `KeyboardEvent.key` → the editor's key, as the desktop app maps them.
fn map_key(key: &str) -> Option<Key> {
    Some(match key {
        "Delete" => Key::Delete,
        "Backspace" => Key::Backspace,
        "Enter" => Key::Enter,
        "Home" => Key::Home,
        "End" => Key::End,
        "Escape" => Key::Escape,
        " " => Key::Space,
        "ArrowLeft" => Key::Left,
        "ArrowRight" => Key::Right,
        "ArrowUp" => Key::Up,
        "ArrowDown" => Key::Down,
        _ => {
            let mut chars = key.chars();
            let c = chars.next()?;
            if chars.next().is_some() {
                return None;
            }
            Key::Char(c.to_ascii_lowercase())
        }
    })
}

fn css(color: Color) -> String {
    format!("rgba({},{},{},{})", color.r, color.g, color.b, f64::from(color.a) / 255.0)
}

fn set_transform(context: &Context, t: Affine) -> Result<(), JsValue> {
    let [a, b, c, d, e, f] = t.as_coeffs();
    context.set_transform(a, b, c, d, e, f)
}

/// `Path2D` objects for the ops of each item, kept while the core keeps
/// reusing the same `Rc` (its per-element geometry cache), so unchanged
/// elements are not converted again every frame.
#[derive(Default)]
struct PathCache {
    entries: HashMap<usize, (Rc<Vec<DrawOp>>, Vec<CachedOp>)>,
}

struct CachedOp {
    path: Path2d,
    clip: Option<Path2d>,
}

impl PathCache {
    fn get(&mut self, ops: &Rc<Vec<DrawOp>>) -> Result<&[CachedOp], JsValue> {
        let key = Rc::as_ptr(ops) as usize;
        if !self.entries.contains_key(&key) {
            let cached = ops
                .iter()
                .map(|op| {
                    let clip = match &op.style {
                        Style::ClippedStroke { clip, .. } => Some(Path2d::new_with_path_string(&clip.to_svg())?),
                        _ => None,
                    };
                    Ok(CachedOp { path: Path2d::new_with_path_string(&op.path.to_svg())?, clip })
                })
                .collect::<Result<Vec<_>, JsValue>>()?;
            // Holding the Rc keeps its address from being reused by another list.
            self.entries.insert(key, (Rc::clone(ops), cached));
        }
        Ok(&self.entries[&key].1)
    }

    fn retain(&mut self, used: &HashSet<usize>) {
        self.entries.retain(|key, _| used.contains(key));
    }
}

/// `<img>` elements decoding the document's images, by file id.
#[derive(Default)]
struct Images {
    elements: HashMap<String, HtmlImageElement>,
}

enum ImageState<'a> {
    Ready(&'a HtmlImageElement),
    Loading,
    Missing,
}

impl Images {
    fn get(&mut self, file_id: &str, files: &Value) -> ImageState<'_> {
        if !self.elements.contains_key(file_id) {
            let Some(url) = files[file_id]["dataURL"].as_str() else {
                return ImageState::Missing;
            };
            let Ok(img) = HtmlImageElement::new() else {
                return ImageState::Missing;
            };
            img.set_src(url);
            self.elements.insert(file_id.to_owned(), img);
        }
        let img = &self.elements[file_id];
        if !img.complete() {
            ImageState::Loading
        } else if img.natural_width() == 0 {
            ImageState::Missing
        } else {
            ImageState::Ready(img)
        }
    }
}

/// Paints `frame` (in logical pixels) scaled by `scale` onto a canvas of
/// `size` device pixels. Returns `true` if some image is still loading.
#[allow(clippy::too_many_arguments)]
fn paint(
    context: &Context,
    layer: &mut Option<(HtmlCanvasElement, Context)>,
    frame: &Frame,
    scale: f64,
    size: (u32, u32),
    paths: &mut PathCache,
    images: &mut Images,
    files: &Value,
) -> Result<bool, JsValue> {
    let (w, h) = (f64::from(size.0), f64::from(size.1));
    context.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)?;
    context.set_global_alpha(1.0);
    context.clear_rect(0.0, 0.0, w, h);
    if !frame.background.is_transparent() {
        context.set_fill_style_str(&css(frame.background));
        context.fill_rect(0.0, 0.0, w, h);
    }
    context.set_line_cap("round");
    context.set_line_join("round");

    let mut pending = false;
    let mut used = HashSet::new();
    for item in &frame.items {
        used.insert(Rc::as_ptr(&item.ops) as usize);
        let cached = paths.get(&item.ops)?;
        // Group opacity: overlapping strokes of one element must not add
        // up, so a translucent element is drawn apart and blended at once.
        let translucent = item.opacity < 1.0;
        let target = if translucent { layer_context(layer, size)? } else { context };
        let transform = Affine::scale(scale) * item.transform;
        set_transform(target, transform)?;
        for (op, cached) in item.ops.iter().zip(cached) {
            pending |= paint_op(target, op, cached, transform, images, files)?;
        }
        if translucent {
            if let Some((canvas, _)) = layer {
                context.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)?;
                context.set_global_alpha(f64::from(item.opacity));
                context.draw_image_with_html_canvas_element(canvas, 0.0, 0.0)?;
                context.set_global_alpha(1.0);
            }
        }
    }
    paths.retain(&used);
    Ok(pending)
}

/// The off-screen layer, sized like the target and cleared.
fn layer_context(layer: &mut Option<(HtmlCanvasElement, Context)>, size: (u32, u32)) -> Result<&Context, JsValue> {
    if layer.is_none() {
        let document = web_sys::window().and_then(|w| w.document()).ok_or("no document")?;
        let canvas: HtmlCanvasElement = document.create_element("canvas")?.dyn_into().map_err(|_| "not a canvas")?;
        let context = context_2d(&canvas)?;
        *layer = Some((canvas, context));
    }
    let (canvas, context) = layer.as_ref().expect("just created");
    if canvas.width() != size.0 || canvas.height() != size.1 {
        canvas.set_width(size.0);
        canvas.set_height(size.1);
    }
    context.set_transform(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)?;
    context.clear_rect(0.0, 0.0, f64::from(size.0), f64::from(size.1));
    context.set_line_cap("round");
    context.set_line_join("round");
    Ok(context)
}

/// Paints one op; the context already has the item's `transform`.
fn paint_op(
    context: &Context,
    op: &DrawOp,
    cached: &CachedOp,
    transform: Affine,
    images: &mut Images,
    files: &Value,
) -> Result<bool, JsValue> {
    let color = css(op.color);
    match &op.style {
        Style::Fill => {
            context.set_fill_style_str(&color);
            context.fill_with_path_2d_and_winding(&cached.path, CanvasWindingRule::Nonzero);
        }
        Style::Stroke { width, dash } => {
            context.set_stroke_style_str(&color);
            context.set_line_width(*width);
            let pattern: js_sys::Array = dash.iter().map(|&d| JsValue::from_f64(d)).collect();
            context.set_line_dash(&pattern)?;
            context.stroke_with_path(&cached.path);
            if !dash.is_empty() {
                context.set_line_dash(&js_sys::Array::new())?;
            }
        }
        Style::ClippedStroke { width, .. } => {
            context.save();
            if let Some(clip) = &cached.clip {
                context.clip_with_path_2d_and_winding(clip, CanvasWindingRule::Nonzero);
            }
            context.set_stroke_style_str(&color);
            context.set_line_width(*width);
            context.stroke_with_path(&cached.path);
            context.restore();
        }
        Style::Image { file_id, crop, flip_x, flip_y } => {
            let img = match images.get(file_id, files) {
                ImageState::Ready(img) => img,
                state => {
                    context.set_fill_style_str(&color);
                    context.fill_with_path_2d_and_winding(&cached.path, CanvasWindingRule::Nonzero);
                    return Ok(matches!(state, ImageState::Loading));
                }
            };
            let target = op.path.bounding_box();
            let source = match crop {
                Some(c) => Rect::new(c.x, c.y, c.x + c.width, c.y + c.height),
                None => Rect::new(0.0, 0.0, f64::from(img.natural_width()), f64::from(img.natural_height())),
            };
            if source.width() <= 0.0 || source.height() <= 0.0 {
                return Ok(false);
            }
            // Source pixels → the element's box, mirrored if flipped.
            let mirror = |flipped: bool, extent: f64| if flipped { (-1.0, extent) } else { (1.0, 0.0) };
            let ((mx, tx), (my, ty)) = (mirror(*flip_x, target.width()), mirror(*flip_y, target.height()));
            let fit = Affine::translate(target.origin().to_vec2())
                * Affine::new([mx, 0.0, 0.0, my, tx, ty])
                * Affine::scale_non_uniform(target.width() / source.width(), target.height() / source.height());
            set_transform(context, transform * fit)?;
            let (x, y, sw, sh) = (source.x0, source.y0, source.width(), source.height());
            context.draw_image_with_html_image_element_and_sw_and_sh_and_dx_and_dy_and_dw_and_dh(
                img, x, y, sw, sh, 0.0, 0.0, sw, sh,
            )?;
            set_transform(context, transform)?;
        }
    }
    Ok(false)
}
