# 代码重构指南

本文档提供具体的代码重构示例和实施步骤。

---

## 🔐 优先级 P0: 安全性修复

### 1. 密码加密存储方案

#### 当前问题
```rust
// ❌ cluster_service.rs:90 - 明文存储密码
.bind(&req.password) // TODO: Encrypt in production
```

#### 解决方案

**步骤 1**: 添加依赖

```toml
# Cargo.toml
[dependencies]
aes-gcm = "0.10"
base64 = "0.21"
rand = "0.8"
```

**步骤 2**: 创建加密服务

```rust
// src/utils/encryption.rs
use aes_gcm::{
    aead::{Aead, KeyInit},
    Aes256Gcm, Nonce,
};
use base64::{Engine as _, engine::general_purpose};
use rand::Rng;

pub struct EncryptionService {
    cipher: Aes256Gcm,
}

impl EncryptionService {
    /// Create encryption service from config
    pub fn new(secret_key: &str) -> Result<Self, ApiError> {
        // Derive 32-byte key from secret
        let key = Self::derive_key(secret_key)?;
        let cipher = Aes256Gcm::new(&key);
        Ok(Self { cipher })
    }

    /// Encrypt password with random nonce
    pub fn encrypt(&self, plaintext: &str) -> Result<String, ApiError> {
        let mut rng = rand::thread_rng();
        let nonce_bytes: [u8; 12] = rng.gen();
        let nonce = Nonce::from_slice(&nonce_bytes);

        let ciphertext = self
            .cipher
            .encrypt(nonce, plaintext.as_bytes())
            .map_err(|e| ApiError::internal_error(format!("Encryption failed: {}", e)))?;

        // Format: nonce:ciphertext (both base64 encoded)
        let result = format!(
            "{}:{}",
            general_purpose::STANDARD.encode(nonce_bytes),
            general_purpose::STANDARD.encode(ciphertext)
        );

        Ok(result)
    }

    /// Decrypt password
    pub fn decrypt(&self, encrypted: &str) -> Result<String, ApiError> {
        let parts: Vec<&str> = encrypted.split(':').collect();
        if parts.len() != 2 {
            return Err(ApiError::internal_error("Invalid encrypted format"));
        }

        let nonce_bytes = general_purpose::STANDARD
            .decode(parts[0])
            .map_err(|e| ApiError::internal_error(format!("Invalid nonce: {}", e)))?;

        let ciphertext = general_purpose::STANDARD
            .decode(parts[1])
            .map_err(|e| ApiError::internal_error(format!("Invalid ciphertext: {}", e)))?;

        let nonce = Nonce::from_slice(&nonce_bytes);

        let plaintext = self
            .cipher
            .decrypt(nonce, ciphertext.as_ref())
            .map_err(|e| ApiError::internal_error(format!("Decryption failed: {}", e)))?;

        String::from_utf8(plaintext)
            .map_err(|e| ApiError::internal_error(format!("Invalid UTF-8: {}", e)))
    }

    fn derive_key(secret: &str) -> Result<aes_gcm::Key<Aes256Gcm>, ApiError> {
        use sha2::{Digest, Sha256};
        let mut hasher = Sha256::new();
        hasher.update(secret.as_bytes());
        let hash = hasher.finalize();
        Ok(*aes_gcm::Key::<Aes256Gcm>::from_slice(&hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_encrypt_decrypt() {
        let service = EncryptionService::new("test-secret-key-32-bytes-long!").unwrap();
        let password = "my-secure-password";
        
        let encrypted = service.encrypt(password).unwrap();
        let decrypted = service.decrypt(&encrypted).unwrap();
        
        assert_eq!(password, decrypted);
    }
}
```

**步骤 3**: 更新 AppState

```rust
// src/main.rs
pub struct AppState {
    pub db: SqlitePool,
    pub encryption_service: Arc<EncryptionService>, // 新增
    // ... 其他字段
}

// 初始化
let encryption_service = Arc::new(
    EncryptionService::new(&config.encryption.secret_key)
        .map_err(|e| format!("Failed to initialize encryption service: {}", e))?
);

let app_state = AppState {
    db: pool.clone(),
    encryption_service: Arc::clone(&encryption_service),
    // ...
};
```

**步骤 4**: 修改 ClusterService

