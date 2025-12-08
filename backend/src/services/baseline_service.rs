//! Baseline Service
//!
//! This service retrieves audit log data from StarRocks and calculates
//! historical performance baselines for adaptive threshold determination.

use crate::services::mysql_client::MySQLClient;
use crate::services::profile_analyzer::analyzer::{
    BaselineCalculator, PerformanceBaseline, QueryComplexity, AuditLogRecord,
};
use std::collections::HashMap;

/// Baseline service for historical performance analysis
pub struct BaselineService {
    calculator: BaselineCalculator,
}

impl BaselineService {
    pub fn new() -> Self {
        Self {
            calculator: BaselineCalculator::new(),
        }
    }
    
    /// Fetch audit logs from StarRocks and calculate baselines
    ///
    /// # Arguments
    /// * `mysql` - MySQL client connected to StarRocks
    /// * `hours_back` - How many hours of history to fetch (default: 168 = 7 days)
    ///
    /// # Returns
    /// HashMap of QueryComplexity -> PerformanceBaseline
    pub async fn calculate_baselines(
        &self,
        mysql: &MySQLClient,
        hours_back: u32,
    ) -> Result<HashMap<QueryComplexity, PerformanceBaseline>, String> {
        // Fetch audit logs from last N hours
        let audit_records = self.fetch_audit_logs(mysql, hours_back).await?;
        
        // Calculate baselines grouped by complexity
        let baselines = self.calculator.calculate_by_complexity(&audit_records);
        
        Ok(baselines)
    }
    
    /// Calculate baseline for a specific table
    pub async fn calculate_table_baseline(
        &self,
        mysql: &MySQLClient,
        table_name: &str,
        hours_back: u32,
    ) -> Result<Option<PerformanceBaseline>, String> {
        let audit_records = self.fetch_audit_logs(mysql, hours_back).await?;
        let baseline = self.calculator.calculate_for_table(&audit_records, table_name);
        Ok(baseline)
    }
    
    /// Fetch audit log records from StarRocks
    async fn fetch_audit_logs(
        &self,
        mysql: &MySQLClient,
        hours_back: u32,
    ) -> Result<Vec<AuditLogRecord>, String> {
        let sql = format!(
            r#"
            SELECT 
                queryId,
                COALESCE(`user`, '') AS user,
                COALESCE(`db`, '') AS db,
                stmt,
                COALESCE(queryType, 'Query') AS queryType,
                queryTime AS query_time_ms,
                COALESCE(`state`, '') AS state,
                `timestamp`
            FROM starrocks_audit_db__.starrocks_audit_tbl__
            WHERE 
                isQuery = 1
                AND `timestamp` >= DATE_SUB(NOW(), INTERVAL {} HOUR)
                AND `state` IN ('EOF', 'OK')  -- Only successful queries
                AND queryTime > 0  -- Filter out invalid times
            ORDER BY `timestamp` DESC
            LIMIT 10000  -- Limit to avoid memory issues
            "#,
            hours_back
        );
        
        let (columns, rows) = mysql.query_raw(&sql).await
            .map_err(|e| format!("Failed to fetch audit logs: {:?}", e))?;
        
        // Build column index map
        let mut col_idx = HashMap::new();
        for (i, col) in columns.iter().enumerate() {
            col_idx.insert(col.clone(), i);
        }
        
        let mut records = Vec::with_capacity(rows.len());
        for row in &rows {
            let query_id = col_idx.get("queryId")
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_default();
            
            let user = col_idx.get("user")
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_default();
            
            let db = col_idx.get("db")
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_default();
            
            let stmt = col_idx.get("stmt")
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_default();
            
            let query_type = col_idx.get("queryType")
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_else(|| "Query".to_string());
            
            let query_time_ms = col_idx.get("query_time_ms")
                .and_then(|&i| row.get(i))
                .and_then(|s| s.parse::<i64>().ok())
                .unwrap_or(0);
            
            let state = col_idx.get("state")
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_default();
            
            let timestamp = col_idx.get("timestamp")
                .and_then(|&i| row.get(i))
                .cloned()
                .unwrap_or_default();
            
            records.push(AuditLogRecord {
                query_id,
                user,
                db,
                stmt,
                query_type,
                query_time_ms,
                state,
                timestamp,
            });
        }
        
        Ok(records)
    }
}

impl Default for BaselineService {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    
    #[test]
    fn test_baseline_service_creation() {
        let service = BaselineService::new();
        assert!(service.calculator.min_sample_size >= 1);
    }
}
