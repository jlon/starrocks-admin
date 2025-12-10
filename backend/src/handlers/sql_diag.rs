//! SQL Diagnosis Handler - LLM-enhanced SQL performance analysis

use axum::extract::{Json, Path, State};
use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::Arc;

use crate::AppState;
use crate::LLMService;
use crate::services::llm::{SqlDiagReq, SqlDiagResp};
use crate::services::mysql_client::MySQLClient;
use crate::utils::error::ApiResult;

// ============================================================================
// Request/Response
// ============================================================================

#[derive(Debug, Deserialize)]
pub struct DiagReq {
    pub sql: String,
    #[serde(default)]
    pub database: Option<String>,
    #[serde(default)]
    pub catalog: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct DiagResp {
    pub ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub data: Option<SqlDiagResp>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub err: Option<String>,
    pub cached: bool,
    pub ms: u64,
}

impl DiagResp {
    fn ok(data: SqlDiagResp, cached: bool, ms: u64) -> Self {
        Self { ok: true, data: Some(data), err: None, cached, ms }
    }
    fn fail(err: impl Into<String>, ms: u64) -> Self {
        Self { ok: false, data: None, err: Some(err.into()), cached: false, ms }
    }
}

// ============================================================================
// Handler
// ============================================================================

/// POST /api/clusters/:cluster_id/sql/diagnose
#[utoipa::path(
    post,
    path = "/api/clusters/{cluster_id}/sql/diagnose",
    params(("cluster_id" = i64, Path, description = "Cluster ID")),
    request_body = DiagReq,
    responses(
        (status = 200, description = "SQL diagnosis result", body = DiagResp),
        (status = 404, description = "Cluster not found"),
    ),
    security(("bearer_auth" = [])),
    tag = "SQL Diagnosis"
)]
pub async fn diagnose(
    State(s): State<Arc<AppState>>,
    Path(cid): Path<i64>,
    Json(req): Json<DiagReq>,
) -> ApiResult<Json<DiagResp>> {
    let t0 = std::time::Instant::now();
    let ms = || t0.elapsed().as_millis() as u64;

    // 1. Check LLM
    if !s.llm_service.is_available() {
        return Ok(Json(DiagResp::fail("LLM service unavailable", ms())));
    }

    // 2. Get cluster
    let cluster = s.cluster_service.get_cluster(cid).await?;

    // 3. Get MySQL client
    let pool = s.mysql_pool_manager.get_pool(&cluster).await?;
    let client = MySQLClient::from_pool(pool);

    // 4. Parallel fetch: explain + schema + vars
    let db = req.database.as_deref().unwrap_or("");
    let cat = req.catalog.as_deref().unwrap_or("default_catalog");
    let tables = extract_tables(&req.sql);

    let (explain, schema, vars) = tokio::join!(
        exec_explain(&client, cat, db, &req.sql),
        fetch_schema(&client, cat, db, &tables),
        fetch_vars(&client)
    );

    // 5. Build LLM request
    let llm_req = SqlDiagReq {
        sql: req.sql.clone(),
        explain: explain.ok(),
        schema: schema.ok(),
        vars: vars.ok(),
    };

    // 6. Call LLM
    let qid = format!("diag_{:x}", t0.elapsed().as_nanos());
    match s
        .llm_service
        .analyze::<SqlDiagReq, SqlDiagResp>(&llm_req, &qid, Some(cid), false)
        .await
    {
        Ok(r) => Ok(Json(DiagResp::ok(r.response, r.from_cache, ms()))),
        Err(e) => Ok(Json(DiagResp::fail(e.to_string(), ms()))),
    }
}

// ============================================================================
// Helper Functions (精简实现)
// ============================================================================

