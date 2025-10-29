use serde_json::Value;
use sqlx::SqlitePool;
use std::collections::HashMap;
use std::sync::Arc;

use crate::models::{
    CreateFunctionRequest, SystemFunction, SystemFunctionPreference, UpdateFunctionRequest,
    UpdateOrderRequest,
};
use crate::services::{ClusterService, MySQLClient, MySQLPoolManager};
use crate::utils::{ApiError, ApiResult};

#[derive(Clone)]
pub struct SystemFunctionService {
    db: Arc<SqlitePool>,
    mysql_pool_manager: Arc<MySQLPoolManager>,
    cluster_service: Arc<ClusterService>,
}

impl SystemFunctionService {
    pub fn new(
        db: Arc<SqlitePool>,
        mysql_pool_manager: Arc<MySQLPoolManager>,
        cluster_service: Arc<ClusterService>,
    ) -> Self {
        Self { db, mysql_pool_manager, cluster_service }
    }

    pub async fn get_functions(&self, cluster_id: i64) -> ApiResult<Vec<SystemFunction>> {
        tracing::debug!("Getting system functions for cluster_id: {}", cluster_id);

        let all_functions = sqlx::query_as::<_, SystemFunction>(
            "SELECT * FROM system_functions WHERE cluster_id IS NULL OR cluster_id = ?",
        )
        .bind(cluster_id)
        .fetch_all(&*self.db)
        .await?;

        tracing::debug!("Found {} function definitions", all_functions.len());

        let preferences = sqlx::query_as::<_, SystemFunctionPreference>(
            "SELECT * FROM system_function_preferences WHERE cluster_id = ?",
        )
        .bind(cluster_id)
        .fetch_all(&*self.db)
        .await?;

        tracing::debug!("Found {} preference settings", preferences.len());

        let preference_map: HashMap<i64, SystemFunctionPreference> = preferences
            .into_iter()
            .map(|p| (p.function_id, p))
            .collect();

        let mut merged_functions: Vec<SystemFunction> = all_functions
            .into_iter()
            .map(|mut func| {
                if let Some(pref) = preference_map.get(&func.id) {
                    func.category_order = pref.category_order;
                    func.display_order = pref.display_order;
                    func.is_favorited = pref.is_favorited;
                }
                if func.cluster_id == 0 {
                    func.cluster_id = cluster_id;
                }
                func
            })
            .collect();
        merged_functions.sort_by(|a, b| {
            a.category_order
                .cmp(&b.category_order)
                .then(a.display_order.cmp(&b.display_order))
        });

        tracing::debug!(
            "Returning {} merged functions for cluster_id: {}",
            merged_functions.len(),
            cluster_id
        );
        Ok(merged_functions)
    }

    pub async fn create_function(
        &self,
        cluster_id: i64,
        req: CreateFunctionRequest,
        user_id: i64,
    ) -> ApiResult<SystemFunction> {
        tracing::info!(
            "Creating system function: {} for cluster_id: {} by user_id: {}",
            req.function_name,
            cluster_id,
            user_id
        );

        // Trim input fields
        let category_name = req.category_name.trim().to_string();
        let function_name = req.function_name.trim().to_string();
        let description = req.description.trim().to_string();
        let sql_query = req.sql_query.trim().to_string();

        // Check if fields are empty after trimming
        if category_name.is_empty() {
            return Err(ApiError::validation_error("Category name cannot be empty"));
        }
        if function_name.is_empty() {
            return Err(ApiError::validation_error("Function name cannot be empty"));
        }
        if description.is_empty() {
            return Err(ApiError::validation_error("Function description cannot be empty"));
        }
        if sql_query.is_empty() {
            return Err(ApiError::validation_error("SQL query cannot be empty"));
        }

        // Validate SQL safety
        self.validate_sql_safety(&sql_query)?;

        // Check function count limit per category (4 functions)
        let count: i64 = sqlx::query_scalar(
            "SELECT COUNT(*) FROM system_functions WHERE cluster_id = ? AND category_name = ?",
        )
        .bind(cluster_id)
        .bind(&category_name)
        .fetch_one(&*self.db)
        .await?;

        if count >= 4 {
            return Err(ApiError::category_full(
                "This category already has 4 functions, cannot add more",
            ));
        }

        // Get the maximum display order for current category
        let max_order: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(display_order) FROM system_functions WHERE cluster_id = ? AND category_name = ?"
        )
        .bind(cluster_id)
        .bind(&category_name)
        .fetch_optional(&*self.db)
        .await?;

        let display_order = max_order.unwrap_or(0) + 1;

        // Get the maximum category order for current cluster
        let max_category_order: Option<i32> = sqlx::query_scalar(
            "SELECT MAX(category_order) FROM system_functions WHERE cluster_id = ?",
        )
        .bind(cluster_id)
        .fetch_optional(&*self.db)
        .await?;

        let category_order = max_category_order.unwrap_or(0) + 1;

