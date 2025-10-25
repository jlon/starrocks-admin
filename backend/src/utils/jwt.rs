use chrono::{Duration, Utc};
use jsonwebtoken::{decode, encode, DecodingKey, EncodingKey, Header, Validation};
use serde::{Deserialize, Serialize};

use crate::utils::error::ApiError;

#[derive(Debug, Serialize, Deserialize)]
pub struct Claims {
    pub sub: String,        // User ID
    pub username: String,   // Username
    pub exp: i64,           // Expiration time
    pub iat: i64,           // Issued at
}

#[derive(Clone)]
pub struct JwtUtil {
    encoding_key: EncodingKey,
    decoding_key: DecodingKey,
    expires_in_hours: i64,
}

impl JwtUtil {
    pub fn new(secret: &str, expires_in: &str) -> Self {
        let hours = Self::parse_expiration(expires_in);

        Self {
            encoding_key: EncodingKey::from_secret(secret.as_bytes()),
            decoding_key: DecodingKey::from_secret(secret.as_bytes()),
            expires_in_hours: hours,
        }
    }

    fn parse_expiration(expires_in: &str) -> i64 {
        // Parse "24h", "7d", etc.
        if expires_in.ends_with('h') {
            expires_in.trim_end_matches('h').parse().unwrap_or(24)
        } else if expires_in.ends_with('d') {
            expires_in.trim_end_matches('d').parse::<i64>().unwrap_or(1) * 24
        } else {
            24 // Default 24 hours
        }
    }

    pub fn generate_token(&self, user_id: i64, username: &str) -> Result<String, ApiError> {
        let now = Utc::now();
        let exp = now + Duration::hours(self.expires_in_hours);

        let claims = Claims {
            sub: user_id.to_string(),
            username: username.to_string(),
            exp: exp.timestamp(),
            iat: now.timestamp(),
        };

        encode(&Header::default(), &claims, &self.encoding_key)
            .map_err(|e| ApiError::internal_error(format!("Failed to generate token: {}", e)))
    }

    pub fn verify_token(&self, token: &str) -> Result<Claims, ApiError> {
        decode::<Claims>(token, &self.decoding_key, &Validation::default())
            .map(|data| data.claims)
            .map_err(|e| {
                tracing::warn!("Token verification failed: {}", e);
                ApiError::TokenExpired
            })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_jwt_generation_and_verification() {
        let jwt_util = JwtUtil::new("test-secret", "24h");
        let token = jwt_util.generate_token(1, "testuser").unwrap();
        let claims = jwt_util.verify_token(&token).unwrap();

        assert_eq!(claims.sub, "1");
        assert_eq!(claims.username, "testuser");
    }

    #[test]
    fn test_parse_expiration() {
        assert_eq!(JwtUtil::parse_expiration("24h"), 24);
        assert_eq!(JwtUtil::parse_expiration("7d"), 168);
        assert_eq!(JwtUtil::parse_expiration("invalid"), 24);
    }
}

