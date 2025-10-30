use axum::{
    Router, middleware as axum_middleware,
    routing::{delete, get, post, put},
};
use std::sync::Arc;
use tower_http::services::ServeDir;
use tracing_subscriber::{layer::SubscriberExt, util::SubscriberInitExt};
use utoipa::OpenApi;
use utoipa_swagger_ui::SwaggerUi;

mod config;
mod db;
mod handlers;
mod middleware;
mod models;
mod services;
mod utils;

use config::Config;
use services::{
    AuthService, ClusterService, DataStatisticsService, MetricsCollectorService, MySQLPoolManager,
    OverviewService, SystemFunctionService,
};
use sqlx::SqlitePool;
use utils::{JwtUtil, ScheduledExecutor};

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
    pub system_function_service: Arc<SystemFunctionService>,
    pub metrics_collector_service: Arc<MetricsCollectorService>,
    pub data_statistics_service: Arc<DataStatisticsService>,
    pub overview_service: Arc<OverviewService>,
}

#[derive(OpenApi)]
#[openapi(
    paths(
        handlers::auth::register,
        handlers::auth::login,
        handlers::auth::get_me,
        handlers::auth::update_me,
        handlers::cluster::create_cluster,
        handlers::cluster::list_clusters,
        handlers::cluster::get_active_cluster,
        handlers::cluster::get_cluster,
        handlers::cluster::update_cluster,
        handlers::cluster::delete_cluster,
        handlers::cluster::activate_cluster,
        handlers::cluster::get_cluster_health,
        handlers::backend::list_backends,
        handlers::frontend::list_frontends,
        handlers::materialized_view::list_materialized_views,
        handlers::materialized_view::get_materialized_view,
        handlers::materialized_view::get_materialized_view_ddl,
        handlers::materialized_view::create_materialized_view,
        handlers::materialized_view::delete_materialized_view,
        handlers::materialized_view::refresh_materialized_view,
        handlers::materialized_view::cancel_refresh_materialized_view,
        handlers::materialized_view::alter_materialized_view,
        handlers::query::list_catalogs,
        handlers::query::list_databases,
        handlers::query::list_catalogs_with_databases,
        handlers::query::list_queries,
        handlers::query::kill_query,
        handlers::query::execute_sql,
        handlers::query_history::list_query_history,
        handlers::sessions::get_sessions,
        handlers::sessions::kill_session,
        handlers::variables::get_variables,
        handlers::variables::update_variable,
        handlers::profile::list_profiles,
        handlers::profile::get_profile,
        handlers::query_profile::get_query_profile,
        handlers::system_management::get_system_functions,
        handlers::system_management::get_system_function_detail,
        handlers::system::get_runtime_info,
        handlers::overview::get_cluster_overview,
        handlers::overview::get_health_cards,
        handlers::overview::get_performance_trends,
        handlers::overview::get_resource_trends,
        handlers::overview::get_data_statistics,
        handlers::overview::get_capacity_prediction,
        handlers::overview::get_extended_cluster_overview,
        handlers::cluster::test_cluster_connection,
    ),
    components(
        schemas(
            models::User,
            models::UserResponse,
            models::CreateUserRequest,
            models::LoginRequest,
            models::LoginResponse,
            models::Cluster,
            models::ClusterResponse,
            models::CreateClusterRequest,
            models::UpdateClusterRequest,
            models::ClusterHealth,
            models::HealthStatus,
            models::HealthCheck,
            models::Backend,
            models::Frontend,
            models::MaterializedView,
            models::CreateMaterializedViewRequest,
            models::RefreshMaterializedViewRequest,
            models::AlterMaterializedViewRequest,
            models::MaterializedViewDDL,
            models::Query,
            models::QueryExecuteRequest,
            models::QueryExecuteResponse,
            models::CatalogWithDatabases,
            models::CatalogsWithDatabasesResponse,
            models::QueryHistoryItem,
            models::QueryHistoryResponse,
            models::ProfileListItem,
            models::ProfileDetail,
            models::RuntimeInfo,
            models::MetricsSummary,
            models::SystemFunction,
            models::CreateFunctionRequest,
            models::UpdateOrderRequest,
            models::FunctionOrder,
            services::ClusterOverview,
            services::ExtendedClusterOverview,
            services::HealthCard,
            services::HealthStatus,
            services::ClusterHealth,
            services::KeyPerformanceIndicators,
            services::ResourceMetrics,
            services::MaterializedViewStats,
            services::LoadJobStats,
            services::TransactionStats,
            services::SchemaChangeStats,
            services::CompactionStats,
            services::BECompactionScore,
            services::CompactionDetailStats,
            services::TopPartitionByScore,
            services::CompactionTaskStats,
            services::CompactionDurationStats,
            services::SessionStats,
            services::RunningQuery,
            services::NetworkIOStats,
            services::Alert,
            services::AlertLevel,
            services::PerformanceTrends,
            services::ResourceTrends,
            services::MetricsSnapshot,
            services::DataStatistics,
            services::TopTableBySize,
            services::TopTableByAccess,
            services::CapacityPrediction,
        )
    ),
    tags(
        (name = "Authentication", description = "User authentication endpoints"),
        (name = "Clusters", description = "Cluster management endpoints"),
        (name = "Backends", description = "Backend node management"),
        (name = "Frontends", description = "Frontend node management"),
        (name = "Materialized Views", description = "Materialized view management"),
        (name = "Queries", description = "Query management"),
        (name = "Profiles", description = "Query profile management"),
        (name = "System", description = "System information"),
    ),
    modifiers(&SecurityAddon)
)]
struct ApiDoc;

