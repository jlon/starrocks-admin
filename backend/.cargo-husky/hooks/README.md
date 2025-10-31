# Git Hooks for Rust Backend

This directory contains Git hooks managed by `cargo-husky`.

## 🎣 Configured Hooks

### pre-commit
- **Purpose**: Auto-format code before committing
- **Actions**:
  - Run `cargo fmt --all` to format code
  - Auto-stage formatted `.rs` files
- **Fast**: Usually completes in <1 second

### pre-push
- **Purpose**: Ensure code quality before pushing to remote
- **Actions**:
  1. Check code formatting (`cargo fmt --check`)
  2. Run clippy with strict checks (`-D warnings`)
  3. Run `cargo check` for compilation errors
- **Time**: 5-30 seconds depending on changes

## 🚀 Installation

Hooks are automatically installed when you run:
```bash
cd backend
cargo build
```

## 🔧 Manual Hook Installation

If hooks are not installed automatically:
```bash
cd backend
cargo install cargo-husky --force
```

## ⚙️ Bypass Hooks (Not Recommended)

If you absolutely need to bypass hooks:
```bash
# Skip pre-commit
git commit --no-verify

# Skip pre-push
git push --no-verify
```

**Warning**: Bypassing hooks may result in CI failures.

## 📝 Hook Execution Flow

```
Developer Action        Hook Trigger       Checks Performed
─────────────────────────────────────────────────────────────
git commit       →      pre-commit    →    cargo fmt (auto-fix)
                                           ↓
                                           Commit created
                                           
git push         →      pre-push      →    cargo fmt --check
                                           cargo clippy -D warnings
                                           cargo check
                                           ↓
                                           Push to remote
```

## 🛠 Troubleshooting

### Hook not running?
Check if hook files have execute permission:
```bash
ls -la .git/hooks/
chmod +x .git/hooks/pre-commit
chmod +x .git/hooks/pre-push
```

### Clippy errors?
Run locally to see detailed errors:
```bash
make clippy
# or
cargo clippy --all-targets --all-features -- -D warnings
```

### Format issues?
Auto-fix formatting:
```bash
make fmt
# or
cargo fmt --all
```

## 📚 Related Commands

See `Makefile` in project root for more commands:
- `make fmt` - Format code
- `make clippy` - Run clippy checks
- `make pre-commit` - Run all pre-commit checks manually