```rust
// src/services/cluster_service.rs
pub struct ClusterService {
    pool: SqlitePool,
    mysql_pool_manager: Arc<MySQLPoolManager>,
    encryption_service: Arc<EncryptionService>, // 新增
}

impl ClusterService {
    pub async fn create_cluster(&self, req: CreateClusterRequest) -> ApiResult<Cluster> {
        // 加密密码
        let encrypted_password = self.encryption_service.encrypt(&req.password)?;
        
        let result = sqlx::query(
            "INSERT INTO clusters (..., password_encrypted, ...) VALUES (..., ?, ...)"
        )
        // ...
        .bind(&encrypted_password) // ✅ 存储加密后的密码
        .execute(&self.pool)
        .await?;
        
        // ...
    }

    pub async fn get_cluster_credentials(&self, cluster: &Cluster) -> ApiResult<String> {
        // 解密密码
        self.encryption_service.decrypt(&cluster.password_encrypted)
    }
}
```

---

## 🏗️ 优先级 P1: 架构重构

### 2. Repository 模式实现

#### 目标
将数据访问逻辑从 Service 层分离到 Repository 层。

#### 实现步骤

**步骤 1**: 创建 Repository trait

```rust
// src/repositories/mod.rs
pub mod cluster_repository;
pub mod user_repository;

pub use cluster_repository::{ClusterRepository, SqliteClusterRepository};
pub use user_repository::{UserRepository, SqliteUserRepository};

// 通用 Repository trait
#[async_trait::async_trait]
pub trait Repository<T, ID> {
    async fn find_by_id(&self, id: ID) -> ApiResult<Option<T>>;
    async fn find_all(&self) -> ApiResult<Vec<T>>;
    async fn create(&self, entity: T) -> ApiResult<T>;
    async fn update(&self, entity: T) -> ApiResult<T>;
    async fn delete(&self, id: ID) -> ApiResult<bool>;
}
```

**步骤 2**: 实现 UserRepository

```rust
// src/repositories/user_repository.rs
use async_trait::async_trait;
use sqlx::SqlitePool;
use crate::models::User;
use crate::utils::{ApiError, ApiResult};

#[async_trait]
pub trait UserRepository: Send + Sync {
    async fn find_by_id(&self, id: i64) -> ApiResult<Option<User>>;
    async fn find_by_username(&self, username: &str) -> ApiResult<Option<User>>;
    async fn create(&self, user: User) -> ApiResult<User>;
    async fn update(&self, user: User) -> ApiResult<User>;
    async fn is_super_admin(&self, user_id: i64) -> ApiResult<bool>;
    async fn is_org_admin(&self, user_id: i64) -> ApiResult<bool>;
}

pub struct SqliteUserRepository {
    pool: SqlitePool,
}

impl SqliteUserRepository {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl UserRepository for SqliteUserRepository {
    async fn find_by_id(&self, id: i64) -> ApiResult<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(id)
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    async fn find_by_username(&self, username: &str) -> ApiResult<Option<User>> {
        sqlx::query_as("SELECT * FROM users WHERE username = ?")
            .bind(username)
            .fetch_optional(&self.pool)
            .await
            .map_err(Into::into)
    }

    async fn create(&self, user: User) -> ApiResult<User> {
        let result = sqlx::query(
            "INSERT INTO users (username, password_hash, email, avatar) VALUES (?, ?, ?, ?)"
        )
        .bind(&user.username)
        .bind(&user.password_hash)
        .bind(&user.email)
        .bind(&user.avatar)
        .execute(&self.pool)
        .await?;

        let user_id = result.last_insert_rowid();
        self.find_by_id(user_id)
            .await?
            .ok_or_else(|| ApiError::internal_error("User not found after creation"))
    }

    async fn update(&self, user: User) -> ApiResult<User> {
        sqlx::query(
            "UPDATE users SET username = ?, email = ?, avatar = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?"
        )
        .bind(&user.username)
        .bind(&user.email)
        .bind(&user.avatar)
        .bind(user.id)
        .execute(&self.pool)
        .await?;

        self.find_by_id(user.id)
            .await?
            .ok_or_else(|| ApiError::internal_error("User not found after update"))
    }

    async fn is_super_admin(&self, user_id: i64) -> ApiResult<bool> {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM user_roles ur 
             INNER JOIN roles r ON ur.role_id = r.id 
             WHERE ur.user_id = ? AND r.code = 'super_admin' LIMIT 1"
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(exists.is_some())
    }

    async fn is_org_admin(&self, user_id: i64) -> ApiResult<bool> {
        let exists: Option<i64> = sqlx::query_scalar(
            "SELECT 1 FROM user_roles ur 
             JOIN roles r ON ur.role_id = r.id 
             WHERE ur.user_id = ? AND r.code LIKE 'org_admin_%' LIMIT 1"
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(exists.is_some())
    }
}
```

