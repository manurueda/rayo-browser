use std::path::Path;

fn main() {
    // Tell rust-embed which folder to use:
    // - ui/out/ if it exists (full Next.js dashboard)
    // - fallback-ui/ otherwise (minimal placeholder)
    let ui_out = Path::new("../../ui/out");
    if ui_out.exists() && ui_out.join("index.html").exists() {
        println!("cargo:rustc-env=RAYO_UI_ASSETS_DIR=../../ui/out/");
    } else {
        println!("cargo:rustc-env=RAYO_UI_ASSETS_DIR=fallback-ui/");
    }
    println!("cargo:rerun-if-changed=../../ui/out/index.html");
    println!("cargo:rerun-if-changed=fallback-ui/index.html");
}
