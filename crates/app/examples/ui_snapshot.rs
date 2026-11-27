//! Drives the editor with scripted input and saves what the window would
//! show, chrome included, as a PNG. Handy for checking UI changes without
//! opening a window:
//!
//! ```sh
//! cargo run -p littledraw --example ui_snapshot -- out.png
//! cargo run -p littledraw --example ui_snapshot -- out-dark.png dark
//! ```
//!
//! Given a `.excalidraw` file it shows that drawing instead, nothing selected,
//! in a 1280×800 window (the landing page's picture):
//!
//! ```sh
//! cargo run -p littledraw --example ui_snapshot -- out.png samples/portada.excalidraw
//! ```

use littledraw_core::kurbo::Point;
use littledraw_core::{Button, Document, Editor, Key, Modifiers, Theme};

#[path = "../src/render.rs"]
mod render;

const SIZE: (u32, u32) = (1100, 760);
const FILE_SIZE: (u32, u32) = (1280, 800);

fn main() {
    let args: Vec<String> = std::env::args().skip(1).collect();
    let out = args.first().cloned().unwrap_or_else(|| "ui_snapshot.png".to_owned());
    let dark = args.iter().any(|a| a == "dark");
    let file = args.iter().find(|a| a.ends_with(".excalidraw"));
    let none = Modifiers::default();
    let mut ed = Editor::new(7);
    if dark {
        ed.set_theme(Theme::Dark);
    }
    if let Some(file) = file {
        let json = std::fs::read_to_string(file).unwrap_or_else(|err| panic!("could not read {file}: {err}"));
        // Sized first: loading centers the drawing in the window.
        ed.resize(f64::from(FILE_SIZE.0), f64::from(FILE_SIZE.1));
        ed.load(Document::from_json(&json).expect("not a .excalidraw file"));
        let frame = ed.frame();
        if let Err(err) = render::write_png(&frame, FILE_SIZE.0, FILE_SIZE.1, &ed.document().files, out.as_ref()) {
            eprintln!("could not write {out}: {err}");
            std::process::exit(1);
        }
        return;
    }
    ed.resize(f64::from(SIZE.0), f64::from(SIZE.1));
    let drag = |ed: &mut Editor, tool: char, from: (f64, f64), to: (f64, f64)| {
        ed.key_down(Key::Char(tool), none);
        ed.pointer_down(Point::from(from), Button::Primary, none);
        ed.pointer_move(Point::from(to), none);
        ed.pointer_up(Point::from(to), Button::Primary, none);
    };

    drag(&mut ed, 'o', (330.0, 150.0), (560.0, 290.0));
    ed.double_click(Point::new(445.0, 150.0));
    ed.text_input("Una elipse con su etiqueta");
    ed.key_down(Key::Escape, none);

    drag(&mut ed, 'd', (700.0, 140.0), (900.0, 300.0));
    drag(&mut ed, 'a', (570.0, 220.0), (690.0, 220.0));

    ed.key_down(Key::Char('t'), none);
    ed.pointer_down(Point::new(330.0, 400.0), Button::Primary, none);
    ed.text_input("Texto libre\nen dos líneas");
    ed.key_down(Key::Escape, none);

    drag(&mut ed, 'r', (640.0, 400.0), (900.0, 560.0));
    ed.double_click(Point::new(770.0, 400.0));
    ed.text_input("Editando: el cursor va al final");

    let frame = ed.frame();
    if let Err(err) = render::write_png(&frame, SIZE.0, SIZE.1, &ed.document().files, out.as_ref()) {
        eprintln!("could not write {out}: {err}");
        std::process::exit(1);
    }
}
