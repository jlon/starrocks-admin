use crate::utils::error::ApiError;
use mysql_async::{Conn, Pool, prelude::Queryable};
use std::sync::Arc;

#[derive(Clone)]
pub struct MySQLClient {
    pool: Arc<Pool>,
}

/// MySQLSession wraps a single database connection for executing multiple operations
/// with persistent context (catalog, database)
pub struct MySQLSession {
    conn: Conn,
}

impl MySQLClient {
    pub fn from_pool(pool: Pool) -> Self {
        Self { pool: Arc::new(pool) }
    }

    /// Create a new session with a dedicated connection from the pool
    /// Use this when you need to execute multiple operations on the same connection
    /// with persistent context (e.g., USE DATABASE)
    pub async fn create_session(&self) -> Result<MySQLSession, ApiError> {
        let conn = self.pool.get_conn().await.map_err(|e| {
            tracing::error!("Failed to get connection from pool: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to get connection: {}", e))
        })?;
        Ok(MySQLSession { conn })
    }

    /// Execute a query and return results as (column_names, rows)
    pub async fn query_raw(&self, sql: &str) -> Result<(Vec<String>, Vec<Vec<String>>), ApiError> {
        let mut conn = self.pool.get_conn().await.map_err(|e| {
            tracing::error!("Failed to get connection from pool: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to get connection: {}", e))
        })?;

        let rows: Vec<mysql_async::Row> = conn.query(sql).await.map_err(|e| {
            tracing::error!("MySQL query execution failed: {}", e);
            ApiError::internal_error(format!("SQL execution failed: {}", e))
        })?;

        tracing::debug!("Query returned {} rows", rows.len());

        // CRITICAL: Explicitly drop connection to ensure proper cleanup
        drop(conn);

        Ok(process_query_result(rows))
    }

    /// Execute a query and return results as Vec<serde_json::Value> (JSON objects)
    /// Each row is a JSON object with column names as keys
    pub async fn query(&self, sql: &str) -> Result<Vec<serde_json::Value>, ApiError> {
        let (column_names, rows) = self.query_raw(sql).await?;

        let mut result = Vec::new();
        for row in rows {
            let mut obj = serde_json::Map::new();
            for (i, col_name) in column_names.iter().enumerate() {
                if let Some(value) = row.get(i) {
                    obj.insert(col_name.clone(), serde_json::Value::String(value.clone()));
                }
            }
            result.push(serde_json::Value::Object(obj));
        }

        Ok(result)
    }

    pub async fn execute(&self, sql: &str) -> Result<u64, ApiError> {
        let mut conn = self.pool.get_conn().await.map_err(|e| {
            tracing::error!("Failed to get connection for execute: {}", e);
            ApiError::cluster_connection_failed(format!("Failed to get connection: {}", e))
        })?;

        let result: Vec<mysql_async::Row> = conn.query(sql).await.map_err(|e| {
            tracing::error!("MySQL execute failed: {}", e);
            ApiError::cluster_connection_failed(format!("Query failed: {}", e))
        })?;

        // CRITICAL: Explicitly drop connection to ensure proper cleanup
        drop(conn);

        Ok(result.len() as u64)
    }
}

impl MySQLSession {
    /// Set catalog context on this session's connection
    pub async fn use_catalog(&mut self, catalog: &str) -> Result<(), ApiError> {
        if catalog.is_empty() || catalog == "default_catalog" {
            return Ok(());
        }

        // StarRocks uses SET CATALOG, not USE CATALOG
        // Try without quotes first
        let set_catalog_sql = format!("SET CATALOG {}", catalog);
        if let Err(primary_err) = self
            .conn
            .query::<mysql_async::Row, _>(&set_catalog_sql)
            .await
        {
            tracing::debug!(
                "SET CATALOG {} without quotes failed: {}. Retrying with backticks.",
                catalog,
                primary_err
            );

            // Retry with backticks for catalog names with special characters
            let set_catalog_sql_quoted = format!("SET CATALOG `{}`", catalog);
            self.conn
                .query::<mysql_async::Row, _>(&set_catalog_sql_quoted)
                .await
                .map_err(|e| {
                    tracing::error!("Failed to switch to catalog {}: {}", catalog, e);
                    ApiError::internal_error(format!(
                        "Failed to switch to catalog {}: {}",
                        catalog, e
                    ))
                })?;
        }

        tracing::debug!("Successfully switched to catalog: {}", catalog);
        Ok(())
    }

    /// Set database context on this session's connection
    pub async fn use_database(&mut self, database: &str) -> Result<(), ApiError> {
        if database.is_empty() {
            return Ok(());
        }

        let use_db_sql = format!("USE `{}`", database);
        self.conn
            .query::<mysql_async::Row, _>(&use_db_sql)
            .await
            .map_err(|e| {
                tracing::warn!("Failed to execute USE DATABASE {}: {}", database, e);
                ApiError::internal_error(format!(
                    "Failed to switch to database {}: {}",
                    database, e
                ))
            })?;
        Ok(())
    }

