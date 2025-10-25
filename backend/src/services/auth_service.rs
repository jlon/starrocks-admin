use crate::models::{CreateUserRequest, LoginRequest, User};
use crate::utils::{ApiError, ApiResult, JwtUtil};
use bcrypt::{hash, verify, DEFAULT_COST};
use sqlx::SqlitePool;
use std::sync::Arc;

#[derive(Clone)]
pub struct AuthService {
    pool: SqlitePool,
    jwt_util: Arc<JwtUtil>,
}

impl AuthService {
    pub fn new(pool: SqlitePool, jwt_util: Arc<JwtUtil>) -> Self {
        Self { pool, jwt_util }
    }

    // Register a new user
    pub async fn register(&self, req: CreateUserRequest) -> ApiResult<User> {
        tracing::debug!("Checking if username exists: {}", req.username);
        
        // Check if username already exists
        let existing_user: Option<User> =
            sqlx::query_as("SELECT * FROM users WHERE username = ?")
                .bind(&req.username)
                .fetch_optional(&self.pool)
                .await?;

        if existing_user.is_some() {
            tracing::warn!("Registration failed: username '{}' already exists", req.username);
            return Err(ApiError::validation_error("Username already exists"));
        }

        tracing::debug!("Hashing password for user: {}", req.username);
        // Hash password
        let password_hash = hash(&req.password, DEFAULT_COST)
            .map_err(|e| {
                tracing::error!("Password hashing failed for user {}: {}", req.username, e);
                ApiError::internal_error(format!("Failed to hash password: {}", e))
            })?;

        tracing::debug!("Inserting user into database: {}", req.username);
        // Insert user
        let result = sqlx::query(
            "INSERT INTO users (username, password_hash, email) VALUES (?, ?, ?)",
        )
        .bind(&req.username)
        .bind(&password_hash)
        .bind(&req.email)
        .execute(&self.pool)
        .await?;

        let user_id = result.last_insert_rowid();

        // Fetch and return the created user
        let user: User = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_one(&self.pool)
            .await?;

        tracing::info!("User registered successfully: {} (ID: {})", user.username, user.id);

        Ok(user)
    }

    // Login and generate JWT token
    pub async fn login(&self, req: LoginRequest) -> ApiResult<(User, String)> {
        tracing::debug!("Looking up user: {}", req.username);
        
        // Find user by username
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = ?")
            .bind(&req.username)
            .fetch_optional(&self.pool)
            .await?;

        let user = user.ok_or_else(|| {
            tracing::warn!("Login failed: user '{}' not found", req.username);
            ApiError::invalid_credentials()
        })?;

        tracing::debug!("Verifying password for user: {}", req.username);
        // Verify password
        let valid = verify(&req.password, &user.password_hash)
            .map_err(|e| {
                tracing::error!("Password verification error for user {}: {}", req.username, e);
                ApiError::internal_error(format!("Password verification failed: {}", e))
            })?;

        if !valid {
            tracing::warn!("Login failed: invalid password for user '{}'", req.username);
            return Err(ApiError::invalid_credentials());
        }

        tracing::debug!("Generating JWT token for user: {}", req.username);
        // Generate JWT token
        let token = self.jwt_util.generate_token(user.id, &user.username)
            .map_err(|e| {
                tracing::error!("JWT token generation failed for user {}: {:?}", req.username, e);
                e
            })?;

        tracing::info!("User logged in successfully: {} (ID: {})", user.username, user.id);

        Ok((user, token))
    }

    // Get user by ID
    pub async fn get_user_by_id(&self, user_id: i64) -> ApiResult<User> {
        let user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE id = ?")
            .bind(user_id)
            .fetch_optional(&self.pool)
            .await?;

        user.ok_or_else(|| ApiError::unauthorized("User not found"))
    }
}