**步骤 3**: 重构 AuthService 使用 Repository

```rust
// src/services/auth_service.rs
use crate::repositories::UserRepository;

pub struct AuthService<R: UserRepository> {
    repository: R,
    jwt_util: Arc<JwtUtil>,
}

impl<R: UserRepository> AuthService<R> {
    pub fn new(repository: R, jwt_util: Arc<JwtUtil>) -> Self {
        Self { repository, jwt_util }
    }

    pub async fn register(&self, req: CreateUserRequest) -> ApiResult<User> {
        // 检查用户名是否存在
        if let Some(_) = self.repository.find_by_username(&req.username).await? {
            return Err(ApiError::validation_error("Username already exists"));
        }

        // 哈希密码
        let password_hash = hash(&req.password, DEFAULT_COST)
            .map_err(|e| ApiError::internal_error(format!("Failed to hash password: {}", e)))?;

        // 创建用户对象
        let user = User {
            id: 0, // 会被数据库自动生成
            username: req.username,
            password_hash,
            email: req.email,
            avatar: req.avatar,
            created_at: chrono::Utc::now().naive_utc(),
            updated_at: chrono::Utc::now().naive_utc(),
            organization_id: None,
        };

        // 保存用户
        self.repository.create(user).await
    }

    pub async fn get_user_by_id(&self, user_id: i64) -> ApiResult<User> {
        self.repository
            .find_by_id(user_id)
            .await?
            .ok_or_else(|| ApiError::unauthorized("User not found"))
    }

    pub async fn to_user_response(&self, user: User) -> ApiResult<UserResponse> {
        let is_super_admin = self.repository.is_super_admin(user.id).await?;
        let is_org_admin = self.repository.is_org_admin(user.id).await?;
        Ok(UserResponse::from_user(user, is_super_admin, is_org_admin))
    }
}
```

---

### 3. 配置模块拆分

#### 当前问题
`config.rs` 文件过大 (512 行)，职责过多。

#### 重构方案

```
src/config/
  ├── mod.rs              // 主配置结构
  ├── cli.rs              // CLI 参数解析
  ├── env.rs              // 环境变量处理
  ├── validation.rs       // 配置验证
  ├── deserializers.rs    // 自定义反序列化
  └── loader.rs           // 配置加载逻辑
```

**实现示例**:

```rust
// src/config/mod.rs
mod cli;
mod env;
mod validation;
mod deserializers;
mod loader;

pub use cli::CommandLineArgs;
pub use loader::ConfigLoader;

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

impl Config {
    pub fn load() -> Result<Self, anyhow::Error> {
        ConfigLoader::new()
            .load_from_file()
            .apply_env_overrides()
            .apply_cli_overrides()
            .validate()
            .build()
    }
}
```

```rust
// src/config/loader.rs
pub struct ConfigLoader {
    config: Config,
    cli_args: CommandLineArgs,
}

impl ConfigLoader {
    pub fn new() -> Self {
        Self {
            config: Config::default(),
            cli_args: CommandLineArgs::parse(),
        }
    }

    pub fn load_from_file(mut self) -> Self {
        if let Some(path) = self.find_config_path() {
            self.config = Self::parse_toml(&path).unwrap_or_default();
        }
        self
    }

    pub fn apply_env_overrides(mut self) -> Self {
        env::apply_overrides(&mut self.config);
        self
    }

    pub fn apply_cli_overrides(mut self) -> Self {
        cli::apply_overrides(&mut self.config, &self.cli_args);
        self
    }

    pub fn validate(self) -> Result<Self, anyhow::Error> {
        validation::validate(&self.config)?;
        Ok(self)
    }

    pub fn build(self) -> Result<Config, anyhow::Error> {
        Ok(self.config)
    }
}
```

---

### 4. 权限提取器简化

#### 当前问题
`permission_extractor.rs` 使用复杂的函数指针和闭包。

#### 优化方案: 配置驱动

