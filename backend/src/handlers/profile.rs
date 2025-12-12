use axum::{
    Json,
    extract::{Path, State},
};
use std::sync::Arc;

use crate::models::{ProfileDetail, ProfileListItem};
use crate::services::MySQLClient;
use crate::services::llm::{
    DiagnosticForLLM, ExecutionPlanForLLM, HotspotNodeForLLM, KeyMetricsForLLM, LLMService,
    OperatorDetailForLLM, ProfileDataForLLM, QuerySummaryForLLM, RootCauseAnalysisRequest,
    RootCauseAnalysisResponse, ScanDetailForLLM, determine_connector_type, determine_table_type,
};
use crate::services::profile_analyzer::{
    AnalysisContext, ClusterVariables, LLMEnhancedAnalysis, ProfileAnalysisResponse,
    analyze_profile_with_context, analyzer::QueryComplexity,
};
use crate::utils::{ApiResult, error::ApiError};

/// Validate and sanitize query_id to prevent SQL injection
/// StarRocks query_id format: UUID like "12345678-1234-1234-1234-123456789abc"
///
/// Returns the sanitized (trimmed) query_id as a String.
/// The sanitized version is what should be used for:
/// - SQL queries (security)
/// - API responses (consistency)
/// - Error messages (clarity)
fn sanitize_query_id(query_id: &str) -> Result<String, ApiError> {
    let id = query_id.trim();
    // Allow alphanumeric, hyphens, and underscores (UUID format)
    if id.is_empty()
        || id.len() > 64
        || !id
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_')
    {
        return Err(ApiError::invalid_data("Invalid query_id format"));
    }
    // Return owned String to avoid lifetime issues and ensure consistency
    Ok(id.to_string())
}

// List all query profiles for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/profiles",
    responses(
        (status = 200, description = "List of query profiles", body = Vec<ProfileListItem>),
        (status = 404, description = "No active cluster found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Profiles"
)]
pub async fn list_profiles(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<Json<Vec<ProfileListItem>>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    tracing::info!("Fetching profile list for cluster {}", cluster.id);

    // Get connection pool and execute SHOW PROFILELIST
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    let (columns, rows) = mysql_client.query_raw("SHOW PROFILELIST").await?;

    tracing::info!(
        "Profile list query returned {} rows with {} columns",
        rows.len(),
        columns.len()
    );

    // Convert rows to ProfileListItem, filtering out Aborted queries
    let profiles: Vec<ProfileListItem> = rows
        .into_iter()
        .filter(|row| {
            // Filter out Aborted state (index 3 is State column)
            let state = row.get(3).map(|s| s.as_str()).unwrap_or("");
            !state.eq_ignore_ascii_case("aborted")
        })
        .map(|row| {
            // SHOW PROFILELIST returns: QueryId, StartTime, Time, State, Statement
            ProfileListItem {
                query_id: row.first().cloned().unwrap_or_default(),
                start_time: row.get(1).cloned().unwrap_or_default(),
                time: row.get(2).cloned().unwrap_or_default(),
                state: row.get(3).cloned().unwrap_or_default(),
                statement: row.get(4).cloned().unwrap_or_default(),
            }
        })
        .collect();

    tracing::info!("Successfully converted {} profiles (Aborted filtered)", profiles.len());
    Ok(Json(profiles))
}

// Get detailed profile for a specific query
#[utoipa::path(
    get,
    path = "/api/clusters/profiles/{query_id}",
    params(
        ("query_id" = String, Path, description = "Query ID")
    ),
    responses(
        (status = 200, description = "Query profile detail", body = ProfileDetail),
        (status = 404, description = "No active cluster found or profile not found")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Profiles"
)]
pub async fn get_profile(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(query_id): Path<String>,
) -> ApiResult<Json<ProfileDetail>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    // Sanitize query_id to prevent SQL injection
    // Note: This trims whitespace and validates format. The sanitized version
    // is used consistently for SQL queries, responses, and error messages.
    let safe_query_id = sanitize_query_id(&query_id)?;

    // Log original vs sanitized if they differ (for debugging)
    if query_id.trim() != query_id {
        tracing::debug!("Query ID sanitized: '{}' -> '{}'", query_id, safe_query_id);
    }

    tracing::info!("Fetching profile detail for query {} in cluster {}", safe_query_id, cluster.id);

    // Get connection pool and execute SELECT get_query_profile()
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    let sql = format!("SELECT get_query_profile('{}')", safe_query_id);
    let (_, rows) = mysql_client.query_raw(&sql).await?;

    // Extract profile content from result
    let profile_content = rows
        .first()
        .and_then(|row| row.first())
        .cloned()
        .unwrap_or_default();

    if profile_content.trim().is_empty() {
        return Err(ApiError::not_found(format!("Profile not found for query: {}", safe_query_id)));
    }

    tracing::info!("Profile content length: {} bytes", profile_content.len());

    // Return sanitized query_id in response for consistency
    // This ensures the API contract is clear: responses use the sanitized (trimmed) version
    Ok(Json(ProfileDetail { query_id: safe_query_id, profile_content }))
}

