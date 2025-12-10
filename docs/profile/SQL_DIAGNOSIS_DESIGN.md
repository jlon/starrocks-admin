# 实时查询 SQL 诊断功能设计文档 V2

## 0. 批评与自我批评

### 0.1 V1 设计存在的严重问题

#### ❌ 问题1: 缺少 EXPLAIN 执行计划
**批评**: V1 设计最大的缺陷是没有将 `EXPLAIN` 结果发送给 LLM。没有执行计划，LLM 只能做表面的语法分析，无法真正诊断性能问题。

**自我批评**: 作为高级架构师，我应该意识到 SQL 优化的核心是执行计划分析，而不是简单的语法检查。

#### ❌ 问题2: Prompt 过于冗长且缺乏性能焦点
**批评**: V1 的 Prompt 包含大量"正确的废话"（语法检查、可读性等），但对性能分析的指导不够具体。

**自我批评**: Prompt 应该聚焦于 StarRocks 特有的性能问题，而不是通用的 SQL 最佳实践。

#### ❌ 问题3: JSON 响应格式过于复杂
**批评**: `improvements` 数组中的 `before/after` 字段冗余，`warnings` 和 `improvements` 边界模糊。

**自我批评**: 应该简化为更扁平的结构，减少 LLM 输出的 token 消耗。

#### ❌ 问题4: 后端代码冗长，未复用现有逻辑
**批评**: V1 的 Rust 代码示例过于冗长，没有利用现有的工具函数和 trait。

**自我批评**: 应该遵循 DRY 原则，复用 `root_cause.rs` 中的模式。

#### ❌ 问题5: 缺少表统计信息
**批评**: 只有表结构，没有行数、数据量等统计信息，LLM 无法判断是否需要分区裁剪。

---

## 1. 核心改进：增加 EXPLAIN 执行计划

### 1.1 为什么 EXPLAIN 是关键

StarRocks 的 `EXPLAIN` 输出包含关键性能信息：
- **Scan 节点**: 是否全表扫描、分区裁剪是否生效
- **Join 策略**: Broadcast vs Shuffle、Colocate Join
- **聚合方式**: 一阶段/两阶段/四阶段聚合
- **数据分布**: 是否存在 Shuffle
- **基数估算**: 预估行数是否合理

### 1.2 EXPLAIN 类型选择
```sql
-- 使用 EXPLAIN VERBOSE 获取详细信息
EXPLAIN VERBOSE SELECT * FROM orders WHERE order_date > '2024-01-01';
```

输出示例：
```
PLAN FRAGMENT 0
  OUTPUT EXPRS: 1: order_id | 2: customer_id | 3: order_date | 4: amount
  PARTITION: UNPARTITIONED
  RESULT SINK
    EXCHANGE ID: 02
    
PLAN FRAGMENT 1
  OUTPUT EXPRS:
  PARTITION: RANDOM
  STREAM DATA SINK
    EXCHANGE ID: 02
    UNPARTITIONED
    
  1:OlapScanNode
     TABLE: orders
     PREAGGREGATION: ON
     partitions=30/30        <-- 分区裁剪信息
     rollup: orders
     tabletRatio=480/480
     cardinality=10000000    <-- 基数估算
     avgRowSize=32.0
     numNodes=3
```

---

## 2. 重新设计的 API

### 2.1 请求格式（精简）
```json
{
  "sql": "SELECT * FROM orders WHERE order_date > '2024-01-01'",
  "database": "sales_db",
  "catalog": "default_catalog"
}
```

### 2.2 响应格式（精简且聚焦性能）
```json
{
  "ok": true,
  "data": {
    "sql": "优化后的 SQL",
    "changed": true,
    "perf_issues": [
      {
        "type": "full_scan",
        "severity": "high",
        "desc": "全表扫描 orders 表（1000万行），建议添加分区条件",
        "fix": "WHERE order_date >= '2024-01-01'"
      }
    ],
    "explain_analysis": {
      "scan_type": "full_scan | partition_prune | index_scan",
      "join_strategy": "broadcast | shuffle | colocate | none",
      "estimated_rows": 10000000,
      "estimated_cost": "high | medium | low"
    },
    "summary": "发现1个高危性能问题：全表扫描",
    "confidence": 0.9
  },
  "cached": false,
  "ms": 1234
}
```

---

## 3. 重新设计的 Prompt（聚焦性能）

### 3.1 System Prompt（精简版，聚焦性能）