```rust
// src/middleware/permission_config.rs
use once_cell::sync::Lazy;
use regex::Regex;

/// Permission rule with pattern matching
pub struct PermissionRule {
    pub method: &'static str,
    pub pattern: Lazy<Regex>,
    pub permission: &'static str,
}

/// Static permission rules configuration
pub static PERMISSION_RULES: &[PermissionRule] = &[
    PermissionRule {
        method: "DELETE",
        pattern: Lazy::new(|| Regex::new(r"^/api/clusters/backends/[^/]+/\d+$").unwrap()),
        permission: "backends:delete",
    },
    PermissionRule {
        method: "DELETE",
        pattern: Lazy::new(|| Regex::new(r"^/api/clusters/queries/.+$").unwrap()),
        permission: "queries:kill",
    },
    PermissionRule {
        method: "DELETE",
        pattern: Lazy::new(|| Regex::new(r"^/api/clusters/sessions/.+$").unwrap()),
        permission: "sessions:kill",
    },
    PermissionRule {
        method: "GET",
        pattern: Lazy::new(|| Regex::new(r"^/api/clusters/profiles/.+$").unwrap()),
        permission: "profiles:get",
    },
    // ... 更多规则
];

/// Extract resource from path
fn extract_resource(path: &str) -> Option<&str> {
    let segments: Vec<&str> = path.strip_prefix("/api/")?.split('/').collect();
    segments.first().copied()
}

/// Extract permission using configured rules
pub fn extract_permission(method: &str, uri: &str) -> Option<(String, String)> {
    // Try exact match rules first
    for rule in PERMISSION_RULES.iter() {
        if rule.method == method && rule.pattern.is_match(uri) {
            let resource = extract_resource(uri)?;
            return Some((resource.to_string(), rule.permission.to_string()));
        }
    }

    // Fallback to generic pattern
    extract_generic_permission(method, uri)
}

fn extract_generic_permission(method: &str, uri: &str) -> Option<(String, String)> {
    let path = uri.strip_prefix("/api/")?;
    let segments: Vec<&str> = path.split('/').collect();
    let resource = segments.first()?;

    let action = match (segments.len(), method) {
        (1, "GET") => "list",
        (1, "POST") => "create",
        (2, "GET") if is_numeric(segments[1]) => "get",
        (2, "PUT") if is_numeric(segments[1]) => "update",
        (2, "DELETE") if is_numeric(segments[1]) => "delete",
        _ => return None,
    };

    Some((resource.to_string(), action.to_string()))
}

fn is_numeric(s: &str) -> bool {
    s.parse::<i64>().is_ok()
}
```

---

## 🧪 优先级 P1: 测试覆盖

### 5. 单元测试框架搭建

```rust
// tests/common/mod.rs - 测试工具函数
use sqlx::SqlitePool;
use starrocks_admin::*;

pub async fn setup_test_db() -> SqlitePool {
    let pool = SqlitePool::connect("sqlite::memory:").await.unwrap();
    
    // 运行迁移
    sqlx::migrate!("./migrations")
        .run(&pool)
        .await
        .unwrap();
    
    pool
}

pub async fn create_test_user(pool: &SqlitePool, username: &str) -> User {
    let password_hash = bcrypt::hash("test-password", bcrypt::DEFAULT_COST).unwrap();
    
    let result = sqlx::query(
        "INSERT INTO users (username, password_hash, email) VALUES (?, ?, ?)"
    )
    .bind(username)
    .bind(&password_hash)
    .bind(format!("{}@test.com", username))
    .execute(pool)
    .await
    .unwrap();
    
    let user_id = result.last_insert_rowid();
    
    sqlx::query_as("SELECT * FROM users WHERE id = ?")
        .bind(user_id)
        .fetch_one(pool)
        .await
        .unwrap()
}
```

