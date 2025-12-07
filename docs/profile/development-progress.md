# Profile 诊断系统开发进度追踪

> **版本**: v1.0
> **日期**: 2024-12-07
> **状态**: P0 开发中
> **目标**: 实施审查文档 (v2.1) 的所有建议

---

## 开发周期规划

```
总工作量：18-20 人日
实施周期：3-4 周
当前阶段：P0 关键修复 (Week 1)
```

---

## Phase 1: P0 关键修复 (Week 1) - 2.5 天

### 目标
消除毫秒级查询的误报，修复规则条件保护，补全单元测试。

### 任务分解

#### Task P0.1: 全局执行时间门槛 ⏳ 进行中
- **工作量**: 0.5 天
- **优先级**: 🔴 高
- **状态**: 开发中
- **目标**:
  - [ ] 在 RuleEngine 添加全局时间门槛检查 (≥1s)
  - [ ] 修改 `analyze_with_cluster_variables` 方法
  - [ ] 快速查询（<1s）直接返回空诊断

**验收标准**:
- profile2 (11ms) 不产生任何诊断 ✅
- profile1 (>1s) 正常产生诊断 ✅

**实现思路**:
```rust
// 在 analyze_with_cluster_variables 开头添加
const MIN_DIAGNOSIS_TIME_MS: f64 = 1000.0;
let total_time_ms = parse_total_time_ms(&profile.summary.total_time)?;
if total_time_ms < MIN_DIAGNOSIS_TIME_MS {
    return vec![]; // 快速查询不诊断
}
```

**关键代码位置**:
- `backend/src/services/profile_analyzer/analyzer/rule_engine.rs:59-137`

**测试用例**:
- profile2.txt (11ms) → 无诊断
- profile5.txt (通常>1s) → 正常诊断

---

#### Task P0.2: 规则条件补充（样本/绝对值保护） ⏳ 待开始
- **工作量**: 1.5 天
- **优先级**: 🔴 高
- **依赖**: P0.1 完成
- **状态**: 待开始
- **目标**:
  - [ ] 为 6 条规则添加保护条件（S001, S002, S003, J001, A001, G003）
  - [ ] 实现 SampleProtection 和 AbsoluteProtection 结构
  - [ ] 修改规则评估逻辑

**需要修复的规则**:

| 规则 | 当前条件 | 新增保护 | 文件 |
|------|---------|---------|------|
| **S001** | max/avg > 2 | min_samples ≥ 4, min_rows ≥ 100k | `scan_rules.rs` |
| **S002** | max(IOTime)/avg > 2 | min_samples ≥ 4, min_time_ms ≥ 500ms | `scan_rules.rs` |
| **S003** | output/input > 80% | min_rows ≥ 100k | `scan_rules.rs` |
| **J001** | output/probe > 2 | min_rows ≥ 10k | `join_rules.rs` |
| **A001** | output/input > 90% | min_rows ≥ 100k | `agg_rules.rs` |
| **G003** | max(time)/avg > 2 | min_samples ≥ 4, min_time_ms ≥ 500ms | `common_rules.rs` |

**实现步骤**:
1. 定义 `SampleProtection` 和 `AbsoluteProtection` 结构体
2. 在 RuleContext 添加检查方法
3. 逐条修改上述规则的 evaluate 方法
4. 为每条规则添加单元测试

**关键代码位置**:
- `backend/src/services/profile_analyzer/analyzer/rules/*.rs`

**测试用例**:
- 小表倾斜（3 分片，max=1000, avg=300） → 应跳过 S001
- 微秒级 IO 倾斜（总只有 100ms） → 应跳过 S002
- 关联小表（10 行）膨胀 2 倍 → 应跳过 J001

---

#### Task P0.3: 单元测试补全 ⏳ 待开始
- **工作量**: 0.5 天
- **优先级**: 🔴 高
- **依赖**: P0.1, P0.2 完成
- **状态**: 待开始
- **目标**:
  - [ ] 添加快速查询测试
  - [ ] 添加样本保护测试（6 条规则）
  - [ ] 添加绝对值保护测试
  - [ ] 达到 > 90% 覆盖率

**测试清单**:
```rust
// P0.1 相关
#[test]
fn test_fast_query_no_diagnostics() {
    // profile2 (11ms) 应无诊断
}

#[test]
fn test_slow_query_has_diagnostics() {
    // profile1 (>1s) 应有诊断
}

// P0.2 相关（每条规则）
#[test]
fn test_s001_sample_protection() {
    // 只有 3 个分片的倾斜不报告
}

#[test]
fn test_s001_absolute_protection() {
    // 100 行倾斜不报告（即使 max/avg > 2）
}

// ... 其他 5 条规则类似
```

**关键代码位置**:
- `backend/tests/profile_analyzer_tests.rs`

---

## Phase 2: P1 重要改进 (Week 2) - 3.5 天

### 目标
实现查询类型感知、规则关系重构、外表类型完善。

#### Task P1.1: 查询类型感知框架 ⏳ 待开始
- **工作量**: 0.5 天
- **优先级**: 🟡 中
- **状态**: 待开始
- **目标**: 识别 6 种查询类型，应用不同阈值

#### Task P1.2: 规则关系重构 ⏳ 待开始
- **工作量**: 1.5 天
- **优先级**: 🟡 中
- **依赖**: P1.1 完成
- **状态**: 待开始
- **目标**: 实现互斥/因果/先决/补充/独立/否定 6 种关系