/// Analyze a query profile and return structured visualization data
#[utoipa::path(
    get,
    path = "/api/clusters/profiles/{query_id}/analyze",
    params(
        ("query_id" = String, Path, description = "Query ID to analyze")
    ),
    responses(
        (status = 200, description = "Profile analysis result with execution tree"),
        (status = 404, description = "No active cluster found or profile not found"),
        (status = 500, description = "Profile parsing failed")
    ),
    security(
        ("bearer_auth" = [])
    ),
    tag = "Profiles"
)]
pub async fn analyze_profile_handler(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(query_id): Path<String>,
) -> ApiResult<Json<ProfileAnalysisResponse>> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    // Sanitize query_id to prevent SQL injection
    // Note: This trims whitespace and validates format. The sanitized version
    // is used consistently for SQL queries, responses, and error messages.
    let safe_query_id = sanitize_query_id(&query_id)?;

    // Log original vs sanitized if they differ (for debugging)
    if query_id.trim() != query_id {
        tracing::debug!("Query ID sanitized: '{}' -> '{}'", query_id, safe_query_id);
    }

    tracing::info!("Analyzing profile for query {} in cluster {}", safe_query_id, cluster.id);

    // Fetch profile content from StarRocks database (NOT from test files)
    // This ensures each query_id gets its actual profile data
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);
    let sql = format!("SELECT get_query_profile('{}')", safe_query_id);
    let (_, rows) = mysql_client.query_raw(&sql).await?;

    // Extract profile content from database result
    let profile_content = rows
        .first()
        .and_then(|row| row.first())
        .cloned()
        .unwrap_or_default();

    if profile_content.trim().is_empty() {
        return Err(ApiError::not_found(format!("Profile not found for query: {}", safe_query_id)));
    }

    tracing::info!(
        "Profile content length: {} bytes for query {}",
        profile_content.len(),
        safe_query_id
    );

    // Fetch live cluster session variables for smart parameter recommendations
    // Graceful degradation: if fetching fails, analysis continues without variables
    let cluster_variables = fetch_cluster_variables(&mysql_client).await;

    // Build analysis context with cluster variables and cluster_id for baseline lookup
    let context = AnalysisContext { cluster_variables, cluster_id: Some(cluster.id) };

    // Step 1: Rule engine analysis (骨架 - skeleton, sync, < 100ms)
    let mut response = analyze_profile_with_context(&profile_content, &context)
        .map_err(|e| ApiError::internal_error(format!("Analysis failed: {}", e)))?;

    // Step 2: Mark LLM as available but pending (frontend will call separate API)
    // This allows fast response for DAG rendering while LLM analysis loads async
    if state.llm_service.is_available() {
        response.llm_analysis = Some(LLMEnhancedAnalysis {
            available: true,
            status: "pending".to_string(), // Frontend should call /api/llm/enhance API
            ..Default::default()
        });
    }

    Ok(Json(response))
}

/// Request body for LLM enhancement
#[derive(Debug, serde::Deserialize)]
pub struct EnhanceProfileRequest {
    /// Pre-analyzed profile data from rule engine (avoids re-parsing)
    pub analysis_data: ProfileAnalysisResponse,
    /// Force refresh - bypass cache and call LLM API
    #[serde(default)]
    pub force_refresh: bool,
}