```rust
// tests/services/auth_service_test.rs
mod common;

use common::*;
use starrocks_admin::services::AuthService;
use starrocks_admin::models::{CreateUserRequest, LoginRequest};
use starrocks_admin::utils::JwtUtil;
use std::sync::Arc;

#[tokio::test]
async fn test_register_success() {
    // Arrange
    let pool = setup_test_db().await;
    let jwt_util = Arc::new(JwtUtil::new("test-secret", "24h"));
    let service = AuthService::new(pool.clone(), jwt_util);
    
    let request = CreateUserRequest {
        username: "testuser".to_string(),
        password: "Test123!@#".to_string(),
        email: Some("test@example.com".to_string()),
        avatar: None,
    };
    
    // Act
    let result = service.register(request).await;
    
    // Assert
    assert!(result.is_ok());
    let user = result.unwrap();
    assert_eq!(user.username, "testuser");
    assert_eq!(user.email, Some("test@example.com".to_string()));
}

#[tokio::test]
async fn test_register_duplicate_username() {
    let pool = setup_test_db().await;
    let jwt_util = Arc::new(JwtUtil::new("test-secret", "24h"));
    let service = AuthService::new(pool.clone(), jwt_util);
    
    // 创建第一个用户
    create_test_user(&pool, "testuser").await;
    
    // 尝试创建重复用户名
    let request = CreateUserRequest {
        username: "testuser".to_string(),
        password: "Test123!@#".to_string(),
        email: None,
        avatar: None,
    };
    
    let result = service.register(request).await;
    
    assert!(result.is_err());
    assert!(result.unwrap_err().to_string().contains("already exists"));
}

#[tokio::test]
async fn test_login_success() {
    let pool = setup_test_db().await;
    let jwt_util = Arc::new(JwtUtil::new("test-secret", "24h"));
    let service = AuthService::new(pool.clone(), jwt_util);
    
    // 注册用户
    let register_req = CreateUserRequest {
        username: "testuser".to_string(),
        password: "Test123!@#".to_string(),
        email: None,
        avatar: None,
    };
    service.register(register_req).await.unwrap();
    
    // 登录
    let login_req = LoginRequest {
        username: "testuser".to_string(),
        password: "Test123!@#".to_string(),
    };
    
    let result = service.login(login_req).await;
    
    assert!(result.is_ok());
    let (user, token) = result.unwrap();
    assert_eq!(user.username, "testuser");
    assert!(!token.is_empty());
}

#[tokio::test]
async fn test_login_invalid_credentials() {
    let pool = setup_test_db().await;
    let jwt_util = Arc::new(JwtUtil::new("test-secret", "24h"));
    let service = AuthService::new(pool.clone(), jwt_util);
    
    create_test_user(&pool, "testuser").await;
    
    let login_req = LoginRequest {
        username: "testuser".to_string(),
        password: "wrong-password".to_string(),
    };
    
    let result = service.login(login_req).await;
    
    assert!(result.is_err());
}
```

---

## 📊 性能优化示例

### 6. 数据库查询优化

#### N+1 查询问题修复

```rust
// ❌ 原实现 - auth_service.rs
pub async fn to_user_response(&self, user: User) -> ApiResult<UserResponse> {
    let is_super_admin = self.is_user_super_admin(user.id).await?; // 查询 1
    let is_org_admin = self.is_user_org_admin(user.id).await?;     // 查询 2
    Ok(UserResponse::from_user(user, is_super_admin, is_org_admin))
}

// ✅ 优化后 - 单次查询
pub async fn to_user_response(&self, user: User) -> ApiResult<UserResponse> {
    let (is_super_admin, is_org_admin): (bool, bool) = sqlx::query_as(
        r#"
        SELECT 
            COALESCE(MAX(CASE WHEN r.code = 'super_admin' THEN 1 ELSE 0 END), 0) as is_super_admin,
            COALESCE(MAX(CASE WHEN r.code LIKE 'org_admin_%' THEN 1 ELSE 0 END), 0) as is_org_admin
        FROM user_roles ur
        LEFT JOIN roles r ON ur.role_id = r.id
        WHERE ur.user_id = ?
        "#
    )
    .bind(user.id)
    .fetch_optional(&self.pool)
    .await?
    .unwrap_or((false, false));
    
    Ok(UserResponse::from_user(user, is_super_admin, is_org_admin))
}
```

### 7. 缓存实现

```rust
// src/cache/mod.rs
use dashmap::DashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};

pub struct CacheEntry<T> {
    value: T,
    expires_at: Instant,
}

pub struct Cache<K, V> 
where
    K: Eq + std::hash::Hash,
{
    data: Arc<DashMap<K, CacheEntry<V>>>,
    ttl: Duration,
}

impl<K, V> Cache<K, V>
where
    K: Eq + std::hash::Hash + Clone,
    V: Clone,
{
    pub fn new(ttl: Duration) -> Self {
        Self {
            data: Arc::new(DashMap::new()),
            ttl,
        }
    }

    pub fn get(&self, key: &K) -> Option<V> {
        self.data.get(key).and_then(|entry| {
            if entry.expires_at > Instant::now() {
                Some(entry.value.clone())
            } else {
                drop(entry);
                self.data.remove(key);
                None
            }
        })
    }

    pub fn set(&self, key: K, value: V) {
        let entry = CacheEntry {
            value,
            expires_at: Instant::now() + self.ttl,
        };
        self.data.insert(key, entry);
    }

    pub fn invalidate(&self, key: &K) {
        self.data.remove(key);
    }

    pub fn clear(&self) {
        self.data.clear();
    }
}
```

