# xtask - StarRocks Admin Build System

基于 [cargo-xtask](https://github.com/matklad/cargo-xtask) 模式的统一构建系统。

## 快速开始

```bash
# 查看帮助
cargo xtask

# 构建项目 (开发模式)
cargo xtask build

# 构建项目 (发布模式)
cargo xtask build --release

# 运行测试
cargo xtask test

# 格式化代码
cargo xtask format

# 检查代码格式 (不修改)
cargo xtask format --check

# 运行 Clippy 检查
cargo xtask clippy

# 构建并运行
cargo xtask run

# 清理构建产物
cargo xtask clean
```

## 命令说明

### build [--release]

构建前端和后端:

1. 构建前端 (npm build)
2. 运行 Clippy 检查 (仅 release 模式)
3. 构建后端 (cargo build)
4. 创建发布包 (仅 release 模式)

**示例**:
```bash
# 开发构建
cargo xtask build

# 发布构建 (包含优化和打包)
cargo xtask build --release
```

发布构建会在 `build/dist/` 目录创建完整的发布包:
```
build/dist/
├── bin/
│   ├── starrocks-admin          # 二进制文件
│   └── starrocks-admin.sh       # 启动脚本
├── conf/
│   └── config.toml              # 配置文件
├── data/                        # 数据目录
├── logs/                        # 日志目录
└── migrations/                  # 数据库迁移
```

### test

运行所有测试:
```bash
cargo xtask test
```

### format [--check]

格式化代码 (Rust + TypeScript/HTML/CSS):

```bash
# 格式化所有代码
cargo xtask format

# 仅检查格式,不修改
cargo xtask format --check
```

### clippy

运行 Clippy 代码检查:
```bash
cargo xtask clippy
```

### run [ARGS...]

构建并运行应用:
```bash
# 基本运行
cargo xtask run

# 带参数运行
cargo xtask run -- --help
```

### clean

清理所有构建产物:
```bash
cargo xtask clean
```

## 与 Makefile 对比

| Makefile | xtask |
|----------|-------|
| `make build` | `cargo xtask build --release` |
| `make docker-build` | (保持不变) |
| `make clean` | `cargo xtask clean` |

## 优势

- ✅ **跨平台**: 纯 Rust 实现,无需 bash
- ✅ **类型安全**: 编译时检查,减少错误
- ✅ **统一接口**: 所有命令通过 `cargo xtask` 调用
- ✅ **易于扩展**: 添加新任务只需修改 Rust 代码
- ✅ **IDE 支持**: 完整的代码补全和跳转

## 实现细节

xtask 是一个普通的 Rust 包,位于 workspace 中:

```toml
[workspace]
members = ["backend", "xtask"]
```

通过 `.cargo/config.toml` 配置 alias:

```toml
[alias]
xtask = "run --package xtask --"
```

这样 `cargo xtask build` 实际上运行的是 `cargo run --package xtask -- build`。