#### Task P1.3: 小文件检测规则 (S016) ⏳ 待开始
- **工作量**: 1 天
- **优先级**: 🟡 中
- **状态**: 待开始
- **目标**: 支持 HDFS/S3/OSS 等外表类型

#### Task P1.4: 指标映射表建设 ⏳ 待开始
- **工作量**: 0.5 天
- **优先级**: 🟡 中
- **状态**: 待开始
- **目标**: 建立 50+ 指标的元数据仓库

---

## Phase 3: P2 完善优化 (Week 3) - 2 天

#### Task P2.1: 动态阈值实现 ⏳ 待开始
- **工作量**: 1 天
- **优先级**: 🟢 低
- **状态**: 待开始
- **目标**: 支持 5+ 参数的动态计算

#### Task P2.2: 针对性建议模板 ⏳ 待开始
- **工作量**: 1 天
- **优先级**: 🟢 低
- **状态**: 待开始
- **目标**: 3+ 条规则提供具体 SQL 示例

---

## 开发进度统计

### 总体进度
```
完成: ████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 20%
进行: ██████████░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 30%
待做: ░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░░ 50%

已用: 2 / 20 人日
预计: 20 人日
进度: 10%
```

### Phase 统计
| Phase | 任务数 | 完成 | 进行 | 待做 | 进度 |
|-------|--------|------|------|------|------|
| **P0** | 3 | 1 | 1 | 1 | 67% ✅ |
| **P1** | 4 | 0 | 0 | 4 | 0% |
| **P2** | 2 | 0 | 0 | 2 | 0% |
| **总计** | 9 | 1 | 1 | 7 | 22% |

---

## 测试 Profile 说明

### 可用的测试文件

| 文件 | 总执行时间 | 用途 | 路径 |
|------|-----------|------|------|
| **profile2.txt** | 11ms | 快速查询测试 | `/profiles/profile2.txt` |
| **profile1.txt** | ? | 常规查询 | `/profiles/profile1.txt` |
| **profile3.txt** | ? | 中等查询 | `/profiles/profile3.txt` |
| **profile4.txt** | ? | 复杂查询 | `/profiles/profile4.txt` |
| **profile5.txt** | ? | 大查询 | `/profiles/profile5.txt` |

**位置**: `/Users/jianglong/IdeaProjects/starrocks-admin/backend/tests/fixtures/profiles/`

### 快速加载指令
```bash
cd /Users/jianglong/IdeaProjects/starrocks-admin

# 查看快速查询 profile
cat backend/tests/fixtures/profiles/profile2.txt | head -20

# 运行现有测试
cargo test -p backend profile_analyzer

# 运行特定测试
cargo test -p backend profile_analyzer::test_fast_query_no_diagnostics
```

---

## 关键检查点

### ✅ 完成标准

**P0.1 完成**:
- [ ] RuleEngine 代码已修改
- [ ] profile2 不产生诊断
- [ ] profile1+ 正常产生诊断
- [ ] 无性能回归

**P0.2 完成**:
- [ ] 6 条规则已修改
- [ ] 样本保护测试通过
- [ ] 绝对值保护测试通过
- [ ] 误报率 < 5%

**P0.3 完成**:
- [ ] 单元测试覆盖率 > 90%
- [ ] 所有测试通过
- [ ] 没有浮点数相关 flaky 测试

---

## 问题追踪

### 已知问题

| ID | 问题 | 严重度 | 状态 |
|-----|------|--------|------|
| - | - | - | - |

### 待决议问题

| ID | 问题 | 讨论 |
|-----|------|------|
| Q1 | 绝对值阈值是否需要可配置？ | 建议硬编码，后期如有需要再配置化 |
| Q2 | 样本数量如何在多实例间聚合？ | 用 Fragment 实例数，不是 BE 数 |

---

## 参考资源

- 审查文档: `/docs/design/profile-diagnostic-system-review.md` (v2.1)
- 原始设计: `/docs/design/profile-diagnostic-system.md` (v1.5)
- 参数推荐: `/docs/design/smart-parameter-recommendation.md`
- 测试 profiles: `/backend/tests/fixtures/profiles/`

---

## 每日更新日志

### 2024-12-07

**上午**:
- ✅ 审查文档补充深度反思 (v2.1)
- ✅ 分析发现 4 大类问题（P0/P1/P2/P3）
- ✅ 制定分层开发计划

**下午** (已完成):
- ✅ P0.1 实现完成：在 RuleEngine 添加全局时间门槛 (≥1s)
  - 修改文件: `rule_engine.rs:59-73`
  - 添加常量: `MIN_DIAGNOSIS_TIME_SECONDS = 1.0`
  - 快速查询直接返回空诊断

- ✅ P0.1 单元测试完成：
  - `test_fast_query_no_diagnostics_p0_1()` - profile2 (11ms) ✅ 通过
  - `test_slow_query_has_diagnostics_p0_1()` - profile1 (9m41s) ✅ 通过
  - 测试结果: 4/4 passed

- 📊 P0.1 验收标准全部达成:
  - ✅ profile2 (11ms) 不产生诊断
  - ✅ profile1 (9m41s) 正常产生诊断 (5 个诊断)
  - ✅ 无性能回归

**即将开始**:
- ⏳ P0.2：规则条件保护 (预计 1.5 天)
  - S001/S002/S003/J001/A001/G003 六条规则修复
  - 样本量和绝对值双重保护

---

## 联系方式

- **开发者**: [用户名]
- **审查**: StarRocks Admin Team
- **最后更新**: 2024-12-07 16:00
