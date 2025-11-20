// StarRocks Admin - Build Task Runner
// Unified build system using cargo xtask pattern

use anyhow::{Context, Result};
use xshell::{cmd, Shell};

fn main() -> Result<()> {
    let sh = Shell::new()?;
    let args: Vec<_> = std::env::args().skip(1).collect();

    match args.get(0).map(|s| s.as_str()) {
        Some("build") => {
            let release = args.contains(&"--release".to_string());
            build(&sh, release)
        }
        Some("test") => test(&sh),
        Some("format") => {
            let check = args.contains(&"--check".to_string());
            format(&sh, check)
        }
        Some("clippy") => clippy(&sh),
        Some("run") => run(&sh, &args[1..]),
        Some("clean") => clean(&sh),
        Some("coverage") => coverage(&sh),
        Some("ci") => ci(&sh),
        Some("dist") => dist(&sh),
        Some("install") => {
            if args.len() < 2 {
                eprintln!("Error: install requires a destination path");
                eprintln!("Usage: cargo xtask install <destination>");
                std::process::exit(1);
            }
            install(&sh, &args[1])
        }
        _ => {
            print_help();
            Ok(())
        }
    }
}

fn print_help() {
    println!("StarRocks Admin - Build Commands:");
    println!();
    println!("Usage: cargo xtask <COMMAND> [OPTIONS]");
    println!();
    println!("Commands:");
    println!("  build [--release]   Build frontend and backend");
    println!("  test                Run all tests");
    println!("  format [--check]    Format code (check mode doesn't modify)");
    println!("  clippy              Run clippy checks");
    println!("  run [ARGS...]       Build and run the application");
    println!("  clean               Clean build artifacts");
    println!("  coverage            Generate test coverage report");
    println!("  ci                  Run all CI checks (format + clippy + build + test)");
    println!("  dist                Create distribution package (tar.gz)");
    println!("  install <path>      Build and install to specified path");
    println!();
    println!("Examples:");
    println!("  cargo xtask build");
    println!("  cargo xtask build --release");
    println!("  cargo xtask test");
    println!("  cargo xtask format --check");
}

/// Build frontend and backend
fn build(sh: &Shell, release: bool) -> Result<()> {
    println!("🔨 Building StarRocks Admin...");
    println!();

    // Step 1: Build frontend
    println!("📦 [1/3] Building frontend...");
    build_frontend(sh)?;
    println!("✅ Frontend build complete");
    println!();

    // Step 2: Run clippy in release mode
    if release {
        println!("🔍 [2/3] Running clippy checks...");
        clippy(sh)?;
        println!("✅ Clippy checks passed");
        println!();
    }

    // Step 3: Build backend
    println!(
        "🦀 [{}/3] Building backend{}...",
        if release { 3 } else { 2 },
        if release { " (release)" } else { "" }
    );
    build_backend(sh, release)?;
    println!("✅ Backend build complete");
    println!();

    println!("🎉 Build complete!");

    if release {
        println!();
        println!("📦 Distribution package location:");
        println!("   build/dist/");
    }

    Ok(())
}

/// Build frontend using npm
fn build_frontend(sh: &Shell) -> Result<()> {
    let _dir = sh.push_dir(project_root().join("frontend"));

    // Install dependencies
    cmd!(sh, "npm install")
        .run()
        .context("Failed to install frontend dependencies")?;

    // Build production bundle
    cmd!(
        sh,
        "npm run build -- --configuration production --base-href ./"
    )
    .run()
    .context("Failed to build frontend")?;

    Ok(())
}

/// Build backend using cargo
fn build_backend(sh: &Shell, release: bool) -> Result<()> {
    let _dir = sh.push_dir(project_root().join("backend"));

    if release {
        cmd!(sh, "cargo build --release")
            .run()
            .context("Failed to build backend in release mode")?;

        // Create distribution structure
        create_distribution(sh)?;
    } else {
        cmd!(sh, "cargo build")
            .run()
            .context("Failed to build backend")?;
    }

    Ok(())
}

