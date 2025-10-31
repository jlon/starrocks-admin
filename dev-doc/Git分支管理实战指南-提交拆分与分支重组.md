# Git 分支管理实战指南：提交拆分与分支重组

## 场景描述

在实际开发中，经常会遇到这样的问题：

1. **在某个功能分支上误提交了其他功能的代码**
2. **一次提交包含了多个不相关的功能**
3. **需要将提交拆分到不同的功能分支**

本文档记录了将构建优化相关代码从日志优化分支中分离，并创建独立分支的完整实战过程。

## 实战案例：从日志优化分支分离构建优化和查询修复

### 初始状态

```bash
# 当前分支：feature/log-format-improvement（日志优化分支）
# 发现以下问题：
# 1. 构建优化相关的更改不应该在这个分支
# 2. 查询功能修复也不应该在日志优化分支
# 3. 需要将它们分离到独立的分支
```

### 需要分离的内容

**日志优化分支中包含了：**
- ✅ 日志格式优化（应该保留）
- ❌ 查询功能修复（需要移到新分支）
- ❌ 构建优化（需要移到新分支）

## 实战步骤

### 第一步：检查当前状态

```bash
# 1. 查看当前分支
git branch

# 2. 查看最近的提交历史
git log --oneline -5

# 3. 查看未提交的更改
git status

# 4. 查看提交的详细内容
git show --stat HEAD
```

**示例输出：**
```
On branch feature/log-format-improvement

最近提交：
848ca1f fix(query): 修复运行中的查询功能 - 正确解析 SHOW PROC 列名和格式
3b1a556 docs: 补充后端日志配置指南
3b13157 feat(log): 支持开发/生产环境日志格式切换

未提交更改：
modified:   backend/.cargo/config.toml              # 构建优化
modified:   scripts/dev/start_backend_dev_optimized.sh  # 构建优化
```

### 第二步：分析提交内容

```bash
# 查看最近一次提交修改了哪些文件
git show HEAD --stat

# 查看最近一次提交的详细更改
git show HEAD
```

**发现：**
- `848ca1f` 提交包含：
  - `backend/src/handlers/query.rs` → 查询修复（应移到查询修复分支）
  - `backend/src/services/starrocks_client.rs` → 查询修复（应移到查询修复分支）
  - `frontend/angular.json` → CommonJS 警告修复（应移到查询修复分支）

### 第三步：分离提交

#### 方法一：使用 `git reset --soft`（推荐）

**优点：**
- 保留所有更改在工作区
- 可以重新组织提交
- 不会丢失任何代码

```bash
# 1. 撤销最近的提交，但保留更改在暂存区
git reset --soft HEAD~1

# 2. 查看状态
git status
# 此时会看到之前提交的所有文件都在暂存区
```

**输出：**
```
On branch feature/log-format-improvement
Changes to be committed:
  (use "git restore --staged <file>..." to unstage)
	modified:   backend/src/handlers/query.rs
	modified:   backend/src/services/starrocks_client.rs
	modified:   frontend/angular.json
```

#### 方法二：使用 `git reset --mixed`（保留在工作区）

```bash
# 撤销提交，并将更改移到工作区（未暂存状态）
git reset --mixed HEAD~1
# 或者简写为
git reset HEAD~1
```

### 第四步：分离不同类型的更改

#### 4.1 提交查询修复相关更改

```bash
# 1. 取消暂存所有文件
git reset HEAD

# 2. 只暂存查询修复相关的文件
git add backend/src/handlers/query.rs \
        backend/src/services/starrocks_client.rs \
        frontend/angular.json

# 3. 提交到当前分支（稍后会移到新分支）
git commit -m "fix(query): 修复运行中的查询功能 - 正确解析 SHOW PROC 列名和格式

- 修复列名匹配问题（ScanRows vs ProcessRows）
- 使用大小写不敏感匹配查找列名
- 将 Sql 列设为可选，使用 ExecProgress 作为备用
- 改进日志输出以便调试
- 添加 HTTP API 和 MySQL 客户端双重备用方案
- 修复 Angular CommonJS 警告（添加 nearley 和 sql-formatter）"
```