/// POST /api/clusters/:cluster_id/profiles/:query_id/enhance
///
/// Enhance profile analysis with LLM - called async by frontend after DAG is rendered.
/// Receives pre-analyzed data to avoid redundant profile parsing.
pub async fn enhance_profile_handler(
    State(state): State<Arc<crate::AppState>>,
    Path((cluster_id, query_id)): Path<(i64, String)>,
    Json(req): Json<EnhanceProfileRequest>,
) -> ApiResult<Json<LLMEnhancedAnalysis>> {
    let safe_query_id = sanitize_query_id(&query_id)?;

    // Check LLM availability
    if !state.llm_service.is_available() {
        return Ok(Json(LLMEnhancedAnalysis {
            available: false,
            status: "LLM service not available".to_string(),
            ..Default::default()
        }));
    }

    // Fetch cluster variables for LLM context (helps avoid redundant suggestions)
    let cluster_variables = {
        let cluster = state.cluster_service.get_cluster(cluster_id).await.ok();
        if let Some(ref c) = cluster {
            if let Ok(pool) = state.mysql_pool_manager.get_pool(c).await {
                let mysql_client = MySQLClient::from_pool(pool);
                fetch_cluster_variables(&mysql_client).await
            } else {
                None
            }
        } else {
            None
        }
    };

    // Use pre-analyzed data directly, no need to re-fetch profile
    match enhance_with_llm(
        &state.llm_service,
        &req.analysis_data,
        &safe_query_id,
        Some(cluster_id),
        cluster_variables.as_ref(),
        req.force_refresh,
    )
    .await
    {
        Ok(llm_analysis) => Ok(Json(llm_analysis)),
        Err(e) => Ok(Json(LLMEnhancedAnalysis {
            available: false,
            status: format!("failed: {}", e),
            ..Default::default()
        })),
    }
}