/// Create distribution package structure
fn create_distribution(sh: &Shell) -> Result<()> {
    let project = project_root();
    let dist_dir = project.join("build/dist");

    // Create directories
    cmd!(sh, "mkdir -p {dist_dir}/bin").run()?;
    cmd!(sh, "mkdir -p {dist_dir}/conf").run()?;
    cmd!(sh, "mkdir -p {dist_dir}/lib").run()?;
    cmd!(sh, "mkdir -p {dist_dir}/data").run()?;
    cmd!(sh, "mkdir -p {dist_dir}/logs").run()?;
    cmd!(sh, "mkdir -p {dist_dir}/migrations").run()?;

    // Copy binary
    let binary_src = project.join("backend/target/release/starrocks-admin");
    let binary_dst = dist_dir.join("bin/starrocks-admin");
    cmd!(sh, "cp {binary_src} {binary_dst}").run()?;

    // Copy migrations
    let migrations_src = project.join("backend/migrations");
    let migrations_dst = dist_dir.join("migrations");
    if migrations_src.exists() {
        cmd!(sh, "cp -r {migrations_src}/* {migrations_dst}/").run()?;
    }

    // Create config file
    create_config_file(sh, &dist_dir)?;

    // Copy startup script
    let script_src = project.join("deploy/scripts/starrocks-admin.sh");
    let script_dst = dist_dir.join("bin/starrocks-admin.sh");
    if script_src.exists() {
        cmd!(sh, "cp {script_src} {script_dst}").run()?;
        cmd!(sh, "chmod +x {script_dst}").run()?;
    }

    Ok(())
}

/// Create default config file
fn create_config_file(_sh: &Shell, dist_dir: &std::path::Path) -> Result<()> {
    let config_path = dist_dir.join("conf/config.toml");
    let config_content = r#"[server]
host = "0.0.0.0"
port = 8080

[database]
url = "sqlite://data/starrocks-admin.db"

[auth]
jwt_secret = "dev-secret-key-change-in-production"
jwt_expires_in = "24h"

[logging]
level = "info,starrocks_admin_backend=debug"
file = "logs/starrocks-admin.log"

[static_config]
enabled = true
web_root = "web"

[metrics]
interval_secs = "30s"
retention_days = "7d"
enabled = true
"#;

    std::fs::write(config_path, config_content).context("Failed to create config file")?;

    Ok(())
}

/// Run all tests
fn test(sh: &Shell) -> Result<()> {
    println!("🧪 Running tests...");
    println!();

    let _dir = sh.push_dir(project_root().join("backend"));

    cmd!(sh, "cargo test --workspace")
        .run()
        .context("Tests failed")?;

    println!();
    println!("✅ All tests passed!");

    Ok(())
}

/// Format code
fn format(sh: &Shell, check: bool) -> Result<()> {
    println!("🎨 Formatting code...");
    println!();

    // Format backend
    println!("📝 Formatting Rust code...");
    let _dir = sh.push_dir(project_root().join("backend"));

    if check {
        cmd!(sh, "cargo fmt --all -- --check")
            .run()
            .context("Rust code is not formatted")?;
        println!("✅ Rust code is properly formatted");
    } else {
        cmd!(sh, "cargo fmt --all")
            .run()
            .context("Failed to format Rust code")?;
        println!("✅ Rust code formatted");
    }

    // Format frontend
    println!();
    println!("📝 Formatting TypeScript/HTML/CSS...");
    let _dir = sh.push_dir(project_root().join("frontend"));

    if check {
        cmd!(sh, "npm run format:check")
            .run()
            .context("Frontend code is not formatted")?;
        println!("✅ Frontend code is properly formatted");
    } else {
        // Install dependencies if needed
        if !sh.path_exists("node_modules") {
            cmd!(sh, "npm install").run()?;
        }
        cmd!(sh, "npm run format")
            .run()
            .context("Failed to format frontend code")?;
        println!("✅ Frontend code formatted");
    }

    Ok(())
}

/// Run clippy checks
fn clippy(sh: &Shell) -> Result<()> {
    let _dir = sh.push_dir(project_root().join("backend"));

    cmd!(sh, "cargo clippy --release --all-targets -- --deny warnings --allow clippy::uninlined-format-args")
        .run()
        .context("Clippy checks failed")?;

    Ok(())
}

/// Build and run the application
fn run(sh: &Shell, args: &[String]) -> Result<()> {
    println!("🚀 Building and running StarRocks Admin...");
    println!();

    // Build in debug mode
    build(sh, false)?;

    println!();
    println!("▶️  Starting application...");
    println!();

    let _dir = sh.push_dir(project_root().join("backend"));

    let mut cmd = cmd!(sh, "cargo run --");
    for arg in args {
        cmd = cmd.arg(arg);
    }

    cmd.run().context("Failed to run application")?;

    Ok(())
}

/// Clean build artifacts
fn clean(sh: &Shell) -> Result<()> {
    println!("🧹 Cleaning build artifacts...");
    println!();

    let project = project_root();

    // Clean backend
    println!("🗑️  Cleaning backend...");
    let _dir = sh.push_dir(project.join("backend"));
    cmd!(sh, "cargo clean").run()?;

    // Clean frontend
    println!("🗑️  Cleaning frontend...");
    let frontend_dist = project.join("frontend/dist");
    let frontend_cache = project.join("frontend/node_modules/.cache");
    if frontend_dist.exists() {
        cmd!(sh, "rm -rf {frontend_dist}").run()?;
    }
    if frontend_cache.exists() {
        cmd!(sh, "rm -rf {frontend_cache}").run()?;
    }

    // Clean build directory
    println!("🗑️  Cleaning build directory...");
    let build_dir = project.join("build");
    if build_dir.exists() {
        cmd!(sh, "rm -rf {build_dir}").run()?;
    }

    println!();
    println!("✅ Clean complete!");

    Ok(())
}