#### 4.2 创建查询修复分支并移动提交

```bash
# 1. 从 main 分支创建新分支
git checkout -b feature/fix-running-queries main

# 2. 从日志优化分支复制提交的更改
git checkout feature/log-format-improvement -- \
    backend/src/handlers/query.rs \
    backend/src/services/starrocks_client.rs \
    frontend/angular.json

# 3. 提交到新分支
git add backend/src/handlers/query.rs \
        backend/src/services/starrocks_client.rs \
        frontend/angular.json

git commit -m "fix(query): 修复运行中的查询功能 - 正确解析 SHOW PROC 列名和格式

- 修复列名匹配问题（ScanRows vs ProcessRows）
- 使用大小写不敏感匹配查找列名
- 将 Sql 列设为可选，使用 ExecProgress 作为备用
- 改进日志输出以便调试
- 添加 HTTP API 和 MySQL 客户端双重备用方案
- 修复 Angular CommonJS 警告（添加 nearley 和 sql-formatter）"
```

**或者使用更简单的方法（从已提交的版本恢复）：**

```bash
# 1. 创建新分支
git checkout -b feature/fix-running-queries main

# 2. 从日志优化分支的提交中恢复文件
git show feature/log-format-improvement:backend/src/handlers/query.rs > backend/src/handlers/query.rs
git show feature/log-format-improvement:backend/src/services/starrocks_client.rs > backend/src/services/starrocks_client.rs
git show feature/log-format-improvement:frontend/angular.json > frontend/angular.json

# 3. 提交
git add backend/src/handlers/query.rs \
        backend/src/services/starrocks_client.rs \
        frontend/angular.json

git commit -m "fix(query): 修复运行中的查询功能"
```

#### 4.3 清理日志优化分支

```bash
# 1. 切换回日志优化分支
git checkout feature/log-format-improvement

# 2. 查看当前状态（应该只有查询修复的文件还在）
git status

# 3. 由于查询修复已移到新分支，重置到之前的状态
# 如果查询修复是最新提交，可以使用：
git reset --hard HEAD~1

# 或者如果需要保留其他未提交的更改：
git reset --soft HEAD~1
git reset HEAD backend/src/handlers/query.rs \
              backend/src/services/starrocks_client.rs \
              frontend/angular.json
git commit --amend --no-edit  # 如果有其他更改需要保留
```

### 第五步：处理构建优化更改

#### 5.1 创建构建优化分支

```bash
# 1. 从 main 创建构建优化分支
git checkout -b feature/build-optimization main

# 2. 如果构建优化更改还在工作区，直接添加并提交
git add backend/.cargo/config.toml \
        scripts/dev/start_backend_dev_optimized.sh \
        dev-doc/编译速度优化.md

git commit -m "feat(build): 优化 Rust 编译速度和增量编译性能

- 自动检测 CPU 核心数并设置并行任务数
- 优化 codegen-units 配置
- 改进启动脚本，自动设置 CARGO_BUILD_JOBS 环境变量
- 添加详细的编译速度优化指南文档"
```

#### 5.2 或者从其他分支复制更改

```bash
# 如果更改还在日志优化分支的工作区
git checkout feature/log-format-improvement

# 暂存构建优化相关的文件
git add backend/.cargo/config.toml \
        scripts/dev/start_backend_dev_optimized.sh \
        dev-doc/编译速度优化.md

# 切换到构建优化分支
git checkout feature/build-optimization

# 从日志分支复制这些文件
git checkout feature/log-format-improvement -- \
    backend/.cargo/config.toml \
    scripts/dev/start_backend_dev_optimized.sh \
    dev-doc/编译速度优化.md

# 提交
git add backend/.cargo/config.toml \
        scripts/dev/start_backend_dev_optimized.sh \
        dev-doc/编译速度优化.md

git commit -m "feat(build): 优化 Rust 编译速度和增量编译性能"
```

