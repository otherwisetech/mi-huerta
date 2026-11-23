//! Windows: embed the app icon and version info in the .exe. Everywhere else a no-op.
fn main() {
    println!("cargo:rerun-if-changed=build.rs");
    println!("cargo:rerun-if-changed=assets/logo/AppIcon.ico");
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() == Ok("windows") {
        windows_resources();
    }
}

#[cfg(windows)]
fn windows_resources() {
    let mut res = winresource::WindowsResource::new();
    res.set_icon("assets/logo/AppIcon.ico");
    res.set("ProductName", "Mi Huerta");
    res.set("FileDescription", "Mi Huerta");
    res.set("InternalName", "mi-huerta");
    res.set("OriginalFilename", "mi-huerta.exe");
    res.compile().expect("embed Windows resources (icon, version info)");
}

#[cfg(not(windows))]
fn windows_resources() {}