/// Enhance profile analysis with LLM-based root cause analysis
///
/// Builds a request from the rule engine results and calls LLM for deeper analysis.
/// Results are merged using ResultMerger to combine rule-based and LLM insights.
async fn enhance_with_llm(
    llm_service: &std::sync::Arc<crate::services::llm::LLMServiceImpl>,
    response: &ProfileAnalysisResponse,
    query_id: &str,
    cluster_id: Option<i64>,
    cluster_variables: Option<&ClusterVariables>,
    force_refresh: bool,
) -> Result<LLMEnhancedAnalysis, String> {
    #[allow(unused_imports)]
    use crate::services::profile_analyzer::{
        LLMCausalChain, LLMHiddenIssue, MergedRecommendation, MergedRootCause,
    };
    use std::collections::HashMap;

    // Build LLM request from profile analysis
    let summary = response.summary.as_ref();

    // Include cluster session variables so LLM knows current settings
    let session_vars: HashMap<String, String> = cluster_variables
        .cloned()
        .unwrap_or_default();

    // Calculate query complexity for LLM context
    let sql = summary.map(|s| s.sql_statement.as_str()).unwrap_or("");
    let complexity = QueryComplexity::from_sql(sql);

    let query_summary = QuerySummaryForLLM {
        sql_statement: summary.map(|s| s.sql_statement.clone()).unwrap_or_default(), // Full SQL, not truncated
        query_type: summary
            .and_then(|s| s.query_type.clone())
            .unwrap_or_else(|| "SELECT".to_string()),
        query_complexity: Some(format!("{:?}", complexity)), // "Simple" | "Medium" | "Complex" | "VeryComplex"
        total_time_seconds: summary
            .map(|s| s.total_time_ms.unwrap_or(0.0) / 1000.0)
            .unwrap_or(0.0),
        scan_bytes: summary.and_then(|s| s.total_bytes_read).unwrap_or(0),
        output_rows: summary.and_then(|s| s.result_rows).unwrap_or(0),
        be_count: summary
            .and_then(|s| s.total_instance_count.map(|c| c as u32))
            .unwrap_or(3),
        has_spill: summary
            .and_then(|s| {
                s.query_spill_bytes
                    .as_ref()
                    .map(|b| !b.is_empty() && b != "0")
            })
            .unwrap_or(false),
        spill_bytes: summary.and_then(|s| s.query_spill_bytes.clone()),
        session_variables: session_vars,
    };

    // Build execution plan description from tree
    let dag_description = response
        .execution_tree
        .as_ref()
        .map(|tree| {
            tree.nodes
                .iter()
                .take(10)
                .map(|n| n.operator_name.clone())
                .collect::<Vec<_>>()
                .join(" -> ")
        })
        .unwrap_or_else(|| "Unknown DAG".to_string());

    // Extract hotspot nodes (time_percentage > 15%)
    let hotspot_nodes: Vec<HotspotNodeForLLM> = response
        .execution_tree
        .as_ref()
        .map(|tree| {
            tree.nodes
                .iter()
                .filter(|n| n.time_percentage.unwrap_or(0.0) > 15.0)
                .take(5)
                .map(|n| HotspotNodeForLLM {
                    operator: n.operator_name.clone(),
                    plan_node_id: n.plan_node_id.unwrap_or(-1),
                    time_percentage: n.time_percentage.unwrap_or(0.0),
                    key_metrics: n
                        .unique_metrics
                        .iter()
                        .take(5)
                        .map(|(k, v)| (k.clone(), v.clone()))
                        .collect(),
                    upstream_operators: vec![],
                })
                .collect()
        })
        .unwrap_or_default();

    let execution_plan = ExecutionPlanForLLM { dag_description, hotspot_nodes };

    // Convert diagnostics to LLM format
    let diagnostics: Vec<DiagnosticForLLM> = response
        .aggregated_diagnostics
        .iter()
        .map(|d| DiagnosticForLLM {
            rule_id: d.rule_id.clone(),
            severity: d.severity.clone(),
            operator: d
                .affected_nodes
                .first()
                .map(|s| s.split('/').next_back().unwrap_or("unknown"))
                .unwrap_or("unknown")
                .to_string(),
            plan_node_id: None,
            message: format!("{} ({}个节点)", d.message, d.node_count),
            evidence: {
                let mut e = HashMap::new();
                e.insert("reason".to_string(), d.reason.clone());
                e.insert("affected_nodes".to_string(), d.affected_nodes.join(", "));
                e
            },
            // AggregatedDiagnostic doesn't have threshold_metadata
            threshold_info: None,
        })
        .collect();

    // Extract scan details with table type info (CRITICAL for correct LLM suggestions)
    let scan_details: Vec<ScanDetailForLLM> = response
        .execution_tree
        .as_ref()
        .map(|tree| {
            tree.nodes
                .iter()
                .filter(|n| n.operator_name.contains("SCAN"))
                .map(|n| {
                    let table_name = n.unique_metrics.get("Table").cloned().unwrap_or_default();
                    let table_type = determine_table_type(&table_name);
                    let connector_type = if table_type == "external" {
                        Some(determine_connector_type(&n.unique_metrics))
                    } else {
                        Some("native".to_string())
                    };

                    ScanDetailForLLM {
                        plan_node_id: n.plan_node_id.unwrap_or(-1),
                        table_name: table_name.clone(),
                        scan_type: n.operator_name.clone(),
                        table_type,
                        connector_type,
                        rows_read: n
                            .unique_metrics
                            .get("RawRowsRead")
                            .and_then(|s| s.replace(",", "").parse().ok())
                            .unwrap_or(0),
                        rows_returned: n.rows.unwrap_or(0),
                        filter_ratio: 0.0,
                        scan_ranges: n
                            .unique_metrics
                            .get("ScanRanges")
                            .and_then(|s| s.replace(",", "").parse().ok()),
                        bytes_read: n
                            .unique_metrics
                            .get("BytesRead")
                            .and_then(|s| s.replace(",", "").replace(" B", "").parse().ok()),
                        io_time_ms: None,
                        cache_hit_rate: n
                            .unique_metrics
                            .get("DataCacheHitRate")
                            .and_then(|s| s.trim_end_matches('%').parse().ok()),
                        predicates: n.unique_metrics.get("Predicates").cloned(),
                        partitions_scanned: n.unique_metrics.get("PartitionsScanned").cloned(),
                        full_table_path: if table_name.contains('.') {
                            Some(table_name.clone())
                        } else {
                            None
                        },
                    }
                })
                .collect()
        })
        .unwrap_or_default();

    // Build profile data for LLM
    let operators: Vec<OperatorDetailForLLM> = response
        .execution_tree
        .as_ref()
        .map(|tree| {
            tree.nodes
                .iter()
                .filter(|n| n.time_percentage.unwrap_or(0.0) > 5.0)
                .map(|n| OperatorDetailForLLM {
                    operator: n.operator_name.clone(),
                    plan_node_id: n.plan_node_id.unwrap_or(-1),
                    time_pct: n.time_percentage.unwrap_or(0.0),
                    rows: n.rows.unwrap_or(0),
                    estimated_rows: None,
                    memory_bytes: None,
                    metrics: n.unique_metrics.clone(),
                })
                .collect()
        })
        .unwrap_or_default();

    let profile_data = ProfileDataForLLM {
        operators,
        time_distribution: None,
        scan_details,
        join_details: vec![],
        agg_details: vec![],
        exchange_details: vec![],
    };

    // Build the LLM request with profile data
    let llm_request = RootCauseAnalysisRequest::builder()
        .query_summary(query_summary)
        .execution_plan(execution_plan)
        .diagnostics(diagnostics)
        .key_metrics(KeyMetricsForLLM::default())
        .profile_data(profile_data)
        .build()
        .map_err(|e| e.to_string())?;

    // Call LLM service with timing
    let start_time = std::time::Instant::now();
    let llm_result = llm_service
        .analyze(&llm_request, query_id, cluster_id, force_refresh)
        .await
        .map_err(|e| e.to_string())?;
    let elapsed_time_ms = start_time.elapsed().as_millis() as u64;

    let llm_response = llm_result.response;
    let from_cache = llm_result.from_cache;

    // Merge LLM response with rule diagnostics
    let root_causes = merge_root_causes(&response.aggregated_diagnostics, &llm_response);
    let recommendations = merge_recommendations(&response.aggregated_diagnostics, &llm_response);

    Ok(LLMEnhancedAnalysis {
        available: true,
        status: "completed".to_string(),
        root_causes,
        causal_chains: llm_response
            .causal_chains
            .into_iter()
            .map(|c| LLMCausalChain { chain: c.chain, explanation: c.explanation })
            .collect(),
        merged_recommendations: recommendations,
        summary: llm_response.summary,
        hidden_issues: llm_response
            .hidden_issues
            .into_iter()
            .map(|h| LLMHiddenIssue { issue: h.issue, suggestion: h.suggestion })
            .collect(),
        from_cache,
        elapsed_time_ms: Some(elapsed_time_ms),
    })
}

