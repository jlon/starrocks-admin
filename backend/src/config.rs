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
    pub metrics: MetricsCollectorConfig,
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

// New: metrics collector configuration section (loaded from conf/config.toml)
#[derive(Debug, Clone, Deserialize)]
#[serde(default)]
pub struct MetricsCollectorConfig {
    /// Metrics collection interval in seconds (default: 30)
    #[serde(deserialize_with = "deserialize_duration_secs")]
    pub interval_secs: u64,
    /// Historical metrics retention days (default: 7)
    #[serde(deserialize_with = "deserialize_days_i64")]
    pub retention_days: i64,
    /// Whether to enable the metrics collector at startup (default: true)
    pub enabled: bool,
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
    /// - APP_METRICS_INTERVAL_SECS: Metrics collection interval in seconds (accepts "30s", "5m", "1h")
    /// - APP_METRICS_RETENTION_DAYS: Retention days for metrics (accepts "7d")
    /// - APP_METRICS_ENABLED: Enable/disable metrics collector (true/false)
    fn apply_env_overrides(&mut self) {
        if let Ok(host) = std::env::var("APP_SERVER_HOST") {
            self.server.host = host;
            tracing::info!("Override server.host from env: {}", self.server.host);
        }

        if let Ok(port) = std::env::var("APP_SERVER_PORT")
            && let Ok(port) = port.parse()
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

        // Metrics collector overrides
        if let Ok(interval) = std::env::var("APP_METRICS_INTERVAL_SECS") {
            match parse_duration_to_secs(&interval) {
                Ok(val) => {
                    self.metrics.interval_secs = val;
                    tracing::info!(
                        "Override metrics.interval_secs from env: {}",
                        self.metrics.interval_secs
                    );
                },
                Err(e) => tracing::warn!(
                    "Invalid APP_METRICS_INTERVAL_SECS '{}': {} (keep {})",
                    interval,
                    e,
                    self.metrics.interval_secs
                ),
            }
        }

        if let Ok(retention) = std::env::var("APP_METRICS_RETENTION_DAYS") {
            match parse_days_to_i64(&retention) {
                Ok(val) => {
                    self.metrics.retention_days = val;
                    tracing::info!(
                        "Override metrics.retention_days from env: {}",
                        self.metrics.retention_days
                    );
                },
                Err(e) => tracing::warn!(
                    "Invalid APP_METRICS_RETENTION_DAYS '{}': {} (keep {})",
                    retention,
                    e,
                    self.metrics.retention_days
                ),
            }
        }

        if let Ok(enabled) = std::env::var("APP_METRICS_ENABLED")
            && let Ok(val) = enabled.parse()
        {
            self.metrics.enabled = val;
            tracing::info!("Override metrics.enabled from env: {}", self.metrics.enabled);
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

        // Validate metrics collector
        if self.metrics.interval_secs == 0 {
            anyhow::bail!("metrics.interval_secs must be > 0");
        }
        if self.metrics.retention_days <= 0 {
            anyhow::bail!("metrics.retention_days must be > 0");
        }

        Ok(())
    }

    fn find_config_file() -> Option<String> {
        let possible_paths =
            ["conf/config.toml", "config.toml", "./conf/config.toml", "./config.toml"];

        for path in &possible_paths {
            if Path::new(path).exists() {
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

impl Default for MetricsCollectorConfig {
    fn default() -> Self {
        Self { interval_secs: 30, retention_days: 7, enabled: true }
    }
}

// =========================
// Helpers for parsing values
// =========================

fn parse_duration_to_secs(input: &str) -> Result<u64, String> {
    // Accept plain numbers (treated as seconds)
    if let Ok(val) = input.parse::<u64>() {
        return Ok(val);
    }

    let s = input.trim().to_lowercase();
    let (num_str, unit) = s.split_at(s.chars().take_while(|c| c.is_ascii_digit()).count());
    if num_str.is_empty() || unit.is_empty() {
        return Err("missing number or unit".into());
    }
    let n: u64 = num_str.parse().map_err(|_| "invalid number".to_string())?;
    match unit {
        "s" | "sec" | "secs" | "second" | "seconds" => Ok(n),
        "m" | "min" | "mins" | "minute" | "minutes" => Ok(n * 60),
        "h" | "hr" | "hour" | "hours" => Ok(n * 60 * 60),
        "d" | "day" | "days" => Ok(n * 60 * 60 * 24),
        _ => Err(format!("unsupported unit: {}", unit)),
    }
}

fn parse_days_to_i64(input: &str) -> Result<i64, String> {
    // Accept plain numbers (treated as days)
    if let Ok(val) = input.parse::<i64>() {
        return Ok(val);
    }

    let s = input.trim().to_lowercase();
    let (num_str, unit) = s.split_at(s.chars().take_while(|c| c.is_ascii_digit()).count());
    if num_str.is_empty() || unit.is_empty() {
        return Err("missing number or unit".into());
    }
    let n: i64 = num_str.parse().map_err(|_| "invalid number".to_string())?;
    match unit {
        "d" | "day" | "days" => Ok(n),
        "w" | "week" | "weeks" => Ok(n * 7),
        _ => Err(format!("unsupported unit: {}", unit)),
    }
}

// Custom serde deserializers to support numeric or human-friendly string values
fn deserialize_duration_secs<'de, D>(deserializer: D) -> Result<u64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = u64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a number of seconds or a string like '30s', '5m', '1h'")
        }
        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v)
        }
        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            if v >= 0 { Ok(v as u64) } else { Err(E::custom("negative not allowed")) }
        }
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            parse_duration_to_secs(v).map_err(E::custom)
        }
        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            parse_duration_to_secs(&v).map_err(E::custom)
        }
    }
    deserializer.deserialize_any(Visitor)
}

fn deserialize_days_i64<'de, D>(deserializer: D) -> Result<i64, D::Error>
where
    D: serde::Deserializer<'de>,
{
    struct Visitor;
    impl<'de> serde::de::Visitor<'de> for Visitor {
        type Value = i64;
        fn expecting(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
            write!(f, "a number of days or a string like '7d' or '2w'")
        }
        fn visit_i64<E>(self, v: i64) -> Result<Self::Value, E> {
            Ok(v)
        }
        fn visit_u64<E>(self, v: u64) -> Result<Self::Value, E> {
            Ok(v as i64)
        }
        fn visit_str<E>(self, v: &str) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            parse_days_to_i64(v).map_err(E::custom)
        }
        fn visit_string<E>(self, v: String) -> Result<Self::Value, E>
        where
            E: serde::de::Error,
        {
            parse_days_to_i64(&v).map_err(E::custom)
        }
    }
    deserializer.deserialize_any(Visitor)
}