### 第六步：验证分支分离结果

```bash
# 1. 查看所有分支
git branch -a

# 2. 查看日志优化分支的提交（应该只有日志相关）
git checkout feature/log-format-improvement
git log --oneline -5

# 3. 查看查询修复分支的提交
git checkout feature/fix-running-queries
git log --oneline -5

# 4. 查看构建优化分支的提交
git checkout feature/build-optimization
git log --oneline -5

# 5. 查看分支关系图
git log --oneline --graph --all --decorate -10
```

**预期结果：**
```
* 7ffaceb (feature/fix-running-queries) fix(query): 修复运行中的查询功能
| * ced33c9 (feature/build-optimization) feat(build): 优化编译速度
|/  
| * 3b1a556 (feature/log-format-improvement) docs: 补充日志配置指南
| * 3b13157 feat(log): 支持日志格式切换
|/  
* be2ffda (main) 共同基础点
```

### 第七步：清理日志优化分支（最终）

```bash
# 确保日志优化分支只包含日志相关的更改
git checkout feature/log-format-improvement

# 查看当前状态
git status

# 应该显示工作目录干净，只有日志相关的提交
```

## 完整命令序列（参考）

```bash
# === 第一步：检查状态 ===
git status
git log --oneline -5
git show HEAD --stat

# === 第二步：撤销最新提交（保留更改） ===
git reset --soft HEAD~1
git status  # 确认文件在暂存区

# === 第三步：取消暂存，分离文件 ===
git reset HEAD

# === 第四步：创建查询修复分支 ===
git checkout -b feature/fix-running-queries main
git checkout feature/log-format-improvement -- \
    backend/src/handlers/query.rs \
    backend/src/services/starrocks_client.rs \
    frontend/angular.json
git add backend/src/handlers/query.rs \
        backend/src/services/starrocks_client.rs \
        frontend/angular.json
git commit -m "fix(query): 修复运行中的查询功能"

# === 第五步：创建构建优化分支 ===
git checkout -b feature/build-optimization main
git checkout feature/log-format-improvement -- \
    backend/.cargo/config.toml \
    scripts/dev/start_backend_dev_optimized.sh \
    dev-doc/编译速度优化.md
git add backend/.cargo/config.toml \
        scripts/dev/start_backend_dev_optimized.sh \
        dev-doc/编译速度优化.md
git commit -m "feat(build): 优化编译速度"

# === 第六步：清理日志优化分支 ===
git checkout feature/log-format-improvement
git reset --hard HEAD~1  # 或使用其他方法清理
git status  # 确认干净

# === 第七步：验证结果 ===
git log --oneline --graph --all --decorate -10
```

## 常见问题处理

### 问题 1：提交已经推送到远程

如果提交已经推送，需要强制推送（**谨慎使用**）：

```bash
# 1. 重置本地分支（按照上述步骤）
git reset --soft HEAD~1
# ... 分离到新分支 ...

# 2. 强制推送（会覆盖远程分支）
git push --force-with-lease origin feature/log-format-improvement

# 或者创建新分支推送
git push origin feature/fix-running-queries
git push origin feature/build-optimization
```

**注意：** `--force-with-lease` 比 `--force` 更安全，会检查远程是否有其他人的提交。

### 问题 2：有多个提交需要分离

如果有多个提交需要分离：

```bash
# 使用交互式变基
git rebase -i HEAD~3  # 3 是要处理的提交数量

# 在编辑器中：
# - 将不需要的提交标记为 "edit"
# - 保存退出
# - 使用 git reset --soft HEAD~1 分离文件
# - 创建新分支并提交
# - git rebase --continue
```

