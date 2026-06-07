use std::fs;
use std::path::Path;

/// Ensures `cargo run` can compile even when the Tauri CLI did not run
/// `beforeBuildCommand` first.
fn main() {
    ensure_frontend_dist();
    tauri_build::build();
}

fn ensure_frontend_dist() {
    println!("cargo:rerun-if-changed=frontend/index.html");
    println!("cargo:rerun-if-changed=frontend/styles.css");
    println!("cargo:rerun-if-changed=frontend/app.js");
    println!("cargo:rerun-if-changed=frontend/src");
    println!("cargo:rerun-if-changed=icons/icon.png");

    let dist = Path::new("frontend").join("dist");
    let dist_app = dist.join("app.js");
    let dist_index = dist.join("index.html");
    let dist_styles = dist.join("styles.css");
    let dist_icon = dist.join("icon.png");

    if dist_app.exists() && dist_index.exists() && dist_styles.exists() && dist_icon.exists() {
        return;
    }

    fs::create_dir_all(&dist).expect("Could not create frontend/dist");
    copy_asset("frontend/index.html", &dist_index);
    copy_asset("frontend/styles.css", &dist_styles);
    copy_asset("icons/icon.png", &dist_icon);

    if !dist_app.exists() {
        copy_asset("frontend/app.js", &dist_app);
    }
}

fn copy_asset(source: &str, destination: &Path) {
    fs::copy(source, destination).unwrap_or_else(|error| {
        panic!(
            "Could not copy {} to {}: {}. Run `npm install` and `npm run build` in frontend.",
            source,
            destination.display(),
            error
        )
    });
}
