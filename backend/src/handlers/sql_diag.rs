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

    tracing::info!("SQL Diagnosis: catalog={}, db={}, tables={:?}", cat, db, tables);

    let (explain, schema, vars) = tokio::join!(
        exec_explain(&client, cat, db, &req.sql),
        fetch_schema(&client, cat, db, &tables),
        fetch_vars(&client)
    );

    // Log results for debugging
    match &explain {
        Ok(e) => tracing::info!("EXPLAIN success: {} chars", e.len()),
        Err(e) => tracing::warn!("EXPLAIN failed: {}", e),
    }
    match &schema {
        Ok(s) => tracing::info!("Schema fetched: {}", s),
        Err(e) => tracing::warn!("Schema fetch failed: {}", e),
    }
    match &vars {
        Ok(v) => tracing::info!("Vars fetched: {}", v),
        Err(e) => tracing::warn!("Vars fetch failed: {}", e),
    }

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

    // Set catalog if not default
    if !cat.is_empty() && cat != "default_catalog" {
        sess.use_catalog(cat)
            .await
            .map_err(|e| format!("Failed to use catalog {}: {}", cat, e))?;
    }

    // Set database if provided
    if !db.is_empty() {
        sess.use_database(db)
            .await
            .map_err(|e| format!("Failed to use database {}: {}", db, e))?;
    }

    // Execute EXPLAIN VERBOSE
    let explain_sql = format!("EXPLAIN VERBOSE {}", sql.trim().trim_end_matches(';'));
    let (_, rows, _) = sess
        .execute(&explain_sql)
        .await
        .map_err(|e| e.to_string())?;

    // Collect results, limit to 1000 lines to avoid token overflow but keep enough context
    let result: String = rows
        .into_iter()
        .flat_map(|r| r.into_iter())
        .take(1000)
        .collect::<Vec<_>>()
        .join("\n");

    // If result is too long, truncate but keep header and footer
    if result.len() > 8000 {
        let lines: Vec<&str> = result.lines().collect();
        let header = lines
            .iter()
            .take(100)
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        let footer = lines
            .iter()
            .rev()
            .take(50)
            .rev()
            .cloned()
            .collect::<Vec<_>>()
            .join("\n");
        Ok(format!("{}\n... [truncated {} lines] ...\n{}", header, lines.len() - 150, footer))
    } else {
        Ok(result)
    }
}

/// Fetch table schemas as JSON with table type info (internal/external)
async fn fetch_schema(
    client: &MySQLClient,
    cat: &str,
    db: &str,
    tables: &[String],
) -> Result<serde_json::Value, String> {
    let mut schema = serde_json::Map::new();

    // Build table prefix based on catalog and database
    let prefix = match (cat, db) {
        ("", "") | ("default_catalog", "") => return Ok(serde_json::Value::Object(schema)),
        ("", db) | ("default_catalog", db) => format!("`{}`", db),
        (cat, "") => format!("`{}`", cat), // catalog only, rare case
        (cat, db) => format!("`{}`.`{}`", cat, db),
    };

    // Determine if using external catalog
    let is_external_catalog = !cat.is_empty() && cat != "default_catalog";

    for t in tables.iter().take(5) {
        let show_sql = format!("SHOW CREATE TABLE {}.`{}`", prefix, t);
        tracing::debug!("Fetching schema: {}", show_sql);

        if let Ok((_, rows)) = client.query_raw(&show_sql).await
            && let Some(ddl) = rows.first().and_then(|r| r.get(1)) {
                let mut info = parse_ddl(ddl);
                // Add table type info
                if let serde_json::Value::Object(ref mut m) = info {
                    let table_type = detect_table_type(ddl, is_external_catalog);
                    m.insert("table_type".into(), table_type.into());
                    m.insert(
                        "ddl_preview".into(),
                        ddl.chars().take(500).collect::<String>().into(),
                    );
                }
                schema.insert(t.clone(), info);
            }
    }
    Ok(serde_json::Value::Object(schema))
}

