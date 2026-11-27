//! Embeds the icon and version info in the Windows executable, so Explorer,
//! the Start menu shortcut and the taskbar show LittleDraw's icon.

fn main() {
    println!("cargo:rerun-if-changed=../../resources/icon.ico");
    #[cfg(windows)]
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        let mut res = winresource::WindowsResource::new();
        res.set_icon("../../resources/icon.ico").set("ProductName", "LittleDraw").set("FileDescription", "LittleDraw");
        res.compile().expect("could not embed the Windows resources");
    }
}