```
你是 StarRocks SQL 性能专家。分析 SQL 和执行计划，识别性能问题。

## 核心任务
1. 分析 EXPLAIN 输出，识别性能瓶颈
2. 给出可直接执行的优化 SQL
3. 量化预期收益

## 性能问题优先级（从高到低）
1. **全表扫描**: partitions=N/N 且 N>10，或 cardinality 过大
2. **笛卡尔积**: CROSS JOIN 或缺少 JOIN 条件
3. **数据倾斜**: Shuffle 后单节点数据量过大
4. **低效 Join**: 大表 Broadcast、未使用 Colocate
5. **冗余计算**: 重复子查询、不必要的 DISTINCT

## EXPLAIN 关键指标解读
- `partitions=M/N`: M<N 表示分区裁剪生效
- `cardinality`: 预估行数，>100万需关注
- `EXCHANGE`: 存在数据 Shuffle，可能是瓶颈
- `BROADCAST`: 小表广播，大表不应 Broadcast
- `COLOCATE`: 最优 Join 方式，无 Shuffle

## 输出规则
1. 只输出有把握的优化，不确定就不说
2. 优化后 SQL 必须语义等价
3. 每个问题必须有具体的 fix 建议
4. severity 只用 high/medium/low
```

### 3.2 User Prompt 模板（包含 EXPLAIN）

```json
{
  "sql": "原始 SQL",
  "explain": "EXPLAIN VERBOSE 输出（关键部分）",
  "schema": {
    "orders": {
      "rows": 10000000,
      "size": "2.5GB",
      "partition_key": "order_date",
      "bucket_key": "order_id",
      "buckets": 16
    }
  },
  "vars": {
    "pipeline_dop": "0",
    "enable_spill": "true"
  }
}
```

### 3.3 输出 JSON Schema（极简）

```json
{
  "sql": "优化后的完整 SQL",
  "changed": true,
  "perf_issues": [
    {
      "type": "full_scan | cartesian | skew | bad_join | redundant",
      "severity": "high | medium | low",
      "desc": "问题描述（一句话）",
      "fix": "修复建议（可执行的 SQL 片段或参数）"
    }
  ],
  "explain_analysis": {
    "scan_type": "full_scan | partition_prune | index_scan",
    "join_strategy": "broadcast | shuffle | colocate | none",
    "estimated_rows": 10000000,
    "estimated_cost": "high | medium | low"
  },
  "summary": "一句话总结",
  "confidence": 0.9
}
```

---

## 4. 后端实现（精简 Rust 代码）

### 4.1 Scenario 实现（复用现有 trait）

文件: `backend/src/services/llm/scenarios/sql_diag.rs`

```rust
//! SQL Diagnosis Scenario - 精简实现

use serde::{Deserialize, Serialize};
use std::collections::hash_map::DefaultHasher;
use std::hash::{Hash, Hasher};

use crate::services::llm::{LLMScenario, LLMAnalysisRequestTrait, LLMAnalysisResponseTrait};

const PROMPT: &str = include_str!("sql_diag_prompt.md");

// ============================================================================
// Request - 极简字段
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SqlDiagReq {
    pub sql: String,
    pub explain: String,                           // EXPLAIN VERBOSE 输出
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub schema: Option<serde_json::Value>,         // 表结构 JSON
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vars: Option<serde_json::Value>,           // Session 变量
}

impl LLMAnalysisRequestTrait for SqlDiagReq {
    fn scenario(&self) -> LLMScenario { LLMScenario::SqlOptimization }
    fn system_prompt(&self) -> String { PROMPT.into() }
    
    fn cache_key(&self) -> String {
        format!("sqldiag:{}", self.sql_hash())
    }
    
    fn sql_hash(&self) -> String {
        let mut h = DefaultHasher::new();
        self.sql.split_whitespace().collect::<Vec<_>>().join(" ").hash(&mut h);
        format!("{:x}", h.finish())
    }
    
    fn profile_hash(&self) -> String {
        let mut h = DefaultHasher::new();
        self.explain.hash(&mut h);
        format!("{:x}", h.finish())
    }
}

// ============================================================================
// Response - 聚焦性能
// ============================================================================

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct SqlDiagResp {
    pub sql: String,
    #[serde(default)]
    pub changed: bool,
    #[serde(default)]
    pub perf_issues: Vec<PerfIssue>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explain_analysis: Option<ExplainAnalysis>,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub confidence: f64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PerfIssue {
    pub r#type: String,      // full_scan | cartesian | skew | bad_join | redundant
    pub severity: String,    // high | medium | low
    pub desc: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub fix: Option<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ExplainAnalysis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scan_type: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub join_strategy: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_rows: Option<u64>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub estimated_cost: Option<String>,
}

impl LLMAnalysisResponseTrait for SqlDiagResp {
    fn summary(&self) -> &str { &self.summary }
    fn confidence(&self) -> Option<f64> { Some(self.confidence) }
}
```

### 4.2 Handler 实现（极简，复用现有服务）

文件: `backend/src/handlers/sql_diag.rs`