/// Merge root causes from rule engine and LLM
fn merge_root_causes(
    rule_diagnostics: &[crate::services::profile_analyzer::AggregatedDiagnostic],
    llm_response: &RootCauseAnalysisResponse,
) -> Vec<crate::services::profile_analyzer::MergedRootCause> {
    use crate::services::profile_analyzer::MergedRootCause;
    use std::collections::HashSet;

    let mut merged = Vec::new();
    let mut seen_ids: HashSet<String> = HashSet::new();

    // First, add LLM root causes (higher priority)
    for llm_rc in &llm_response.root_causes {
        let id = llm_rc.root_cause_id.clone();
        seen_ids.insert(id.clone());

        // Find related rule diagnostics
        let related_rules: Vec<String> = llm_rc
            .symptoms
            .iter()
            .filter(|s| rule_diagnostics.iter().any(|d| &d.rule_id == *s))
            .cloned()
            .collect();

        let source = if related_rules.is_empty() { "llm" } else { "both" };

        merged.push(MergedRootCause {
            id,
            related_rule_ids: related_rules,
            description: llm_rc.description.clone(),
            is_implicit: llm_rc.is_implicit,
            confidence: llm_rc.confidence,
            source: source.to_string(),
            evidence: llm_rc.evidence.clone(),
            symptoms: llm_rc.symptoms.clone(),
        });
    }

    // Add uncovered rule diagnostics as independent issues
    for diag in rule_diagnostics {
        let is_covered = llm_response
            .root_causes
            .iter()
            .any(|rc| rc.symptoms.contains(&diag.rule_id));

        if !is_covered {
            let id = format!("rule_{}", diag.rule_id);
            if !seen_ids.contains(&id) {
                seen_ids.insert(id.clone());
                merged.push(MergedRootCause {
                    id,
                    related_rule_ids: vec![diag.rule_id.clone()],
                    description: diag.message.clone(),
                    is_implicit: false,
                    confidence: 1.0,
                    source: "rule".to_string(),
                    evidence: vec![diag.reason.clone()],
                    symptoms: vec![],
                });
            }
        }
    }

    // Sort by confidence (descending)
    merged.sort_by(|a, b| {
        b.confidence
            .partial_cmp(&a.confidence)
            .unwrap_or(std::cmp::Ordering::Equal)
    });
    merged
}

