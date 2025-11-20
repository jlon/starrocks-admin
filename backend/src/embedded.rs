use rust_embed::RustEmbed;

/// Embedded web assets (frontend build output)
/// This embeds the frontend build output into the binary at compile time.
/// The frontend must be built before compiling Rust (see build scripts).
/// Path is relative to backend/Cargo.toml location.
#[derive(RustEmbed)]
#[folder = "../frontend/dist"]
pub struct WebAssets;
