use std::path::Path;
use std::process::Command;

fn main() {
    println!("cargo:rerun-if-changed=assets/src/app.css");
    println!("cargo:rerun-if-changed=templates/");
    println!("cargo:rerun-if-changed=tailwind.config.js");

    let output = Path::new("assets/static/app.css");
    let input = Path::new("assets/src/app.css");
    let binary = Path::new("bin/tailwindcss");

    let profile = std::env::var("PROFILE").unwrap_or_default();
    let is_release = profile == "release";

    if !binary.exists() {
        // Release builds must not silently ship with stub CSS. Every Docker
        // image that goes to ghcr.io is a release build; any CI or package
        // that skips tailwind would otherwise serve a completely unstyled
        // web UI. Hard-fail so the regression is caught at build time.
        if is_release {
            panic!(
                "Tailwind binary not found at bin/tailwindcss — refusing to build release \
                 without compiled CSS. Run `just tailwind` locally, or install tailwindcss \
                 into bin/tailwindcss in the Docker builder stage."
            );
        }

        println!(
            "cargo:warning=Tailwind binary not found at bin/tailwindcss — run `just tailwind` to build CSS"
        );
        // Create empty output so rust-embed doesn't fail in debug dev loop
        if !output.exists() {
            std::fs::create_dir_all("assets/static").ok();
            std::fs::write(output, "/* tailwind not built yet */").ok();
        }
        return;
    }

    if !input.exists() {
        return;
    }

    let status = Command::new(binary.canonicalize().unwrap())
        .args([
            "-i",
            "assets/src/app.css",
            "-o",
            "assets/static/app.css",
            "--minify",
        ])
        .status();

    match status {
        Ok(s) if s.success() => {}
        Ok(s) => println!("cargo:warning=Tailwind build failed with status: {s}"),
        Err(e) => println!("cargo:warning=Tailwind build error: {e}"),
    }
}
