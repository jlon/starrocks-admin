//! StarRocks Admin Library
//!
//! This library contains all the core modules for the StarRocks Admin application.

use std::sync::Arc;
use sqlx::SqlitePool;

pub mod config;
pub mod db;
pub mod embedded;
pub mod handlers;
pub mod middleware;
pub mod models;
pub mod services;
pub mod utils;

// Re-export commonly used types
pub use config::Config;
pub use services::{
    AuthService, CasbinService, ClusterService, DataStatisticsService, MetricsCollectorService,
    MySQLPoolManager, OrganizationService, OverviewService, PermissionService, RoleService,
    SystemFunctionService, UserRoleService, UserService,
};
pub use services::llm::{LLMService, LLMServiceImpl, LLMError, LLMProviderInfo};
pub use utils::JwtUtil;

/// Application shared state
///
/// Design Philosophy: Keep it simple - Rust's type system IS our DI container.
/// No need for Service Container pattern with dyn Any.
/// All services are wrapped in Arc for cheap cloning and thread safety.
#[derive(Clone)]
pub struct AppState {
    // Core dependencies
    pub db: SqlitePool,

    // Managers
    pub mysql_pool_manager: Arc<MySQLPoolManager>,
    pub jwt_util: Arc<JwtUtil>,

    // Services (grouped by domain)
    pub auth_service: Arc<AuthService>,
    pub cluster_service: Arc<ClusterService>,
    pub organization_service: Arc<OrganizationService>,
    pub system_function_service: Arc<SystemFunctionService>,
    pub metrics_collector_service: Arc<MetricsCollectorService>,
    pub data_statistics_service: Arc<DataStatisticsService>,
    pub overview_service: Arc<OverviewService>,

    // RBAC Services
    pub casbin_service: Arc<CasbinService>,
    pub permission_service: Arc<PermissionService>,
    pub role_service: Arc<RoleService>,
    pub user_role_service: Arc<UserRoleService>,
    pub user_service: Arc<UserService>,

    // LLM Service
    pub llm_service: Arc<LLMServiceImpl>,
}
