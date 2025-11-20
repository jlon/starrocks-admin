use std::path::Path;

fn main() {
    // Path to embedded web assets (relative to backend directory)
    let web_assets_path = Path::new("../frontend/dist");

    // Create the directory if it doesn't exist (for development environment)
    // This prevents rust-analyzer errors when frontend hasn't been built yet
    if !web_assets_path.exists() {
        println!(
            "cargo:warning=Frontend assets directory doesn't exist yet. Creating placeholder..."
        );
        if let Err(e) = std::fs::create_dir_all(web_assets_path) {
            println!("cargo:warning=Failed to create web assets directory: {}", e);
        }
    }

    // Tell Cargo to rerun this build script if the web assets change
    println!("cargo:rerun-if-changed=../frontend/dist");

    // Also rerun if the build script itself changes
    println!("cargo:rerun-if-changed=build.rs");
}
