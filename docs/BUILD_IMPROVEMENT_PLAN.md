# StarRocks Admin 构建改进方案

> 基于 Zellij 最佳实践的工程化改进

## 一、现状与问题

### 当前架构
```
Makefile → bash 脚本 → npm/cargo → 手动打包
```

### 主要问题
- ❌ **无 CI/CD**: 没有自动化测试和发布
- ❌ **工具链分散**: Makefile + bash,跨平台差
- ❌ **测试缺失**: 覆盖率约 10%
- ❌ **手动发布**: 耗时 30 分钟,易出错

---

## 二、Zellij 学习要点

### 1. cargo xtask 构建系统 ⭐⭐⭐⭐⭐

**统一的任务管理**:
```bash
cargo xtask build    # 构建
cargo xtask test     # 测试
cargo xtask format   # 格式化
cargo xtask clippy   # 代码检查
cargo xtask run      # 运行
```

**优势**:
- ✅ 纯 Rust,跨平台
- ✅ 类型安全,易维护
- ✅ 统一接口,易扩展

### 2. 完善的 CI/CD ⭐⭐⭐⭐⭐

**GitHub Actions 流程**:
- `rust.yml`: 多平台构建、测试、格式检查
- `e2e.yml`: Docker 容器端到端测试
- `release.yml`: 自动发布、多平台交叉编译、checksum

### 3. Workspace 管理 ⭐⭐⭐⭐

```toml
[workspace]
members = ["backend", "xtask"]

[workspace.dependencies]  # 统一依赖版本
axum = "0.7"
tokio = { version = "1", features = ["full"] }
```

### 4. Release 优化 ⭐⭐⭐⭐

```toml
[profile.release]
lto = true              # 链接时优化
strip = true            # 去除调试符号
codegen-units = 1       # 最大优化
```

---

## 三、改进方案

### Phase 1: 引入 cargo xtask (Week 1-2)

#### 目录结构
```
starrocks-admin/
├── xtask/
│   ├── Cargo.toml
│   └── src/
│       ├── main.rs
│       ├── build.rs
│       ├── test.rs
│       └── release.rs
└── .cargo/config.toml
```

#### 核心实现

**build.rs**:
```rust
pub fn build(sh: &Shell, release: bool) -> Result<()> {
    build_frontend(sh)?;      // npm build
    if release {
        clippy_check(sh)?;    // cargo clippy
    }
    build_backend(sh, release)?;  // cargo build
    create_dist(sh)?;         // 打包
    Ok(())
}
```

**test.rs**:
```rust
pub fn test(sh: &Shell) -> Result<()> {
    run_unit_tests(sh)?;
    run_integration_tests(sh)?;
    Ok(())
}
```

**使用方式**:
```bash
cargo xtask build --release
cargo xtask test
cargo xtask format
```

### Phase 2: CI/CD 自动化 (Week 3-4)

#### .github/workflows/ci.yml
```yaml
name: CI
on: [push, pull_request]

jobs:
  format:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo fmt --check
      
  clippy:
    runs-on: ubuntu-latest
    steps:
      - uses: actions/checkout@v4
      - run: cargo clippy -- --deny warnings
      
  build-test:
    strategy:
      matrix:
        os: [ubuntu-latest, macos-latest]
    runs-on: ${{ matrix.os }}
    steps:
      - uses: actions/checkout@v4
      - run: cargo xtask build
      - run: cargo xtask test
```

#### .github/workflows/release.yml
```yaml
name: Release
on:
  push:
    tags: ['v*.*.*']

jobs:
  release:
    strategy:
      matrix:
        target:
          - x86_64-unknown-linux-musl
          - aarch64-unknown-linux-musl
          - x86_64-apple-darwin
          - aarch64-apple-darwin
    steps:
      - uses: actions/checkout@v4
      - run: cargo xtask release --target ${{ matrix.target }}
      - run: tar czf starrocks-admin-${{ matrix.target }}.tar.gz -C build/dist .
      - run: sha256sum starrocks-admin-${{ matrix.target }}.tar.gz
      - uses: actions/upload-release-asset@v1
```

### Phase 3: 测试体系 (Week 5-6)

#### 测试金字塔
```
E2E Tests (20%)      ← 关键用户流程
Integration (30%)    ← API + 数据库
Unit Tests (60%)     ← 业务逻辑
```