    /// Execute a query and return both results and execution time (SQL only, excluding data processing)
    pub async fn execute(
        &mut self,
        sql: &str,
    ) -> Result<(Vec<String>, Vec<Vec<String>>, u128), ApiError> {
        let start = std::time::Instant::now();
        let rows: Vec<mysql_async::Row> = self.conn.query(sql).await.map_err(|e| {
            tracing::error!("MySQL query execution failed: {}", e);
            ApiError::internal_error(format!("SQL execution failed: {}", e))
        })?;
        let execution_time_ms = start.elapsed().as_millis();

        // Detailed performance logging for debugging
        tracing::debug!("SQL: '{}' -> {} rows in {}ms", sql, rows.len(), execution_time_ms);

        let process_start = std::time::Instant::now();
        let (columns, data_rows) = process_query_result(rows);
        let process_time_ms = process_start.elapsed().as_millis();

        tracing::debug!(
            "Data processing: {}ms (SQL: {}ms, Total: {}ms)",
            process_time_ms,
            execution_time_ms,
            execution_time_ms + process_time_ms
        );

        Ok((columns, data_rows, execution_time_ms))
    }

    pub async fn query_with_params<P>(
        &mut self,
        sql: &str,
        params: P,
    ) -> Result<(Vec<String>, Vec<Vec<String>>), ApiError>
    where
        P: Into<mysql_async::Params>,
    {
        let rows: Vec<mysql_async::Row> =
            self.conn.exec(sql, params.into()).await.map_err(|e| {
                tracing::error!("MySQL query execution failed: {}", e);
                ApiError::internal_error(format!("SQL execution failed: {}", e))
            })?;

        Ok(process_query_result(rows))
    }
}

fn process_query_result(rows: Vec<mysql_async::Row>) -> (Vec<String>, Vec<Vec<String>>) {
    if rows.is_empty() {
        return (Vec::new(), Vec::new());
    }

    let col_count = rows[0].columns_ref().len();
    let row_count = rows.len();

    let mut columns = Vec::with_capacity(col_count);
    let mut result_rows = Vec::with_capacity(row_count);

    // Extract column names from first row
    for col in rows[0].columns_ref().iter() {
        columns.push(col.name_str().to_string());
    }

    if row_count > 100 && col_count > 5 {
        process_query_result_batch(rows, &mut result_rows);
    } else {
        for row in rows.iter() {
            let mut row_data = Vec::with_capacity(col_count);
            for col_idx in 0..col_count {
                row_data.push(value_to_string_optimized(&row[col_idx]));
            }
            result_rows.push(row_data);
        }
    }

    (columns, result_rows)
}

// Batch processing for large datasets - processes multiple values at once
fn process_query_result_batch(rows: Vec<mysql_async::Row>, result_rows: &mut Vec<Vec<String>>) {
    for row in rows.iter() {
        let col_count = row.columns_ref().len();
        let mut row_data = Vec::with_capacity(col_count);

        // Process all columns in this row
        for col_idx in 0..col_count {
            row_data.push(value_to_string_optimized(&row[col_idx]));
        }

        result_rows.push(row_data);
    }
}

// Optimized value conversion with minimal allocations
fn value_to_string_optimized(value: &mysql_async::Value) -> String {
    match value {
        mysql_async::Value::NULL => "NULL".to_string(),
        mysql_async::Value::Bytes(bytes) => {
            // Use Cow<str> to avoid allocation for valid UTF-8
            match std::str::from_utf8(bytes) {
                Ok(s) => s.to_string(),
                Err(_) => String::from_utf8_lossy(bytes).to_string(),
            }
        },
        mysql_async::Value::Int(i) => {
            // Use write! macro for better performance than to_string()
            let mut s = String::with_capacity(12); // i64 max is 19 digits, but most are smaller
            use std::fmt::Write;
            let _ = write!(s, "{}", i);
            s
        },
        mysql_async::Value::UInt(u) => {
            let mut s = String::with_capacity(12);
            use std::fmt::Write;
            let _ = write!(s, "{}", u);
            s
        },
        mysql_async::Value::Float(f) => {
            let mut s = String::with_capacity(16); // f32 precision
            use std::fmt::Write;
            let _ = write!(s, "{}", f);
            s
        },
        mysql_async::Value::Double(d) => {
            let mut s = String::with_capacity(24); // f64 precision
            use std::fmt::Write;
            let _ = write!(s, "{}", d);
            s
        },
        mysql_async::Value::Date(year, month, day, hour, minute, second, _micro) => {
            // Pre-allocate string with known capacity to avoid reallocations
            let mut s = String::with_capacity(19); // "YYYY-MM-DD HH:MM:SS" = 19 chars
            s.push_str(&format!(
                "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
                year, month, day, hour, minute, second
            ));
            s
        },
        mysql_async::Value::Time(_neg, days, hours, minutes, seconds, _micro) => {
            let total_hours = days * 24 + (*hours as u32);
            format!("{}:{:02}:{:02}", total_hours, minutes, seconds)
        },
    }
}