        let function_id = sqlx::query_scalar::<_, i64>(
            "INSERT INTO system_functions (
                cluster_id, category_name, function_name, description, sql_query,
                display_order, category_order, is_favorited, created_by
            ) VALUES (?, ?, ?, ?, ?, ?, ?, 0, ?) RETURNING id",
        )
        .bind(cluster_id)
        .bind(&category_name)
        .bind(&function_name)
        .bind(&description)
        .bind(&sql_query)
        .bind(display_order)
        .bind(category_order)
        .bind(user_id)
        .fetch_one(&*self.db)
        .await?;

        // Return the created function
        let function =
            sqlx::query_as::<_, SystemFunction>("SELECT * FROM system_functions WHERE id = ?")
                .bind(function_id)
                .fetch_one(&*self.db)
                .await?;

        Ok(function)
    }

    // Execute custom function SQL
    pub async fn execute_function(
        &self,
        cluster_id: i64,
        function_id: i64,
    ) -> ApiResult<Vec<HashMap<String, Value>>> {
        // Get function information
        let function = sqlx::query_as::<_, SystemFunction>(
            "SELECT * FROM system_functions WHERE id = ? AND cluster_id = ?",
        )
        .bind(function_id)
        .bind(cluster_id)
        .fetch_optional(&*self.db)
        .await?;

        let function = match function {
            Some(f) => f,
            None => return Err(ApiError::not_found("Function not found or deleted")),
        };

        // Update function updated_at timestamp
        sqlx::query("UPDATE system_functions SET updated_at = CURRENT_TIMESTAMP WHERE id = ?")
            .bind(function_id)
            .execute(&*self.db)
            .await?;

        // Get cluster information
        let cluster = self.cluster_service.get_cluster(cluster_id).await?;

        // Execute SQL query using MySQL client
        let pool = self.mysql_pool_manager.get_pool(&cluster).await?;
        let mysql_client = MySQLClient::from_pool(pool);

        let (columns, rows) = mysql_client.query_raw(&function.sql_query).await?;

        // Convert to HashMap format
        let result: Vec<HashMap<String, Value>> = rows
            .into_iter()
            .map(|row| {
                columns
                    .iter()
                    .zip(row.iter())
                    .map(|(col, val)| (col.clone(), Value::String(val.clone())))
                    .collect()
            })
            .collect();

        Ok(result)
    }

    // Update system function access time
    pub async fn update_system_function_access_time(&self, function_name: &str) -> ApiResult<()> {
        sqlx::query(
            "UPDATE system_functions SET updated_at = CURRENT_TIMESTAMP WHERE function_name = ? AND cluster_id IS NULL"
        )
        .bind(function_name)
        .execute(&*self.db)
        .await?;

        Ok(())
    }

    // Update display and category orders
    pub async fn update_orders(&self, cluster_id: i64, req: UpdateOrderRequest) -> ApiResult<()> {
        let mut tx = self.db.begin().await?;

        for order in req.functions {
            // Unified handling: all function ordering managed through preferences table
            // Use UPSERT syntax (SQLite 3.24.0+)
            sqlx::query(
                "INSERT INTO system_function_preferences (cluster_id, function_id, category_order, display_order, is_favorited, updated_at)
                 VALUES (?, ?, ?, ?, COALESCE((SELECT is_favorited FROM system_function_preferences WHERE cluster_id = ? AND function_id = ?), false), CURRENT_TIMESTAMP)
                 ON CONFLICT(cluster_id, function_id) DO UPDATE SET
                 category_order = excluded.category_order,
                 display_order = excluded.display_order,
                 updated_at = CURRENT_TIMESTAMP"
            )
            .bind(cluster_id)
            .bind(order.id)
            .bind(order.category_order)
            .bind(order.display_order)
            .bind(cluster_id)
            .bind(order.id)
            .execute(&mut *tx)
            .await?;
        }

        tx.commit().await?;
        Ok(())
    }

    // Toggle favorite status
    pub async fn toggle_favorite(
        &self,
        cluster_id: i64,
        function_id: i64,
    ) -> ApiResult<SystemFunction> {
        // Unified handling: all function favorites managed through preferences table

        // Query current favorite status
        let current_favorited: Option<bool> = sqlx::query_scalar(
            "SELECT is_favorited FROM system_function_preferences WHERE cluster_id = ? AND function_id = ?"
        )
        .bind(cluster_id)
        .bind(function_id)
        .fetch_optional(&*self.db)
        .await?;

        let new_favorited = !current_favorited.unwrap_or(false);

        // Get current function ordering info (use defaults if preference doesn't exist)
        let (default_category_order, default_display_order): (i32, i32) = sqlx::query_as(
            "SELECT category_order, display_order FROM system_functions WHERE id = ?",
        )
        .bind(function_id)
        .fetch_one(&*self.db)
        .await?;

        // UPSERT operation
        sqlx::query(
            "INSERT INTO system_function_preferences (cluster_id, function_id, category_order, display_order, is_favorited, updated_at)
             VALUES (?, ?, 
                     COALESCE((SELECT category_order FROM system_function_preferences WHERE cluster_id = ? AND function_id = ?), ?),
                     COALESCE((SELECT display_order FROM system_function_preferences WHERE cluster_id = ? AND function_id = ?), ?),
                     ?, CURRENT_TIMESTAMP)
             ON CONFLICT(cluster_id, function_id) DO UPDATE SET
             is_favorited = excluded.is_favorited,
             updated_at = CURRENT_TIMESTAMP"
        )
        .bind(cluster_id)
        .bind(function_id)
        .bind(cluster_id)
        .bind(function_id)
        .bind(default_category_order)
        .bind(cluster_id)
        .bind(function_id)
        .bind(default_display_order)
        .bind(new_favorited)
        .execute(&*self.db)
        .await?;

        // Return updated function
        let functions = self.get_functions(cluster_id).await?;
        functions
            .into_iter()
            .find(|f| f.id == function_id)
            .ok_or_else(|| ApiError::not_found("Function not found or deleted"))
    }

    // Update custom function
    pub async fn update_function(
        &self,
        cluster_id: i64,
        function_id: i64,
        req: UpdateFunctionRequest,
    ) -> ApiResult<SystemFunction> {
        // Trim input fields
        let category_name = req.category_name.trim().to_string();
        let function_name = req.function_name.trim().to_string();
        let description = req.description.trim().to_string();
        let sql_query = req.sql_query.trim().to_string();

        // Check if fields are empty after trimming
        if category_name.is_empty() {
            return Err(ApiError::validation_error("Category name cannot be empty"));
        }
        if function_name.is_empty() {
            return Err(ApiError::validation_error("Function name cannot be empty"));
        }
        if description.is_empty() {
            return Err(ApiError::validation_error("Function description cannot be empty"));
        }
        if sql_query.is_empty() {
            return Err(ApiError::validation_error("SQL query cannot be empty"));
        }

        // Validate SQL safety
        self.validate_sql_safety(&sql_query)?;

        // 更新功能
        sqlx::query(
            "UPDATE system_functions SET 
             category_name = ?, function_name = ?, description = ?, sql_query = ?, updated_at = CURRENT_TIMESTAMP
             WHERE id = ? AND cluster_id = ?"
        )
        .bind(category_name)
        .bind(function_name)
        .bind(description)
        .bind(sql_query)
        .bind(function_id)
        .bind(cluster_id)
        .execute(&*self.db)
        .await?;

        // Return updated function
        let functions = self.get_functions(cluster_id).await?;
        functions
            .into_iter()
            .find(|f| f.id == function_id)
            .ok_or_else(|| ApiError::not_found("Function not found or deleted"))
    }

    // Delete custom function
    pub async fn delete_function(&self, cluster_id: i64, function_id: i64) -> ApiResult<()> {
        let result = sqlx::query("DELETE FROM system_functions WHERE id = ? AND cluster_id = ?")
            .bind(function_id)
            .bind(cluster_id)
            .execute(&*self.db)
            .await?;

        if result.rows_affected() == 0 {
            return Err(ApiError::not_found("Function not found or deleted"));
        }

        Ok(())
    }

    // Validate SQL safety (only allow SELECT/SHOW)
    fn validate_sql_safety(&self, sql: &str) -> ApiResult<()> {
        let trimmed_sql = sql.trim().to_uppercase();

        // Check if starts with SELECT or SHOW
        if !trimmed_sql.starts_with("SELECT") && !trimmed_sql.starts_with("SHOW") {
            return Err(ApiError::invalid_sql("Only SELECT and SHOW type SQL queries are allowed"));
        }

        // Check for dangerous keywords
        let dangerous_keywords = vec![
            "DROP", "DELETE", "UPDATE", "INSERT", "ALTER", "CREATE", "TRUNCATE", "EXEC", "EXECUTE",
            "CALL", "GRANT", "REVOKE", "COMMIT", "ROLLBACK",
        ];

        for keyword in dangerous_keywords {
            if trimmed_sql.contains(&format!(" {}", keyword))
                || trimmed_sql.contains(&format!("{} ", keyword))
            {
                return Err(ApiError::sql_safety_violation(format!(
                    "SQL查询包含不允许的关键字：{}",
                    keyword
                )));
            }
        }

        Ok(())
    }

    // Delete category
    pub async fn delete_category(&self, category_name: &str) -> ApiResult<()> {
        // Check if it's a system category
        let system_categories = [
            "集群信息",
            "数据库管理",
            "事务管理",
            "任务管理",
            "元数据管理",
            "存储管理",
            "作业管理",
        ];

        if system_categories.contains(&category_name) {
            return Err(ApiError::invalid_data("不能删除系统默认分类"));
        }

        // Delete all custom functions in this category
        sqlx::query(
            "DELETE FROM system_functions WHERE category_name = ? AND cluster_id IS NOT NULL",
        )
        .bind(category_name)
        .execute(&*self.db)
        .await?;

        // Delete preferences for this category
        sqlx::query(
            "DELETE FROM system_function_preferences WHERE function_id IN (
                SELECT id FROM system_functions WHERE category_name = ? AND cluster_id IS NOT NULL
            )",
        )
        .bind(category_name)
        .execute(&*self.db)
        .await?;

        Ok(())
    }
}