```rust
use axum::{extract::{Path, State, Json}, http::StatusCode};
use std::sync::Arc;
use crate::{AppState, handlers::ApiResult};
use crate::services::llm::scenarios::sql_diag::{SqlDiagReq, SqlDiagResp};

#[derive(Debug, serde::Deserialize)]
pub struct DiagReq {
    pub sql: String,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub catalog: Option<String>,
}

#[derive(Debug, serde::Serialize)]
pub struct DiagResp {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<SqlDiagResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    pub cached: bool,
    pub ms: u64,
}

/// POST /api/clusters/:id/sql/diagnose
pub async fn diagnose(
    State(s): State<Arc<AppState>>,
    Path(cid): Path<i64>,
    Json(req): Json<DiagReq>,
) -> ApiResult<Json<DiagResp>> {
    let t0 = std::time::Instant::now();
    
    // 1. 检查 LLM 可用性
    if !s.llm_service.is_available() {
        return Ok(Json(DiagResp { ok: false, data: None, err: Some("LLM unavailable".into()), cached: false, ms: t0.elapsed().as_millis() as u64 }));
    }
    
    // 2. 获取集群连接并执行 EXPLAIN
    let cluster = s.cluster_service.get_cluster(cid).await.map_err(|e| (StatusCode::NOT_FOUND, e.to_string()))?;
    let db = req.database.as_deref().unwrap_or("default");
    let explain = exec_explain(&s, &cluster, db, &req.sql).await.unwrap_or_default();
    
    // 3. 并行获取 schema 和 vars
    let (schema, vars) = tokio::join!(
        fetch_schema(&s, &cluster, db, &req.sql),
        fetch_vars(&s, &cluster)
    );
    
    // 4. 构建请求并调用 LLM
    let llm_req = SqlDiagReq { sql: req.sql.clone(), explain, schema: schema.ok(), vars: vars.ok() };
    let qid = format!("diag_{:x}", t0.elapsed().as_nanos());
    
    match s.llm_service.analyze::<SqlDiagReq, SqlDiagResp>(&llm_req, &qid, Some(cid), false).await {
        Ok(r) => Ok(Json(DiagResp { ok: true, data: Some(r.response), err: None, cached: r.from_cache, ms: t0.elapsed().as_millis() as u64 })),
        Err(e) => Ok(Json(DiagResp { ok: false, data: None, err: Some(e.to_string()), cached: false, ms: t0.elapsed().as_millis() as u64 })),
    }
}

// 执行 EXPLAIN VERBOSE
async fn exec_explain(s: &AppState, cluster: &crate::models::Cluster, db: &str, sql: &str) -> Result<String, String> {
    let explain_sql = format!("EXPLAIN VERBOSE {}", sql);
    s.node_service.execute_sql(cluster, db, &explain_sql).await
        .map(|rows| rows.into_iter().map(|r| r.values().next().cloned().unwrap_or_default()).collect::<Vec<_>>().join("\n"))
        .map_err(|e| e.to_string())
}

// 获取涉及表的 schema（复用现有方法）
async fn fetch_schema(s: &AppState, cluster: &crate::models::Cluster, db: &str, sql: &str) -> Result<serde_json::Value, String> {
    let tables = extract_tables(sql);
    let mut schema = serde_json::Map::new();
    for t in tables.iter().take(5) {  // 最多5个表
        if let Ok(info) = s.node_service.get_table_info(cluster, db, t).await {
            schema.insert(t.clone(), serde_json::to_value(&info).unwrap_or_default());
        }
    }
    Ok(serde_json::Value::Object(schema))
}

// 获取 session 变量（复用现有方法）
async fn fetch_vars(s: &AppState, cluster: &crate::models::Cluster) -> Result<serde_json::Value, String> {
    s.node_service.get_session_variables(cluster).await
        .map(|v| serde_json::to_value(&v).unwrap_or_default())
        .map_err(|e| e.to_string())
}

// 从 SQL 提取表名（简单正则）
fn extract_tables(sql: &str) -> Vec<String> {
    use regex::Regex;
    let re = Regex::new(r"(?i)\b(?:FROM|JOIN|INTO)\s+([`\w]+(?:\.[`\w]+)*)").unwrap();
    re.captures_iter(sql).filter_map(|c| c.get(1).map(|m| m.as_str().trim_matches('`').to_string())).collect()
}
```

---

## 5. Prompt 文件（独立 Markdown）

文件: `backend/src/services/llm/scenarios/sql_diag_prompt.md`

```markdown
你是 StarRocks SQL 性能专家。分析用户 SQL 和 EXPLAIN 执行计划，识别性能问题并给出优化建议。

## 核心任务
1. 分析 EXPLAIN 输出，识别性能瓶颈
2. 给出可直接执行的优化 SQL
3. 量化预期收益

## 性能问题检测（按优先级）

### 🔴 HIGH - 必须修复
| 问题 | EXPLAIN 特征 | 优化方向 |
|------|-------------|---------|
| 全表扫描 | `partitions=N/N` 且 cardinality>100万 | 添加分区条件 |
| 笛卡尔积 | `CROSS JOIN` 或无 JOIN 条件 | 添加 JOIN 条件 |
| 大表 Broadcast | `BROADCAST` + cardinality>100万 | 改用 Shuffle 或 Colocate |

### 🟡 MEDIUM - 建议修复
| 问题 | EXPLAIN 特征 | 优化方向 |
|------|-------------|---------|
| 未使用 Colocate | 同分桶表 JOIN 但无 `COLOCATE` | 检查 Colocate Group |
| 多次 Shuffle | 多个 `EXCHANGE` 节点 | 调整 JOIN 顺序 |
| 基数估算偏差 | cardinality 与实际差距>10倍 | ANALYZE TABLE |

### 🟢 LOW - 可选优化
| 问题 | 特征 | 优化方向 |
|------|------|---------|
| SELECT * | 查询所有列 | 指定需要的列 |
| 缺少 LIMIT | 无结果限制 | 添加 LIMIT |
| 冗余 DISTINCT | GROUP BY 后 DISTINCT | 移除 DISTINCT |

## EXPLAIN 关键指标

```
partitions=M/N     -- M<N 表示分区裁剪生效，M=N 表示全表扫描
cardinality=X      -- 预估行数，>100万需关注
EXCHANGE           -- 数据 Shuffle，可能是瓶颈
BROADCAST          -- 小表广播，大表不应 Broadcast
COLOCATE           -- 最优 Join，无 Shuffle
tabletRatio=A/B    -- A<B 表示 Tablet 裁剪生效
```

## 输出规则
1. **只输出有把握的优化**，不确定就不说
2. **优化后 SQL 必须语义等价**
3. **每个问题必须有具体的 fix**
4. **severity 只用 high/medium/low**
5. **confidence 基于 EXPLAIN 信息的完整度**

## JSON 输出格式

```json
{
  "sql": "优化后的完整 SQL（如无变化则返回原 SQL）",
  "changed": true,
  "perf_issues": [
    {
      "type": "full_scan",
      "severity": "high",
      "desc": "全表扫描 orders 表（预估1000万行）",
      "fix": "添加分区条件: WHERE order_date >= '2024-01-01'"
    }
  ],
  "explain_analysis": {
    "scan_type": "full_scan",
    "join_strategy": "shuffle",
    "estimated_rows": 10000000,
    "estimated_cost": "high"
  },
  "summary": "发现1个高危问题：全表扫描，建议添加分区条件",
  "confidence": 0.9
}
```
```

---

## 6. 对比 V1 vs V2

| 维度 | V1 | V2 |
|------|----|----|
| **EXPLAIN** | ❌ 无 | ✅ 核心输入 |
| **Prompt 长度** | ~800 字 | ~400 字 |
| **性能焦点** | 弱（语法为主） | 强（执行计划为主） |
| **JSON 字段数** | 12+ | 6 |
| **Rust 代码行数** | ~200 | ~80 |
| **复用现有代码** | 低 | 高 |

---

## 7. 实现检查清单

### 后端 ✅ 已完成
- [x] 新增 `sql_diag.rs` scenario（~60行）
- [x] 新增 `sql_diag_prompt.md`（~100行）
- [x] 新增 handler `diagnose`（~100行）
- [x] 注册路由 `/api/clusters/:id/sql/diagnose`
- [x] 并行获取 EXPLAIN、schema、vars
- [x] 复用 MySQLClient 执行查询

### 前端 ✅ 已完成
- [x] 添加"诊断"按钮（warning 样式）
- [x] 诊断结果弹窗（显示性能问题、执行计划分析、优化SQL）
- [x] 接受/拒绝逻辑（应用优化后SQL到编辑器）
- [x] 加载状态、错误处理、缓存标识

---

## 8. 风险与缓解

| 风险 | 缓解措施 |
|------|---------|
| EXPLAIN 执行失败 | 降级为无执行计划诊断 |
| EXPLAIN 输出过长 | 截取前 200 行 |
| LLM 返回非法 JSON | 使用 `serde_json::from_str` 的 `Result` 处理 |
| 优化后 SQL 语义变化 | Prompt 强调语义等价，前端提示用户验证 |

---

## 9. 变更记录

| 版本 | 日期 | 变更 |
|------|------|------|
| V1 | 2024-12-10 | 初始设计 |
| V2 | 2024-12-10 | 增加 EXPLAIN、精简 Prompt、优化 JSON 格式、精简 Rust 代码 |
| V2.1 | 2024-12-10 | 后端实现完成，前端实现完成，编译通过 |