struct SecurityAddon;

impl utoipa::Modify for SecurityAddon {
    fn modify(&self, openapi: &mut utoipa::openapi::OpenApi) {
        let components = openapi.components.as_mut().unwrap();
        components.add_security_scheme(
            "bearer_auth",
            utoipa::openapi::security::SecurityScheme::Http(utoipa::openapi::security::Http::new(
                utoipa::openapi::security::HttpAuthScheme::Bearer,
            )),
        );
    }
}

#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    // Load configuration first
    let config = Config::load()?;

    // Initialize logging
    let log_filter = tracing_subscriber::EnvFilter::new(&config.logging.level);

    let registry = tracing_subscriber::registry().with(log_filter);

    // Add file logging if configured
    if let Some(log_file) = &config.logging.file {
        // Ensure log directory exists
        if let Some(parent) = std::path::Path::new(log_file).parent() {
            let _ = std::fs::create_dir_all(parent);
        }

        let file_appender = tracing_appender::rolling::daily("logs", "starrocks-admin.log");
        let (non_blocking, _guard) = tracing_appender::non_blocking(file_appender);
        registry
            .with(tracing_subscriber::fmt::layer().with_writer(non_blocking))
            .with(tracing_subscriber::fmt::layer())
            .init();
    } else {
        registry.with(tracing_subscriber::fmt::layer()).init();
    }
    tracing::info!("StarRocks Admin starting up");
    tracing::info!("Configuration loaded successfully");

    let pool = db::create_pool(&config.database.url).await?;
    tracing::info!("Database pool created successfully");

    // Initialize core components
    let jwt_util = Arc::new(JwtUtil::new(&config.auth.jwt_secret, &config.auth.jwt_expires_in));
    let mysql_pool_manager = Arc::new(MySQLPoolManager::new());

    let auth_service = Arc::new(AuthService::new(pool.clone(), Arc::clone(&jwt_util)));

    let cluster_service = Arc::new(ClusterService::new(pool.clone()));

    let system_function_service = Arc::new(SystemFunctionService::new(
        Arc::new(pool.clone()),
        Arc::clone(&mysql_pool_manager),
        Arc::clone(&cluster_service),
    ));

    // Create new services for cluster overview
    let metrics_collector_service = Arc::new(MetricsCollectorService::new(
        pool.clone(),
        Arc::clone(&cluster_service),
        Arc::clone(&mysql_pool_manager),
        config.metrics.retention_days,
    ));

    let data_statistics_service = Arc::new(DataStatisticsService::new(
        pool.clone(),
        Arc::clone(&cluster_service),
        Arc::clone(&mysql_pool_manager),
    ));

    let overview_service = Arc::new(
        OverviewService::new(
            pool.clone(),
            Arc::clone(&cluster_service),
            Arc::clone(&mysql_pool_manager),
        )
        .with_data_statistics(Arc::clone(&data_statistics_service)),
    );

    // Build AppState with all services
    let app_state = AppState {
        db: pool.clone(),
        mysql_pool_manager: Arc::clone(&mysql_pool_manager),
        jwt_util: Arc::clone(&jwt_util),
        auth_service: Arc::clone(&auth_service),
        cluster_service: Arc::clone(&cluster_service),
        system_function_service: Arc::clone(&system_function_service),
        metrics_collector_service: Arc::clone(&metrics_collector_service),
        data_statistics_service: Arc::clone(&data_statistics_service),
        overview_service: Arc::clone(&overview_service),
    };

    // Start metrics collector using ScheduledExecutor (configurable interval)
    if config.metrics.enabled {
        let interval = std::time::Duration::from_secs(config.metrics.interval_secs);
        tracing::info!(
            "Starting metrics collector with interval: {}s (retention_days={})",
            config.metrics.interval_secs,
            config.metrics.retention_days
        );
        let executor = ScheduledExecutor::new("metrics-collector", interval);
        executor.spawn(Arc::clone(&metrics_collector_service));
    } else {
        tracing::warn!("Metrics collector disabled by configuration");
    }

    // Wrap AppState in Arc for shared ownership across routes
    let app_state_arc = Arc::new(app_state);

    // Auth state for middleware
    let auth_state = middleware::AuthState { jwt_util: Arc::clone(&jwt_util) };

    // Public routes (no authentication required)
    let public_routes = Router::new()
        .route("/api/auth/register", post(handlers::auth::register))
        .route("/api/auth/login", post(handlers::auth::login))
        .with_state(Arc::clone(&app_state_arc));

    // Protected routes (require authentication)
    let protected_routes = Router::new()
        // Auth
        .route("/api/auth/me", get(handlers::auth::get_me))
        .route("/api/auth/me", put(handlers::auth::update_me))
        // Clusters
        .route("/api/clusters", post(handlers::cluster::create_cluster))
        .route("/api/clusters", get(handlers::cluster::list_clusters))
        .route("/api/clusters/active", get(handlers::cluster::get_active_cluster))
        .route("/api/clusters/:id", get(handlers::cluster::get_cluster))
        .route("/api/clusters/:id", put(handlers::cluster::update_cluster))
        .route("/api/clusters/:id", delete(handlers::cluster::delete_cluster))
        .route("/api/clusters/:id/activate", put(handlers::cluster::activate_cluster))
        .route("/api/clusters/health/test", post(handlers::cluster::test_cluster_connection))
        .route(
            "/api/clusters/:id/health",
            get(handlers::cluster::get_cluster_health).post(handlers::cluster::get_cluster_health),
        )
        // Backends
        .route("/api/clusters/backends", get(handlers::backend::list_backends))
        .route("/api/clusters/backends/:host/:port", delete(handlers::backend::delete_backend))
        // Frontends
        .route("/api/clusters/frontends", get(handlers::frontend::list_frontends))
        // Queries
        .route("/api/clusters/catalogs", get(handlers::query::list_catalogs))
        .route("/api/clusters/databases", get(handlers::query::list_databases))
        .route("/api/clusters/catalogs-databases", get(handlers::query::list_catalogs_with_databases))
        .route("/api/clusters/queries", get(handlers::query::list_queries))
        .route("/api/clusters/queries/execute", post(handlers::query::execute_sql))
        .route("/api/clusters/queries/:query_id", delete(handlers::query::kill_query))
        .route("/api/clusters/queries/history", get(handlers::query_history::list_query_history))
        .route(
            "/api/clusters/queries/:query_id/profile",
            get(handlers::query_profile::get_query_profile),
        )
        // Materialized Views
        .route(
            "/api/clusters/materialized_views",
            get(handlers::materialized_view::list_materialized_views)
                .post(handlers::materialized_view::create_materialized_view),
        )
        .route(
            "/api/clusters/materialized_views/:mv_name",
            get(handlers::materialized_view::get_materialized_view)
                .delete(handlers::materialized_view::delete_materialized_view)
                .put(handlers::materialized_view::alter_materialized_view),
        )
        .route(
            "/api/clusters/materialized_views/:mv_name/ddl",
            get(handlers::materialized_view::get_materialized_view_ddl),
        )
        .route(
            "/api/clusters/materialized_views/:mv_name/refresh",
            post(handlers::materialized_view::refresh_materialized_view),
        )
        .route(
            "/api/clusters/materialized_views/:mv_name/cancel",
            post(handlers::materialized_view::cancel_refresh_materialized_view),
        )
        // Profiles
        .route("/api/clusters/profiles", get(handlers::profile::list_profiles))
        .route("/api/clusters/profiles/:query_id", get(handlers::profile::get_profile))
        // Sessions
        .route("/api/clusters/sessions", get(handlers::sessions::get_sessions))
        .route("/api/clusters/sessions/:session_id", delete(handlers::sessions::kill_session))
        // Variables
        .route("/api/clusters/variables", get(handlers::variables::get_variables))
        .route("/api/clusters/variables/:variable_name", put(handlers::variables::update_variable))
        // System
        .route("/api/clusters/system/runtime_info", get(handlers::system::get_runtime_info))
        .route("/api/clusters/system", get(handlers::system_management::get_system_functions))
        .route(
            "/api/clusters/system/:function_name",
            get(handlers::system_management::get_system_function_detail),
        )
        // System Functions
        .route(
            "/api/clusters/system-functions",
            get(handlers::system_function::get_system_functions)
                .post(handlers::system_function::create_system_function),
        )
        .route(
            "/api/clusters/system-functions/orders",
            put(handlers::system_function::update_function_orders),
        )
        .route(
            "/api/clusters/system-functions/:function_id/execute",
            post(handlers::system_function::execute_system_function),
        )
        .route(
            "/api/clusters/system-functions/:function_id/favorite",
            put(handlers::system_function::toggle_function_favorite),
        )
        .route(
            "/api/clusters/system-functions/:function_id",
            put(handlers::system_function::update_function)
                .delete(handlers::system_function::delete_system_function),
        )
        .route(
            "/api/system-functions/:function_name/access-time",
            put(handlers::system_function::update_system_function_access_time),
        )
        .route(
            "/api/system-functions/category/:category_name",
            delete(handlers::system_function::delete_category),
        )
        // Overview
        .route("/api/clusters/overview", get(handlers::overview::get_cluster_overview))
        .route(
            "/api/clusters/overview/extended",
            get(handlers::overview::get_extended_cluster_overview),
        )
        .route("/api/clusters/overview/health", get(handlers::overview::get_health_cards))
        .route(
            "/api/clusters/overview/performance",
            get(handlers::overview::get_performance_trends),
        )
        .route("/api/clusters/overview/resources", get(handlers::overview::get_resource_trends))
        .route("/api/clusters/overview/data-stats", get(handlers::overview::get_data_statistics))
        .route(
            "/api/clusters/overview/capacity-prediction",
            get(handlers::overview::get_capacity_prediction),
        )
        .route(
            "/api/clusters/overview/compaction-details",
            get(handlers::overview::get_compaction_detail_stats),
        )
        .with_state(Arc::clone(&app_state_arc))
        .layer(axum_middleware::from_fn_with_state(auth_state, middleware::auth_middleware));

    let health_routes = Router::new()
        .route("/health", get(health_check))
        .route("/ready", get(ready_check));

    // Static file serving (if enabled)
    let static_routes = if config.static_config.enabled {
        tracing::info!(
            "Static file serving enabled, serving from: {}",
            config.static_config.web_root
        );
        Router::new().nest_service("/", ServeDir::new(&config.static_config.web_root))
    } else {
        Router::new()
    };

    let app = Router::new()
        .merge(SwaggerUi::new("/api-docs").url("/api-docs/openapi.json", ApiDoc::openapi()))
        .merge(public_routes)
        .merge(protected_routes)
        .merge(health_routes)
        .merge(static_routes)
        .layer(tower_http::trace::TraceLayer::new_for_http())
        .layer(tower_http::cors::CorsLayer::permissive());

    let addr = format!("{}:{}", config.server.host, config.server.port);
    let listener = tokio::net::TcpListener::bind(&addr).await?;

    tracing::info!("Server listening on http://{}", addr);
    tracing::info!("API documentation available at http://{}/api-docs", addr);
    tracing::info!("StarRocks Admin is ready to serve requests");

    axum::serve(listener, app).await?;

    Ok(())
}

async fn health_check() -> &'static str {
    "OK"
}

async fn ready_check() -> &'static str {
    "READY"
}
