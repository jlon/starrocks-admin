use crate::models::{CreateUserRequest, LoginRequest, UpdateUserRequest, User, UserResponse};
use crate::utils::{ApiError, ApiResult, JwtUtil};
use bcrypt::{DEFAULT_COST, hash, verify};
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
        let existing_user: Option<User> = sqlx::query_as("SELECT * FROM users WHERE username = ?")
            .bind(&req.username)
            .fetch_optional(&self.pool)
            .await?;

        if existing_user.is_some() {
            tracing::warn!("Registration failed: username '{}' already exists", req.username);
            return Err(ApiError::validation_error("Username already exists"));
        }

        tracing::debug!("Hashing password for user: {}", req.username);
        // Hash password
        let password_hash = hash(&req.password, DEFAULT_COST).map_err(|e| {
            tracing::error!("Password hashing failed for user {}: {}", req.username, e);
            ApiError::internal_error(format!("Failed to hash password: {}", e))
        })?;

        tracing::debug!("Inserting user into database: {}", req.username);
        // Insert user
        let result = sqlx::query(
            "INSERT INTO users (username, password_hash, email, avatar) VALUES (?, ?, ?, ?)",
        )
        .bind(&req.username)
        .bind(&password_hash)
        .bind(&req.email)
        .bind(&req.avatar)
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
        let valid = verify(&req.password, &user.password_hash).map_err(|e| {
            tracing::error!("Password verification error for user {}: {}", req.username, e);
            ApiError::internal_error(format!("Password verification failed: {}", e))
        })?;

        if !valid {
            tracing::warn!("Login failed: invalid password for user '{}'", req.username);
            return Err(ApiError::invalid_credentials());
        }

        tracing::debug!("Generating JWT token for user: {}", req.username);
        // Generate JWT token
        let token = self
            .jwt_util
            .generate_token(user.id, &user.username)
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

    // Update user information
    pub async fn update_user(&self, user_id: i64, req: UpdateUserRequest) -> ApiResult<User> {
        tracing::debug!("Updating user information for user_id: {}", user_id);

        // Get current user
        let user = self.get_user_by_id(user_id).await?;

        // If changing password, verify current password first
        if let (Some(current_pwd), Some(new_pwd)) = (&req.current_password, &req.new_password) {
            tracing::debug!("Verifying current password for user_id: {}", user_id);
            let valid = verify(current_pwd, &user.password_hash).map_err(|e| {
                tracing::error!("Password verification error: {}", e);
                ApiError::internal_error(format!("Password verification failed: {}", e))
            })?;

            if !valid {
                tracing::warn!("Current password verification failed for user_id: {}", user_id);
                return Err(ApiError::validation_error("Current password is incorrect"));
            }

            // Hash new password
            tracing::debug!("Hashing new password for user_id: {}", user_id);
            let new_password_hash = hash(new_pwd, DEFAULT_COST).map_err(|e| {
                tracing::error!("Password hashing failed: {}", e);
                ApiError::internal_error(format!("Failed to hash password: {}", e))
            })?;

            // Update password
            sqlx::query(
                "UPDATE users SET password_hash = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?",
            )
            .bind(&new_password_hash)
            .bind(user_id)
            .execute(&self.pool)
            .await?;

            tracing::info!("Password updated successfully for user_id: {}", user_id);
        }

        // Update email if provided
        if let Some(email) = &req.email {
            tracing::debug!("Updating email for user_id: {}", user_id);
            sqlx::query("UPDATE users SET email = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(email)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        }

        // Update avatar if provided
        if let Some(avatar) = &req.avatar {
            tracing::debug!("Updating avatar for user_id: {}", user_id);
            sqlx::query("UPDATE users SET avatar = ?, updated_at = CURRENT_TIMESTAMP WHERE id = ?")
                .bind(avatar)
                .bind(user_id)
                .execute(&self.pool)
                .await?;
        }

        // Fetch and return updated user
        let updated_user = self.get_user_by_id(user_id).await?;
        tracing::info!(
            "User updated successfully: {} (ID: {})",
            updated_user.username,
            updated_user.id
        );

        Ok(updated_user)
    }

    pub async fn is_user_super_admin(&self, user_id: i64) -> ApiResult<bool> {
        let exists: Option<i64> = sqlx::query_scalar(
            r#"
            SELECT ur.role_id
            FROM user_roles ur
            INNER JOIN roles r ON ur.role_id = r.id
            WHERE ur.user_id = ? AND r.code = 'super_admin'
            LIMIT 1
            "#,
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(exists.is_some())
    }

    pub async fn to_user_response(&self, user: User) -> ApiResult<UserResponse> {
        let is_super_admin = self.is_user_super_admin(user.id).await?;
        let is_org_admin = self.is_user_org_admin(user.id).await?;
        Ok(UserResponse::from_user(user, is_super_admin, is_org_admin))
    }

    async fn is_user_org_admin(&self, user_id: i64) -> ApiResult<bool> {
        let exists: Option<(i64,)> = sqlx::query_as(
            "SELECT 1 FROM user_roles ur 
             JOIN roles r ON ur.role_id = r.id 
             WHERE ur.user_id = ? AND r.code LIKE 'org_admin_%' 
             LIMIT 1",
        )
        .bind(user_id)
        .fetch_optional(&self.pool)
        .await?;

        Ok(exists.is_some())
    }
}
