use serde::Deserialize;
use std::fs;
use std::path::Path;

#[derive(Debug, Clone, Deserialize, Default)]
#[serde(default)]
pub struct Config {
    pub server: ServerConfig,
    pub database: DatabaseConfig,
    pub auth: AuthConfig,
    pub logging: LoggingConfig,
    pub static_config: StaticConfig,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct ServerConfig {
    pub host: String,
    pub port: u16,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct DatabaseConfig {
    pub url: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct AuthConfig {
    pub jwt_secret: String,
    pub jwt_expires_in: String,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct LoggingConfig {
    pub level: String,
    pub file: Option<String>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct StaticConfig {
    pub enabled: bool,
    pub web_root: String,
}

impl Config {
    /// Load configuration with environment variable override support
    ///
    /// Loading order:
    /// 1. Load from config.toml file
    /// 2. Override with environment variables (prefixed with APP_)
    /// 3. Validate the final configuration
    pub fn load() -> Result<Self, anyhow::Error> {
        // 1. Load from config file
        let mut config = if let Some(config_path) = Self::find_config_file() {
            Self::from_toml(&config_path)?
        } else {
            tracing::warn!("Configuration file not found, using defaults");
            Config::default()
        };

        // 2. Override with environment variables
        config.apply_env_overrides();

        // 3. Validate configuration
        config.validate()?;

        Ok(config)
    }

    /// Apply environment variable overrides
    ///
    /// Supported environment variables:
    /// - APP_SERVER_HOST: Server host (default: 0.0.0.0)
    /// - APP_SERVER_PORT: Server port (default: 8080)
    /// - APP_DATABASE_URL: Database URL (default: sqlite://data/starrocks-admin.db)
    /// - APP_JWT_SECRET: JWT secret key
    /// - APP_JWT_EXPIRES_IN: JWT expiration time (e.g., "24h")
    /// - APP_LOG_LEVEL: Logging level (e.g., "info,starrocks_admin_backend=debug")
    fn apply_env_overrides(&mut self) {
        if let Ok(host) = std::env::var("APP_SERVER_HOST") {
            self.server.host = host;
            tracing::info!("Override server.host from env: {}", self.server.host);
        }

        if let Ok(port_str) = std::env::var("APP_SERVER_PORT")
            && let Ok(port) = port_str.parse::<u16>()
        {
            self.server.port = port;
            tracing::info!("Override server.port from env: {}", self.server.port);
        }

        if let Ok(db_url) = std::env::var("APP_DATABASE_URL") {
            self.database.url = db_url;
            tracing::info!("Override database.url from env");
        }

        if let Ok(secret) = std::env::var("APP_JWT_SECRET") {
            self.auth.jwt_secret = secret;
            tracing::info!("Override auth.jwt_secret from env");
        }

        if let Ok(expires) = std::env::var("APP_JWT_EXPIRES_IN") {
            self.auth.jwt_expires_in = expires;
            tracing::info!("Override auth.jwt_expires_in from env: {}", self.auth.jwt_expires_in);
        }

        if let Ok(level) = std::env::var("APP_LOG_LEVEL") {
            self.logging.level = level;
            tracing::info!("Override logging.level from env: {}", self.logging.level);
        }
    }

    /// Validate configuration
    fn validate(&self) -> Result<(), anyhow::Error> {
        // Warn if using default JWT secret in production
        if self.auth.jwt_secret == "dev-secret-key-change-in-production" {
            tracing::warn!("⚠️  WARNING: Using default JWT secret!");
            tracing::warn!(
                "⚠️  Please set APP_JWT_SECRET environment variable or update config.toml"
            );
            tracing::warn!("⚠️  This is INSECURE for production use!");
        }

        // Validate server port
        if self.server.port == 0 {
            anyhow::bail!("Server port cannot be 0");
        }

        // Validate database URL
        if self.database.url.is_empty() {
            anyhow::bail!("Database URL cannot be empty");
        }

        Ok(())
    }

    fn find_config_file() -> Option<String> {
        // Get environment (dev, prod, etc.)
        let env = std::env::var("APP_ENV")
            .or_else(|_| std::env::var("ENVIRONMENT"))
            .unwrap_or_else(|_| "dev".to_string());

        // Try environment-specific config first
        let env_specific_paths = [
            format!("conf/{}/config.toml", env),
            format!("./conf/{}/config.toml", env),
            format!("backend/conf/{}/config.toml", env),
            format!("../conf/{}/config.toml", env),
        ];

        for path in &env_specific_paths {
            if Path::new(path).exists() {
                tracing::info!("Using environment-specific config: {}", path);
                return Some(path.to_string());
            }
        }

        // Fall back to generic config paths
        let possible_paths = [
            "conf/config.toml",
            "config.toml",
            "./conf/config.toml",
            "./config.toml",
            "backend/conf/config.toml",
        ];

        for path in &possible_paths {
            if Path::new(path).exists() {
                tracing::info!("Using generic config: {}", path);
                return Some(path.to_string());
            }
        }
        None
    }

    fn from_toml(path: &str) -> Result<Self, anyhow::Error> {
        let content = fs::read_to_string(path)?;
        let config: Config = toml::from_str(&content)?;
        Ok(config)
    }
}

impl Default for ServerConfig {
    fn default() -> Self {
        Self { host: "0.0.0.0".to_string(), port: 8080 }
    }
}

impl Default for DatabaseConfig {
    fn default() -> Self {
        Self { url: "sqlite://tmp/starrocks-admin.db".to_string() }
    }
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: "dev-secret-key-change-in-production".to_string(),
            jwt_expires_in: "24h".to_string(),
        }
    }
}

impl Default for LoggingConfig {
    fn default() -> Self {
        Self {
            level: "info,starrocks_admin_backend=debug".to_string(),
            file: Some("logs/starrocks-admin.log".to_string()),
        }
    }
}

impl Default for StaticConfig {
    fn default() -> Self {
        Self { enabled: true, web_root: "web".to_string() }
    }
}