/// Merge recommendations from rule engine and LLM
fn merge_recommendations(
    rule_diagnostics: &[crate::services::profile_analyzer::AggregatedDiagnostic],
    llm_response: &RootCauseAnalysisResponse,
) -> Vec<crate::services::profile_analyzer::MergedRecommendation> {
    use crate::services::profile_analyzer::MergedRecommendation;
    use std::collections::HashSet;

    let mut merged = Vec::new();
    let mut seen_actions: HashSet<String> = HashSet::new();

    // First, add LLM recommendations (root cause fixes)
    for rec in &llm_response.recommendations {
        let action_key = normalize_action(&rec.action);
        if !seen_actions.contains(&action_key) {
            seen_actions.insert(action_key);
            merged.push(MergedRecommendation {
                priority: rec.priority,
                action: rec.action.clone(),
                expected_improvement: rec.expected_improvement.clone(),
                sql_example: rec.sql_example.clone(),
                source: "llm".to_string(),
                related_root_causes: vec![],
                is_root_cause_fix: true,
            });
        }
    }

    // Add rule engine suggestions
    let mut rule_priority = merged.len() as u32 + 1;
    for diag in rule_diagnostics {
        for suggestion in &diag.suggestions {
            let action_key = normalize_action(suggestion);
            if !seen_actions.contains(&action_key) {
                seen_actions.insert(action_key.clone());
                merged.push(MergedRecommendation {
                    priority: rule_priority,
                    action: suggestion.clone(),
                    expected_improvement: String::new(),
                    sql_example: None,
                    source: "rule".to_string(),
                    related_root_causes: vec![diag.rule_id.clone()],
                    is_root_cause_fix: false,
                });
                rule_priority += 1;
            } else if let Some(existing) = merged
                .iter_mut()
                .find(|r| normalize_action(&r.action) == action_key)
                && existing.source == "llm" {
                    existing.source = "both".to_string();
                }
        }
    }

    merged.sort_by_key(|r| r.priority);
    merged
}

/// Normalize action text for deduplication
fn normalize_action(action: &str) -> String {
    action
        .to_lowercase()
        .chars()
        .filter(|c| c.is_alphanumeric())
        .collect()
}

/// Truncate SQL statement for LLM request (kept for future use)
#[allow(dead_code)]
fn truncate_sql(sql: &str, max_len: usize) -> String {
    if sql.len() <= max_len { sql.to_string() } else { format!("{}...", &sql[..max_len]) }
}

/// Parameters we query from cluster for smart recommendations
/// These are used to provide context-aware parameter suggestions
const CLUSTER_VARIABLE_NAMES: &[&str] = &[
    "query_mem_limit",
    "query_timeout",
    "enable_spill",
    "pipeline_dop",
    "parallel_fragment_exec_instance_num",
    "io_tasks_per_scan_operator",
    "enable_global_runtime_filter",
    "runtime_join_filter_push_down_limit",
    "enable_scan_datacache",
    "enable_populate_datacache",
    "enable_query_cache",
    "pipeline_profile_level",
];

/// Fetch relevant session variables from the cluster
///
/// Returns `None` if query fails (graceful degradation).
/// This allows analysis to continue even if variable fetching fails,
/// though parameter recommendations may be less accurate.
async fn fetch_cluster_variables(mysql_client: &MySQLClient) -> Option<ClusterVariables> {
    // Build SQL query with parameterized variable names
    // Note: Variable names are constants, so SQL injection is not a concern here
    let sql = format!(
        "SHOW VARIABLES WHERE Variable_name IN ({})",
        CLUSTER_VARIABLE_NAMES
            .iter()
            .map(|name| format!("'{}'", name))
            .collect::<Vec<_>>()
            .join(",")
    );

    match mysql_client.query_raw(&sql).await {
        Ok((_, rows)) => {
            let mut variables = ClusterVariables::new();
            for row in rows {
                // SHOW VARIABLES returns: Variable_name, Value
                if row.len() >= 2 {
                    let var_name = row[0].clone();
                    let var_value = row[1].clone();
                    variables.insert(var_name, var_value);
                }
            }
            tracing::debug!(
                "Fetched {} cluster variables for smart recommendations",
                variables.len()
            );
            Some(variables)
        },
        Err(e) => {
            tracing::warn!(
                "Failed to fetch cluster variables: {}, analysis will continue without them",
                e
            );
            None
        },
    }
}