### 问题 3：文件修改有冲突

如果在分离过程中遇到冲突：

```bash
# 1. 使用 git show 直接恢复文件内容
git show <commit-hash>:<file-path> > <file-path>

# 2. 手动解决冲突
# 编辑文件，保留需要的更改

# 3. 添加到新分支
git add <file>
git commit
```

### 问题 4：需要保留部分更改在原分支

如果某些文件需要在多个分支中：

```bash
# 1. 在目标分支上恢复文件
git checkout feature/target-branch
git checkout feature/source-branch -- path/to/file

# 2. 提交
git add path/to/file
git commit -m "feat: 添加共享文件"
```

## 最佳实践

### 1. 提交前检查

```bash
# 提交前先检查将要提交的内容
git diff --cached  # 查看暂存区的更改
git status         # 查看所有更改
```

### 2. 使用有意义的提交信息

```bash
# 好的提交信息格式：
# <type>(<scope>): <subject>
#
# <body>（可选）
#
# <footer>（可选）
#
# 示例：
git commit -m "fix(query): 修复运行中的查询功能

- 修复列名匹配问题
- 使用大小写不敏感匹配
- 改进错误处理"
```

### 3. 保持分支主题单一

- 一个分支只负责一个功能
- 避免在一个分支上混合多个不相关的功能
- 如果发现混合了，及时分离

### 4. 使用 Git 别名提高效率

```bash
# 添加有用的别名
git config --global alias.lg "log --oneline --graph --all --decorate"
git config --global alias.st "status"
git config --global alias.co "checkout"
git config --global alias.br "branch"

# 使用
git lg -10  # 查看分支图
```

### 5. 定期同步主分支

```bash
# 定期从 main 分支更新功能分支
git checkout feature/your-feature
git merge main
# 或使用 rebase（保持历史线性）
git rebase main
```

## 验证清单

分离完成后，检查以下事项：

- [ ] 每个分支的提交历史只包含相关功能的更改
- [ ] 工作目录干净（`git status` 显示 `nothing to commit`）
- [ ] 分支关系图显示正确的分离结构
- [ ] 每个分支都可以独立编译/运行
- [ ] 提交信息清晰描述了更改内容
- [ ] 没有丢失任何更改

## 实际案例总结

### 本次操作的最终结果

**三个独立分支：**

1. **`feature/log-format-improvement`** (日志优化)
   - 提交：日志格式切换、日志配置指南
   - 文件：日志相关的配置和代码

2. **`feature/fix-running-queries`** (查询修复)
   - 提交：修复运行中查询功能
   - 文件：`query.rs`, `starrocks_client.rs`, `angular.json`

3. **`feature/build-optimization`** (构建优化)
   - 提交：优化编译速度
   - 文件：`config.toml`, `start_backend_dev_optimized.sh`, 优化指南

### 关键命令总结

```bash
# 撤销提交但保留更改
git reset --soft HEAD~1

# 创建新分支
git checkout -b feature/new-branch main

# 从其他分支复制文件
git checkout source-branch -- path/to/file

# 查看分支关系
git log --oneline --graph --all --decorate -10
```

## 注意事项

1. **备份重要更改**：在操作前确保重要代码已保存
2. **团队协作**：如果分支已推送，分离后需要协调团队成员
3. **测试验证**：分离后确保每个分支的功能正常
4. **提交信息**：使用清晰的提交信息，便于后续查找和维护

## 相关资源

- [Git 官方文档](https://git-scm.com/doc)
- [Pro Git 中文版](https://git-scm.com/book/zh/v2)
- [Git 分支管理最佳实践](https://www.git-tower.com/learn/git/ebook/cn/command-line/advanced-topics/branching-best-practices)

---

**文档版本：** 1.0  
**最后更新：** 2025-10-31  
**基于实战案例：** 从日志优化分支分离构建优化和查询修复