/// Execute EXPLAIN VERBOSE
async fn exec_explain(
    client: &MySQLClient,
    cat: &str,
    db: &str,
    sql: &str,
) -> Result<String, String> {
    let mut sess = client.create_session().await.map_err(|e| e.to_string())?;
    if !cat.is_empty() && cat != "default_catalog" {
        let _ = sess.use_catalog(cat).await;
    }
    if !db.is_empty() {
        let _ = sess.use_database(db).await;
    }
    let (_, rows, _) = sess
        .execute(&format!("EXPLAIN VERBOSE {}", sql.trim().trim_end_matches(';')))
        .await
        .map_err(|e| e.to_string())?;
    Ok(rows
        .into_iter()
        .flat_map(|r| r.into_iter())
        .take(200)
        .collect::<Vec<_>>()
        .join("\n"))
}

/// Fetch table schemas as JSON
async fn fetch_schema(
    client: &MySQLClient,
    cat: &str,
    db: &str,
    tables: &[String],
) -> Result<serde_json::Value, String> {
    let mut schema = serde_json::Map::new();
    let prefix = match cat {
        "" | "default_catalog" => format!("`{}`", db),
        _ => format!("`{}`.`{}`", cat, db),
    };

    for t in tables.iter().take(5) {
        if let Ok((_, rows)) = client
            .query_raw(&format!("SHOW CREATE TABLE {}.`{}`", prefix, t))
            .await
        {
            rows.first()
                .and_then(|r| r.get(1))
                .map(|ddl| schema.insert(t.clone(), parse_ddl(ddl)));
        }
    }
    Ok(serde_json::Value::Object(schema))
}

/// Parse DDL to extract key info (partition, distribution, rows)
fn parse_ddl(ddl: &str) -> serde_json::Value {
    let mut m = serde_json::Map::new();
    let cap = |pat: &str| Regex::new(pat).ok().and_then(|re| re.captures(ddl));

    cap(r"(?i)PARTITION BY\s+(\w+)\s*\(([^)]+)\)").map(|c| m.insert("partition".into(), serde_json::json!({"type": c.get(1).map(|x| x.as_str()), "key": c.get(2).map(|x| x.as_str())})));
    cap(r"(?i)DISTRIBUTED BY\s+HASH\s*\(([^)]+)\)\s*BUCKETS\s*(\d+)").map(|c| m.insert("dist".into(), serde_json::json!({"key": c.get(1).map(|x| x.as_str()), "buckets": c.get(2).and_then(|x| x.as_str().parse::<u32>().ok())})));
    cap(r#"(?i)"row_count"\s*:\s*"?(\d+)"?"#)
        .and_then(|c| c.get(1)?.as_str().parse::<u64>().ok())
        .map(|n| m.insert("rows".into(), n.into()));

    serde_json::Value::Object(m)
}

/// Fetch session variables
async fn fetch_vars(client: &MySQLClient) -> Result<serde_json::Value, String> {
    const VARS: &[&str] = &[
        "pipeline_dop",
        "enable_spill",
        "query_timeout",
        "broadcast_row_limit",
        "enable_query_cache",
    ];
    let sql = format!(
        "SHOW VARIABLES WHERE Variable_name IN ({})",
        VARS.iter()
            .map(|v| format!("'{}'", v))
            .collect::<Vec<_>>()
            .join(",")
    );
    let (_, rows) = client.query_raw(&sql).await.map_err(|e| e.to_string())?;
    Ok(serde_json::Value::Object(
        rows.into_iter()
            .filter(|r| r.len() >= 2)
            .map(|r| (r[0].clone(), r[1].clone().into()))
            .collect(),
    ))
}

/// Extract table names from SQL
fn extract_tables(sql: &str) -> Vec<String> {
    Regex::new(r"(?i)\b(?:FROM|JOIN|INTO)\s+`?(\w+)`?(?:\.`?(\w+)`?(?:\.`?(\w+)`?)?)?")
        .ok()
        .map(|re| {
            re.captures_iter(sql)
                .filter_map(|c| {
                    c.get(3)
                        .or(c.get(2))
                        .or(c.get(1))
                        .map(|m| m.as_str().into())
                })
                .collect()
        })
        .unwrap_or_default()
}