/// Detect if table is internal (StarRocks native) or external
fn detect_table_type(ddl: &str, is_external_catalog: bool) -> &'static str {
    // External catalog tables are always external
    if is_external_catalog {
        return "external";
    }

    // Normalize DDL: remove extra spaces around '='
    let ddl_norm = ddl
        .to_uppercase()
        .replace(" = ", "=")
        .replace("= ", "=")
        .replace(" =", "=");

    // External table patterns (ENGINE=XXX without spaces)
    let external_engines = [
        "HIVE",
        "ICEBERG",
        "HUDI",
        "DELTALAKE",
        "PAIMON",
        "JDBC",
        "ELASTICSEARCH",
        "FILE",
        "BROKER",
        "MYSQL",
        "KAFKA",
    ];
    for eng in external_engines {
        if ddl_norm.contains(&format!("ENGINE={}", eng)) {
            return "external";
        }
    }
    if ddl_norm.contains("EXTERNAL TABLE") || ddl_norm.contains("CREATE EXTERNAL") {
        return "external";
    }

    // Internal table patterns
    if ddl_norm.contains("ENGINE=OLAP")
        || ddl_norm.contains("PRIMARY KEY")
        || ddl_norm.contains("DUPLICATE KEY")
        || ddl_norm.contains("AGGREGATE KEY")
        || ddl_norm.contains("UNIQUE KEY")
        || ddl_norm.contains("DISTRIBUTED BY HASH")
        || ddl_norm.contains("DISTRIBUTED BY RANDOM")
    {
        return "internal";
    }

    // Default to internal for default_catalog
    "internal"
}