/// Run all CI checks (format + clippy + build + test)
fn ci(sh: &Shell) -> Result<()> {
    println!("🔄 Running CI pipeline...");
    println!();

    // Step 1: Format check
    println!("📝 [1/4] Checking code format...");
    format(sh, true)?;
    println!("✅ Format check passed");
    println!();

    // Step 2: Clippy
    println!("🔍 [2/4] Running clippy checks...");
    clippy(sh)?;
    println!("✅ Clippy checks passed");
    println!();

    // Step 3: Build
    println!("🔨 [3/4] Building project...");
    build(sh, true)?;
    println!("✅ Build successful");
    println!();

    // Step 4: Test
    println!("🧪 [4/4] Running tests...");
    test(sh)?;
    println!("✅ All tests passed");
    println!();

    println!("🎉 CI pipeline completed successfully!");

    Ok(())
}

/// Create distribution package (tar.gz)
fn dist(sh: &Shell) -> Result<()> {
    println!("📦 Creating distribution package...");
    println!();

    // Build in release mode
    println!("🔨 Building release version...");
    build(sh, true)?;
    println!();

    let project = project_root();
    let dist_dir = project.join("build/dist");

    // Generate timestamp-based package name
    let timestamp = chrono::Local::now().format("%Y%m%d_%H%M%S");
    let package_name = format!("starrocks-admin-{}.tar.gz", timestamp);
    let package_path = dist_dir.join(&package_name);

    println!("📋 Creating tarball: {}...", package_name);

    // Create tar.gz
    let _dir = sh.push_dir(&dist_dir);
    cmd!(
        sh,
        "tar czf {package_name} bin conf lib data logs migrations"
    )
    .run()
    .context("Failed to create tarball")?;

    println!();
    println!("✅ Distribution package created!");
    println!("   Location: {}", package_path.display());
    println!(
        "   Size: {} MB",
        std::fs::metadata(&package_path)?.len() / 1024 / 1024
    );
    println!();
    println!("📝 To extract:");
    println!("   tar xzf {}", package_name);

    Ok(())
}

/// Install built binary to specified path
fn install(sh: &Shell, destination: &str) -> Result<()> {
    println!("📦 Installing StarRocks Admin to {}...", destination);
    println!();

    // Build in release mode
    println!("🔨 Building release version...");
    build(sh, true)?;
    println!();

    let project = project_root();
    let binary_src = project.join("backend/target/release/starrocks-admin");
    let dest_path = std::path::Path::new(destination);

    // Create destination directory if it doesn't exist
    if let Some(parent) = dest_path.parent() {
        std::fs::create_dir_all(parent).context("Failed to create destination directory")?;
    }

    // Copy binary
    println!("📋 Copying binary to {}...", destination);
    std::fs::copy(&binary_src, dest_path).context("Failed to copy binary")?;

    // Make executable (Unix only)
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let mut perms = std::fs::metadata(dest_path)?.permissions();
        perms.set_mode(0o755);
        std::fs::set_permissions(dest_path, perms)?;
    }

    println!();
    println!("✅ Installation complete!");
    println!("   Binary: {}", destination);

    Ok(())
}

/// Generate test coverage report
fn coverage(sh: &Shell) -> Result<()> {
    println!("📊 Generating test coverage report...");
    println!();

    let _dir = sh.push_dir(project_root().join("backend"));

    // Check if cargo-tarpaulin is installed
    let tarpaulin_check = cmd!(sh, "cargo tarpaulin --version").ignore_status().run();

    if tarpaulin_check.is_err() {
        println!("⚠️  cargo-tarpaulin not found. Installing...");
        cmd!(sh, "cargo install cargo-tarpaulin")
            .run()
            .context("Failed to install cargo-tarpaulin")?;
    }

    // Generate coverage
    cmd!(
        sh,
        "cargo tarpaulin --workspace --out Html --out Xml --output-dir ../build/coverage"
    )
    .run()
    .context("Failed to generate coverage report")?;

    println!();
    println!("✅ Coverage report generated!");
    println!("   HTML: build/coverage/index.html");
    println!("   XML:  build/coverage/cobertura.xml");

    Ok(())
}

/// Get project root directory
fn project_root() -> std::path::PathBuf {
    std::path::Path::new(&env!("CARGO_MANIFEST_DIR"))
        .ancestors()
        .nth(1)
        .unwrap()
        .to_path_buf()
}
