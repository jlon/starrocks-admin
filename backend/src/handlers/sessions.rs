use axum::{
    Json,
    extract::{Path, State},
    http::StatusCode,
    response::IntoResponse,
};
use serde_json::json;
use std::sync::Arc;

use crate::{
    models::starrocks::Session,
    services::mysql_client::MySQLClient,
    utils::error::{ApiError, ApiResult},
};

/// Get all sessions (connections) for a cluster
#[utoipa::path(
    get,
    path = "/api/clusters/sessions",
    responses(
        (status = 200, description = "Sessions list", body = Vec<Session>),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn get_sessions(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
) -> ApiResult<impl IntoResponse> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    // Get MySQL client from pool
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // Get sessions using MySQL SHOW PROCESSLIST
    let sessions = get_sessions_from_starrocks(&mysql_client).await?;

    Ok(Json(sessions))
}

/// Kill a session (connection)
#[utoipa::path(
    delete,
    path = "/api/clusters/sessions/{session_id}",
    params(
        ("session_id" = String, Path, description = "Session/Connection ID")
    ),
    responses(
        (status = 200, description = "Session killed successfully"),
        (status = 404, description = "No active cluster found"),
        (status = 500, description = "Internal server error")
    ),
    security(
        ("bearer" = [])
    )
)]
pub async fn kill_session(
    State(state): State<Arc<crate::AppState>>,
    axum::extract::Extension(org_ctx): axum::extract::Extension<crate::middleware::OrgContext>,
    Path(session_id): Path<String>,
) -> ApiResult<impl IntoResponse> {
    // Get the active cluster with organization isolation
    let cluster = if org_ctx.is_super_admin {
        state.cluster_service.get_active_cluster().await?
    } else {
        state
            .cluster_service
            .get_active_cluster_by_org(org_ctx.organization_id)
            .await?
    };

    // Get MySQL client from pool
    let pool = state.mysql_pool_manager.get_pool(&cluster).await?;
    let mysql_client = MySQLClient::from_pool(pool);

    // Kill session using MySQL protocol
    kill_session_via_starrocks(&mysql_client, &session_id).await?;

    Ok((StatusCode::OK, Json(json!({ "message": "Session killed successfully" }))))
}

// Helper functions to get sessions from StarRocks
async fn get_sessions_from_starrocks(mysql_client: &MySQLClient) -> ApiResult<Vec<Session>> {
    // StarRocks doesn't have /api/show_proc?path=/sessions endpoint
    // We need to use MySQL protocol to execute SHOW PROCESSLIST
    tracing::info!("Fetching sessions via MySQL SHOW PROCESSLIST");

    // Execute SHOW PROCESSLIST to get all active sessions
    let sql = "SHOW PROCESSLIST";
    let (_, rows) = mysql_client.query_raw(sql).await.map_err(|e| {
        tracing::error!("Failed to execute SHOW PROCESSLIST: {:?}", e);
        ApiError::cluster_connection_failed(format!("Failed to fetch sessions: {:?}", e))
    })?;

    tracing::info!("SHOW PROCESSLIST returned {} rows", rows.len());

    // Parse SHOW PROCESSLIST output
    // Columns: Id, User, Host, Db, Command, Time, State, Info
    let mut sessions = Vec::new();

    for row in rows {
        // row is Vec<String>, we access by index
        // SHOW PROCESSLIST columns: Id(0), User(1), Host(2), Db(3), Command(4), Time(5), State(6), Info(7)
        let id = row.first().cloned().unwrap_or_default();
        let user = row.get(1).cloned().unwrap_or_default();
        let host = row.get(2).cloned().unwrap_or_default();
        let db = row.get(3).cloned();
        let command = row.get(4).cloned().unwrap_or_default();
        let time_str = row.get(5).cloned().unwrap_or_else(|| "0".to_string());
        let state = row.get(6).cloned().unwrap_or_default();
        let info = row.get(7).cloned();

        // Debug log to help identify sleep connections
        tracing::debug!("Session {}: command='{}', state='{}'", id, command, state);

        let session = Session { id, user, host, db, command, time: time_str, state, info };

        sessions.push(session);
    }

    tracing::info!("Fetched {} active sessions", sessions.len());
    Ok(sessions)
}

async fn kill_session_via_starrocks(mysql_client: &MySQLClient, session_id: &str) -> ApiResult<()> {
    // Use MySQL protocol to execute KILL CONNECTION command
    tracing::info!("Killing session: {}", session_id);

    // Try KILL CONNECTION first (preferred), then fallback to KILL
    let kill_sql = format!("KILL CONNECTION {}", session_id);

    match mysql_client.execute(&kill_sql).await {
        Ok(_) => {
            tracing::info!("Successfully killed session: {}", session_id);
            Ok(())
        },
        Err(e) => {
            tracing::warn!("KILL CONNECTION failed, trying KILL: {:?}", e);
            // Fallback to simple KILL
            let fallback_sql = format!("KILL {}", session_id);
            mysql_client.execute(&fallback_sql).await.map_err(|err| {
                tracing::error!("Failed to kill session {}: {:?}", session_id, err);
                ApiError::cluster_connection_failed(format!("Failed to kill session: {:?}", err))
            })?;
            Ok(())
        },
    }
}