#### 实现
```rust
// backend/src/handlers/cluster_test.rs
#[tokio::test]
async fn test_get_clusters() {
    let app = create_test_app().await;
    let response = app.get("/api/clusters").await;
    assert_eq!(response.status(), 200);
}

// backend/tests/integration_test.rs
#[tokio::test]
async fn test_cluster_lifecycle() {
    // 完整的增删改查流程测试
}
```

---

## 四、实施计划

### Sprint 1 (Week 1-2): xtask 基础
- [ ] 创建 xtask 包
- [ ] 实现 build/test/format/clippy 任务
- [ ] 迁移 Makefile 逻辑
- [ ] 更新文档

### Sprint 2 (Week 3-4): CI/CD
- [ ] 创建 GitHub Actions 工作流
- [ ] 配置多平台构建
- [ ] 自动化测试
- [ ] 代码质量检查

### Sprint 3 (Week 5-6): 测试
- [ ] 单元测试 (目标 60%)
- [ ] 集成测试 (目标 30%)
- [ ] E2E 测试框架

### Sprint 4 (Week 7-8): 发布
- [ ] 自动化发布流程
- [ ] 多平台交叉编译
- [ ] Checksum 生成
- [ ] GitHub Release 自动上传

---

## 五、预期收益

### 效率提升

| 指标 | 当前 | 目标 | 提升 |
|------|------|------|------|
| 构建时间 | 3 min | 2 min | 33% ⬆️ |
| 发布时间 | 30 min | 5 min | 83% ⬆️ |
| 测试覆盖 | 10% | 60% | 500% ⬆️ |
| 支持平台 | 1 | 4 | 300% ⬆️ |

### ROI 分析
```
投资: 16 人天 (6 周)
年化收益: ~305 小时
ROI: 138%
回本周期: 6 个月
```

### 核心价值
- ✅ **自动化**: 构建、测试、发布全自动
- ✅ **质量**: Clippy + 测试覆盖 + 安全审计
- ✅ **效率**: 发布时间从 30 分钟降至 5 分钟
- ✅ **可靠**: 错误率从 10% 降至 <1%

---

## 六、快速开始

### 立即可做的改进

**1. 添加 .cargo/config.toml**:
```toml
[alias]
xtask = "run --package xtask --"
```

**2. 创建 xtask/Cargo.toml**:
```toml
[package]
name = "xtask"
version = "0.1.0"
edition = "2021"

[dependencies]
anyhow = "1.0"
xshell = "0.2"
```

**3. 创建 xtask/src/main.rs**:
```rust
use anyhow::Result;
use xshell::{cmd, Shell};

fn main() -> Result<()> {
    let sh = Shell::new()?;
    let args: Vec<_> = std::env::args().skip(1).collect();
    
    match args.get(0).map(|s| s.as_str()) {
        Some("build") => build(&sh),
        Some("test") => test(&sh),
        _ => {
            println!("Usage: cargo xtask <build|test>");
            Ok(())
        }
    }
}

fn build(sh: &Shell) -> Result<()> {
    cmd!(sh, "bash build/build-frontend.sh").run()?;
    cmd!(sh, "bash build/build-backend.sh").run()?;
    Ok(())
}

fn test(sh: &Shell) -> Result<()> {
    cmd!(sh, "cargo test --workspace").run()?;
    Ok(())
}
```

**4. 使用**:
```bash
cargo xtask build
cargo xtask test
```

---

## 七、参考资料

- [cargo-xtask 模式](https://github.com/matklad/cargo-xtask)
- [Zellij 构建系统](https://github.com/zellij-org/zellij/tree/main/xtask)
- [GitHub Actions 文档](https://docs.github.com/en/actions)
- [Rust 交叉编译](https://rust-lang.github.io/rustup/cross-compilation.html)

---

## 总结

通过学习 Zellij 的最佳实践,我们可以:

1. **用 cargo xtask 替代 Makefile** - 统一、跨平台、类型安全
2. **建立完整 CI/CD** - 自动化测试、多平台构建、自动发布
3. **完善测试体系** - 单元测试 + 集成测试 + E2E 测试
4. **优化发布流程** - 从手动 30 分钟到自动 5 分钟

**实施建议**: 优先 Phase 1 和 Phase 2,快速见效,然后逐步完善测试和发布流程。