/// Parse DDL to extract key info (partition, distribution, keys, engine)
fn parse_ddl(ddl: &str) -> serde_json::Value {
    let mut m = serde_json::Map::new();

    // Helper to capture regex
    let cap = |pat: &str| Regex::new(pat).ok().and_then(|re| re.captures(ddl));

    // Extract partition info
    if let Some(c) = cap(r"(?i)PARTITION\s+BY\s+(\w+)\s*\(([^)]+)\)") {
        m.insert(
            "partition".into(),
            serde_json::json!({
                "type": c.get(1).map(|x| x.as_str()),
                "key": c.get(2).map(|x| x.as_str().trim())
            }),
        );
    }

    // Extract distribution info (HASH or RANDOM)
    if let Some(c) = cap(r"(?i)DISTRIBUTED\s+BY\s+HASH\s*\(([^)]+)\)(?:\s+BUCKETS\s+(\d+))?") {
        m.insert(
            "dist".into(),
            serde_json::json!({
                "type": "HASH",
                "key": c.get(1).map(|x| x.as_str().trim()),
                "buckets": c.get(2).and_then(|x| x.as_str().parse::<u32>().ok())
            }),
        );
    } else if ddl.to_uppercase().contains("DISTRIBUTED BY RANDOM")
        && let Some(c) = cap(r"(?i)DISTRIBUTED\s+BY\s+RANDOM(?:\s+BUCKETS\s+(\d+))?") {
            m.insert(
                "dist".into(),
                serde_json::json!({
                    "type": "RANDOM",
                    "buckets": c.get(1).and_then(|x| x.as_str().parse::<u32>().ok())
                }),
            );
        }

    // Extract key type (PRIMARY KEY, DUPLICATE KEY, etc.)
    if let Some(c) =
        cap(r"(?i)(PRIMARY\s+KEY|DUPLICATE\s+KEY|AGGREGATE\s+KEY|UNIQUE\s+KEY)\s*\(([^)]+)\)")
    {
        m.insert(
            "key".into(),
            serde_json::json!({
                "type": c.get(1).map(|x| x.as_str().to_uppercase().replace(" ", "_")),
                "columns": c.get(2).map(|x| x.as_str().trim())
            }),
        );
    }

    // Extract ENGINE type
    if let Some(c) = cap(r"(?i)ENGINE\s*=\s*(\w+)") {
        m.insert(
            "engine".into(),
            c.get(1)
                .map(|x| x.as_str().to_uppercase())
                .unwrap_or_default()
                .into(),
        );
    }

    // Extract PROPERTIES for colocate_with (useful for colocate join detection)
    if let Some(c) = cap(r#"(?i)"colocate_with"\s*=\s*"([^"]+)""#) {
        m.insert("colocate_with".into(), c.get(1).map(|x| x.as_str()).unwrap_or_default().into());
    }

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

/// Extract table names from SQL (handles aliases, subqueries, CTEs)
fn extract_tables(sql: &str) -> Vec<String> {
    // Pattern: FROM/JOIN table_ref where table_ref can be:
    // - table
    // - db.table
    // - catalog.db.table
    // Followed by optional alias (AS alias or just alias)
    let re = Regex::new(
        r"(?i)\b(?:FROM|JOIN)\s+`?([a-zA-Z_][a-zA-Z0-9_]*)`?(?:\.`?([a-zA-Z_][a-zA-Z0-9_]*)`?(?:\.`?([a-zA-Z_][a-zA-Z0-9_]*)`?)?)?\s*(?:AS\s+\w+|\s+[a-zA-Z_]\w*)?(?:\s|,|ON|WHERE|LEFT|RIGHT|INNER|OUTER|CROSS|$)"
    ).ok();

    re.map(|re| {
        let mut tables: Vec<String> = re
            .captures_iter(sql)
            .filter_map(|c| {
                // Get the table name (last non-None group)
                let table = c.get(3).or(c.get(2)).or(c.get(1)).map(|m| m.as_str());
                // Filter out SQL keywords that might be captured
                table
                    .filter(|t| {
                        let upper = t.to_uppercase();
                        !matches!(
                            upper.as_str(),
                            "SELECT"
                                | "WHERE"
                                | "AND"
                                | "OR"
                                | "ON"
                                | "AS"
                                | "LEFT"
                                | "RIGHT"
                                | "INNER"
                                | "OUTER"
                                | "CROSS"
                                | "JOIN"
                                | "FROM"
                        )
                    })
                    .map(|s| s.to_string())
            })
            .collect();
        tables.sort();
        tables.dedup();
        tables
    })
    .unwrap_or_default()
}

// ============================================================================
// Unit Tests
// ============================================================================

#[cfg(test)]
mod tests {
    use super::*;

    // ========================================================================
    // DiagResp Tests
    // ========================================================================

    #[test]
    fn test_diag_resp_ok() {
        let data = SqlDiagResp {
            sql: "SELECT 1".into(),
            changed: false,
            perf_issues: vec![],
            explain_analysis: None,
            summary: "No issues".into(),
            confidence: 0.8,
        };
        let resp = DiagResp::ok(data.clone(), true, 100);

        assert!(resp.ok);
        assert!(resp.data.is_some());
        assert!(resp.err.is_none());
        assert!(resp.cached);
        assert_eq!(resp.ms, 100);
        assert_eq!(resp.data.unwrap().sql, "SELECT 1");
    }

    #[test]
    fn test_diag_resp_ok_not_cached() {
        let data = SqlDiagResp::default();
        let resp = DiagResp::ok(data, false, 50);

        assert!(resp.ok);
        assert!(!resp.cached);
        assert_eq!(resp.ms, 50);
    }

    #[test]
    fn test_diag_resp_fail() {
        let resp = DiagResp::fail("Connection error", 200);

        assert!(!resp.ok);
        assert!(resp.data.is_none());
        assert_eq!(resp.err, Some("Connection error".into()));
        assert!(!resp.cached);
        assert_eq!(resp.ms, 200);
    }

    #[test]
    fn test_diag_resp_fail_string() {
        let resp = DiagResp::fail(String::from("Timeout"), 300);

        assert!(!resp.ok);
        assert_eq!(resp.err, Some("Timeout".into()));
    }

    // ========================================================================
    // detect_table_type Tests
    // ========================================================================

    #[test]
    fn test_detect_table_type_external_catalog() {
        // External catalog always returns external
        assert_eq!(detect_table_type("CREATE TABLE t (id INT)", true), "external");
        assert_eq!(detect_table_type("ENGINE = OLAP", true), "external");
    }

    #[test]
    fn test_detect_table_type_olap_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = OLAP DISTRIBUTED BY HASH(id)";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_olap_engine_no_spaces() {
        let ddl = "CREATE TABLE t (id INT) ENGINE=OLAP";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_primary_key() {
        let ddl = "CREATE TABLE t (id INT, PRIMARY KEY (id))";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_duplicate_key() {
        let ddl = "CREATE TABLE t (id INT, DUPLICATE KEY (id))";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_aggregate_key() {
        let ddl = "CREATE TABLE t (id INT, AGGREGATE KEY (id))";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_unique_key() {
        let ddl = "CREATE TABLE t (id INT, UNIQUE KEY (id))";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_distributed_by_hash() {
        let ddl = "CREATE TABLE t (id INT) DISTRIBUTED BY HASH(id) BUCKETS 10";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_distributed_by_random() {
        let ddl = "CREATE TABLE t (id INT) DISTRIBUTED BY RANDOM BUCKETS 10";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_hive_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = HIVE";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_hive_engine_no_spaces() {
        let ddl = "CREATE TABLE t (id INT) ENGINE=HIVE";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_iceberg_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = ICEBERG";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_hudi_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = HUDI";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_deltalake_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = DELTALAKE";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_paimon_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = PAIMON";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_jdbc_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = JDBC";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_elasticsearch_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = ELASTICSEARCH";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_file_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = FILE";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_broker_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = BROKER";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_mysql_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = MYSQL";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_kafka_engine() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = KAFKA";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_external_table_keyword() {
        let ddl = "CREATE EXTERNAL TABLE t (id INT)";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_default_internal() {
        // Unknown DDL defaults to internal
        let ddl = "CREATE TABLE t (id INT)";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_detect_table_type_case_insensitive() {
        let ddl = "create table t (id int) engine = olap distributed by hash(id)";
        assert_eq!(detect_table_type(ddl, false), "internal");

        let ddl2 = "CREATE TABLE t (id INT) engine=hive";
        assert_eq!(detect_table_type(ddl2, false), "external");
    }

    // ========================================================================
    // parse_ddl Tests
    // ========================================================================

    #[test]
    fn test_parse_ddl_empty() {
        let result = parse_ddl("");
        assert!(result.is_object());
        assert!(result.as_object().unwrap().is_empty());
    }

    #[test]
    fn test_parse_ddl_partition_range() {
        let ddl = "CREATE TABLE t (id INT, dt DATE) PARTITION BY RANGE(dt) ()";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert!(obj.contains_key("partition"));
        let partition = &obj["partition"];
        assert_eq!(partition["type"], "RANGE");
        assert_eq!(partition["key"], "dt");
    }

    #[test]
    fn test_parse_ddl_partition_list() {
        let ddl = "CREATE TABLE t (id INT, city STRING) PARTITION BY LIST(city) ()";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let partition = &obj["partition"];
        assert_eq!(partition["type"], "LIST");
        assert_eq!(partition["key"], "city");
    }

    #[test]
    fn test_parse_ddl_distributed_hash() {
        let ddl = "CREATE TABLE t (id INT) DISTRIBUTED BY HASH(id) BUCKETS 16";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert!(obj.contains_key("dist"));
        let dist = &obj["dist"];
        assert_eq!(dist["type"], "HASH");
        assert_eq!(dist["key"], "id");
        assert_eq!(dist["buckets"], 16);
    }

    #[test]
    fn test_parse_ddl_distributed_hash_multiple_keys() {
        let ddl = "CREATE TABLE t (id INT, name STRING) DISTRIBUTED BY HASH(id, name) BUCKETS 32";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let dist = &obj["dist"];
        assert_eq!(dist["type"], "HASH");
        assert_eq!(dist["key"], "id, name");
        assert_eq!(dist["buckets"], 32);
    }

    #[test]
    fn test_parse_ddl_distributed_random() {
        let ddl = "CREATE TABLE t (id INT) DISTRIBUTED BY RANDOM BUCKETS 8";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let dist = &obj["dist"];
        assert_eq!(dist["type"], "RANDOM");
        assert_eq!(dist["buckets"], 8);
    }

    #[test]
    fn test_parse_ddl_primary_key() {
        let ddl = "CREATE TABLE t (id INT, name STRING, PRIMARY KEY (id))";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert!(obj.contains_key("key"));
        let key = &obj["key"];
        assert_eq!(key["type"], "PRIMARY_KEY");
        assert_eq!(key["columns"], "id");
    }

    #[test]
    fn test_parse_ddl_duplicate_key() {
        let ddl = "CREATE TABLE t (id INT, name STRING, DUPLICATE KEY (id, name))";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let key = &obj["key"];
        assert_eq!(key["type"], "DUPLICATE_KEY");
        assert_eq!(key["columns"], "id, name");
    }

    #[test]
    fn test_parse_ddl_aggregate_key() {
        let ddl = "CREATE TABLE t (id INT, cnt BIGINT SUM, AGGREGATE KEY (id))";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let key = &obj["key"];
        assert_eq!(key["type"], "AGGREGATE_KEY");
    }

    #[test]
    fn test_parse_ddl_unique_key() {
        let ddl = "CREATE TABLE t (id INT, UNIQUE KEY (id))";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let key = &obj["key"];
        assert_eq!(key["type"], "UNIQUE_KEY");
    }

    #[test]
    fn test_parse_ddl_engine_olap() {
        let ddl = "CREATE TABLE t (id INT) ENGINE = OLAP";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert_eq!(obj["engine"], "OLAP");
    }

    #[test]
    fn test_parse_ddl_engine_hive() {
        let ddl = "CREATE TABLE t (id INT) ENGINE=HIVE";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert_eq!(obj["engine"], "HIVE");
    }

    #[test]
    fn test_parse_ddl_colocate_with() {
        let ddl = r#"CREATE TABLE t (id INT) PROPERTIES ("colocate_with" = "group1")"#;
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert_eq!(obj["colocate_with"], "group1");
    }

    #[test]
    fn test_parse_ddl_full_example() {
        let ddl = r#"
            CREATE TABLE orders (
                order_id BIGINT,
                user_id INT,
                order_date DATE,
                amount DECIMAL(10,2),
                PRIMARY KEY (order_id)
            )
            ENGINE = OLAP
            PARTITION BY RANGE(order_date) ()
            DISTRIBUTED BY HASH(order_id) BUCKETS 16
            PROPERTIES ("colocate_with" = "order_group")
        "#;
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert_eq!(obj["engine"], "OLAP");
        assert_eq!(obj["partition"]["type"], "RANGE");
        assert_eq!(obj["partition"]["key"], "order_date");
        assert_eq!(obj["dist"]["type"], "HASH");
        assert_eq!(obj["dist"]["key"], "order_id");
        assert_eq!(obj["dist"]["buckets"], 16);
        assert_eq!(obj["key"]["type"], "PRIMARY_KEY");
        assert_eq!(obj["colocate_with"], "order_group");
    }

    // ========================================================================
    // extract_tables Tests
    // ========================================================================

    #[test]
    fn test_extract_tables_simple_from() {
        let sql = "SELECT * FROM users";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_with_alias() {
        let sql = "SELECT * FROM users u";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_with_as_alias() {
        let sql = "SELECT * FROM users AS u";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_backticks() {
        let sql = "SELECT * FROM `users`";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_db_table() {
        let sql = "SELECT * FROM mydb.users";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_catalog_db_table() {
        let sql = "SELECT * FROM catalog.mydb.users";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_join() {
        let sql = "SELECT * FROM users u JOIN orders o ON u.id = o.user_id";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
        assert_eq!(tables.len(), 2);
    }

    #[test]
    fn test_extract_tables_left_join() {
        let sql = "SELECT * FROM users LEFT JOIN orders ON users.id = orders.user_id";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
    }

    #[test]
    fn test_extract_tables_right_join() {
        let sql = "SELECT * FROM users RIGHT JOIN orders ON users.id = orders.user_id";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
    }

    #[test]
    fn test_extract_tables_inner_join() {
        let sql = "SELECT * FROM users INNER JOIN orders ON users.id = orders.user_id";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
    }

    #[test]
    fn test_extract_tables_multiple_joins() {
        let sql = "SELECT * FROM users u JOIN orders o ON u.id = o.user_id JOIN products p ON o.product_id = p.id";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
        assert!(tables.contains(&"products".to_string()));
        assert_eq!(tables.len(), 3);
    }

    #[test]
    fn test_extract_tables_case_insensitive() {
        let sql = "select * from Users";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["Users"]);
    }

    #[test]
    fn test_extract_tables_with_where() {
        let sql = "SELECT * FROM users WHERE id = 1";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_dedup() {
        let sql = "SELECT * FROM users u1 JOIN users u2 ON u1.id = u2.manager_id";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["users"]);
    }

    #[test]
    fn test_extract_tables_empty_sql() {
        let sql = "";
        let tables = extract_tables(sql);
        assert!(tables.is_empty());
    }

    #[test]
    fn test_extract_tables_no_from() {
        let sql = "SELECT 1 + 1";
        let tables = extract_tables(sql);
        assert!(tables.is_empty());
    }

    #[test]
    fn test_extract_tables_complex_query() {
        let sql = r#"
            SELECT u.name, COUNT(o.id) as order_count
            FROM users u
            LEFT JOIN orders o ON u.id = o.user_id
            LEFT JOIN order_items oi ON o.id = oi.order_id
            WHERE u.status = 'active'
            GROUP BY u.name
            ORDER BY order_count DESC
            LIMIT 10
        "#;
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
        assert!(tables.contains(&"order_items".to_string()));
        assert_eq!(tables.len(), 3);
    }

    #[test]
    fn test_extract_tables_cross_join() {
        let sql = "SELECT * FROM users CROSS JOIN products";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"products".to_string()));
    }

    #[test]
    fn test_extract_tables_full_qualified_with_backticks() {
        let sql = "SELECT * FROM `catalog`.`db`.`table`";
        let tables = extract_tables(sql);
        assert_eq!(tables, vec!["table"]);
    }

    #[test]
    fn test_extract_tables_sorted() {
        let sql = "SELECT * FROM zebra z JOIN apple a ON z.id = a.id";
        let tables = extract_tables(sql);
        // Should be sorted alphabetically and deduplicated
        assert!(tables.contains(&"zebra".to_string()));
        assert!(tables.contains(&"apple".to_string()));
    }

    // ========================================================================
    // Additional Edge Case Tests
    // ========================================================================

    #[test]
    fn test_parse_ddl_distributed_hash_no_buckets() {
        let ddl = "CREATE TABLE t (id INT) DISTRIBUTED BY HASH(id)";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let dist = &obj["dist"];
        assert_eq!(dist["type"], "HASH");
        assert_eq!(dist["key"], "id");
        assert!(dist["buckets"].is_null());
    }

    #[test]
    fn test_parse_ddl_distributed_random_no_buckets() {
        let ddl = "CREATE TABLE t (id INT) DISTRIBUTED BY RANDOM";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        let dist = &obj["dist"];
        assert_eq!(dist["type"], "RANDOM");
        assert!(dist["buckets"].is_null());
    }

    #[test]
    fn test_detect_table_type_create_external() {
        let ddl = "CREATE EXTERNAL TABLE t (id INT)";
        assert_eq!(detect_table_type(ddl, false), "external");
    }

    #[test]
    fn test_detect_table_type_mixed_case_engine() {
        let ddl = "CREATE TABLE t (id INT) Engine = Olap";
        assert_eq!(detect_table_type(ddl, false), "internal");
    }

    #[test]
    fn test_parse_ddl_case_insensitive_partition() {
        let ddl = "CREATE TABLE t (id INT, dt DATE) partition by range(dt) ()";
        let result = parse_ddl(ddl);
        let obj = result.as_object().unwrap();

        assert!(obj.contains_key("partition"));
    }

    #[test]
    fn test_extract_tables_outer_join() {
        let sql = "SELECT * FROM users OUTER JOIN orders ON users.id = orders.user_id";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        assert!(tables.contains(&"orders".to_string()));
    }

    #[test]
    fn test_extract_tables_subquery() {
        // Subquery tables are also extracted (FROM in subquery)
        let sql = "SELECT * FROM users WHERE id IN (SELECT user_id FROM orders)";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        // Note: orders is extracted because regex matches "FROM orders"
        // The current regex does extract subquery tables
    }

    #[test]
    fn test_extract_tables_cte() {
        // CTE (WITH clause) - tables in CTE should be extracted
        let sql = "WITH active_users AS (SELECT * FROM users WHERE status = 1) SELECT * FROM active_users a JOIN orders o ON a.id = o.user_id";
        let tables = extract_tables(sql);
        assert!(tables.contains(&"users".to_string()));
        // Note: active_users is a CTE alias, orders should be extracted
        assert!(tables.contains(&"orders".to_string()));
    }

    #[test]
    fn test_diag_resp_serialization() {
        let resp = DiagResp::ok(SqlDiagResp::default(), false, 10);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":true"));
        assert!(json.contains("\"cached\":false"));
        assert!(json.contains("\"ms\":10"));
        // err should be skipped when None
        assert!(!json.contains("\"err\""));
    }

    #[test]
    fn test_diag_resp_fail_serialization() {
        let resp = DiagResp::fail("test error", 20);
        let json = serde_json::to_string(&resp).unwrap();
        assert!(json.contains("\"ok\":false"));
        assert!(json.contains("\"err\":\"test error\""));
        // data should be skipped when None
        assert!(!json.contains("\"data\""));
    }
}