```rust
// 使用缓存
use std::time::Duration;
use once_cell::sync::Lazy;

static USER_ROLE_CACHE: Lazy<Cache<i64, Vec<String>>> = 
    Lazy::new(|| Cache::new(Duration::from_secs(300))); // 5分钟缓存

impl AuthService {
    pub async fn get_user_roles_cached(&self, user_id: i64) -> ApiResult<Vec<String>> {
        // 尝试从缓存获取
        if let Some(roles) = USER_ROLE_CACHE.get(&user_id) {
            tracing::debug!("Cache hit for user roles: {}", user_id);
            return Ok(roles);
        }

        // 缓存未命中，从数据库查询
        tracing::debug!("Cache miss for user roles: {}", user_id);
        let roles = self.fetch_user_roles_from_db(user_id).await?;
        
        // 写入缓存
        USER_ROLE_CACHE.set(user_id, roles.clone());
        
        Ok(roles)
    }

    // 角色变更时清除缓存
    pub async fn update_user_roles(&self, user_id: i64, roles: Vec<i64>) -> ApiResult<()> {
        // 更新数据库
        self.update_user_roles_in_db(user_id, roles).await?;
        
        // 清除缓存
        USER_ROLE_CACHE.invalidate(&user_id);
        
        Ok(())
    }
}
```

---

## 📝 文档规范

### 8. API 文档模板

```rust
/// Create a new cluster in the system
///
/// # Arguments
///
/// * `req` - Cluster creation request containing configuration details
/// * `user_id` - ID of the user creating the cluster
/// * `requestor_org` - Optional organization ID of the requestor
/// * `is_super_admin` - Whether the requestor is a super admin
///
/// # Returns
///
/// Returns the created `Cluster` on success, or an `ApiError` on failure.
///
/// # Errors
///
/// This function will return an error if:
/// * Cluster name already exists (`ValidationError`)
/// * Database connection fails (`DatabaseError`)
/// * User lacks required permissions (`Unauthorized`)
///
/// # Examples
///
/// ```no_run
/// use starrocks_admin::services::ClusterService;
/// use starrocks_admin::models::CreateClusterRequest;
///
/// # async fn example(service: ClusterService) -> Result<(), Box<dyn std::error::Error>> {
/// let request = CreateClusterRequest {
///     name: "prod-cluster".to_string(),
///     fe_host: "192.168.1.100".to_string(),
///     fe_query_port: 9030,
///     // ... other fields
/// };
///
/// let cluster = service.create_cluster(request, 1, None, false).await?;
/// println!("Created cluster: {}", cluster.name);
/// # Ok(())
/// # }
/// ```
///
/// # Implementation Notes
///
/// - Automatically sets the first cluster in an organization as active
/// - Trims whitespace from all string inputs
/// - Validates cluster name uniqueness within the system
/// - Resolves target organization based on user permissions
///
/// # Security
///
/// - Requires valid user authentication
/// - Enforces organization-level access control
/// - Super admins can create clusters for any organization
///
pub async fn create_cluster(
    &self,
    req: CreateClusterRequest,
    user_id: i64,
    requestor_org: Option<i64>,
    is_super_admin: bool,
) -> ApiResult<Cluster> {
    // Implementation...
}
```

---

## 🎯 实施计划

### 第一周: 安全性
- [ ] 实现 EncryptionService
- [ ] 更新 ClusterService 使用加密
- [ ] 修改数据库迁移脚本
- [ ] 测试加密/解密功能
- [ ] 限制生产环境 CORS

### 第二周: Repository 模式
- [ ] 创建 Repository trait
- [ ] 实现 UserRepository
- [ ] 实现 ClusterRepository
- [ ] 重构 AuthService
- [ ] 重构 ClusterService

### 第三周: 测试覆盖
- [ ] 搭建测试框架
- [ ] AuthService 单元测试
- [ ] ClusterService 单元测试
- [ ] 集成测试
- [ ] 达到 >80% 覆盖率

### 第四周: 配置和文档
- [ ] 拆分 config 模块
- [ ] 简化 permission_extractor
- [ ] 添加 API 文档
- [ ] 性能优化
- [ ] 代码审查

---

## 📚 参考资料

- [Rust API Guidelines](https://rust-lang.github.io/api-guidelines/)
- [Axum Examples](https://github.com/tokio-rs/axum/tree/main/examples)
- [SQLx Best Practices](https://github.com/launchbadge/sqlx/blob/main/FAQ.md)
- [Tokio Tutorial](https://tokio.rs/tokio/tutorial)

---

**最后更新**: 2024-11-26
