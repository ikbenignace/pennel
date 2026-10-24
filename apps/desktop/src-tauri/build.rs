fn main() {
    link_clang_runtime_on_macos();
    tauri_build::build()
}

/// The Metal backend of transcribe.cpp uses `@available` checks, which need `___isPlatformVersionAtLeast`
/// from clang's runtime. Xcode links it for Swift targets; a plain cargo link does not.
fn link_clang_runtime_on_macos() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("macos") {
        return;
    }
    let Ok(output) = std::process::Command::new("clang")
        .arg("--print-runtime-dir")
        .output()
    else {
        return;
    };
    let dir = String::from_utf8_lossy(&output.stdout).trim().to_owned();
    if !dir.is_empty() {
        println!("cargo:rustc-link-search=native={dir}");
        println!("cargo:rustc-link-lib=static=clang_rt.osx");
    }
}
