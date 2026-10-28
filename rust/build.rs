// Embed the app icon + version info into LyricToolsPP.exe on Windows builds.
fn main() {
    println!("cargo:rerun-if-changed=assets/start.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/start.ico")
        .set("ProductName", "LyricTools++")
        .set("FileDescription", "LyricTools++ lyric notepad")
        .set("LegalCopyright", "MIT License")
        .set("OriginalFilename", "LyricToolsPP.exe");
    if let Err(err) = res.compile() {
        // A missing resource compiler shouldn't block a dev build; the exe
        // just won't have an icon.
        println!("cargo:warning=no exe icon/version info: {err}");
    }
}
