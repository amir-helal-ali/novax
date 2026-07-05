//! NovaX Authentication
//!
//! Provides JWT-based authentication with PostgreSQL session storage,
//! email verification, password reset, and OAuth2 (Google + GitHub).
//!
//! ## Features
//! - Password hashing with Argon2id (industry standard)
//! - JWT tokens (HMAC-SHA256) for stateless auth
//! - Session storage in PostgreSQL for revocation
//! - Email verification tokens
//! - Password reset tokens
//! - OAuth2: Google + GitHub
//! - Constant-time password comparison
//! - Secure defaults (no insecure algorithms)
//!
//! ## Quick Start
//! ```rust,no_run
//! use novax_auth::{AuthService, AuthConfig};
//!
//! let auth = AuthService::new(AuthConfig::default());
//! let user = auth.register(&pool, "alice@example.com", "alice", "secure_password").await?;
//! let session = auth.login(&pool, "alice@example.com", "secure_password").await?;
//! let verified = auth.verify_token(&session.token).await?;
//! ```

use std::sync::Arc;

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use base64::{engine::general_purpose::URL_SAFE_NO_PAD, Engine};
use chrono::{DateTime, Duration, Utc};
use hmac::{Hmac, Mac};
use serde::{Deserialize, Serialize};
use sha2::Sha256;
use subtle::ConstantTimeEq;
use thiserror::Error;
use tracing::{info, warn};
use uuid::Uuid;

#[cfg(feature = "postgres")]
use sqlx::PgPool;

pub mod oauth;
pub mod tokens;

pub use oauth::{OAuthConfig, OAuthProvider, OAuthProviderConfig, OAuthUserInfo, build_auth_url, generate_state};
pub use tokens::{EmailVerificationToken, PasswordResetToken};

type HmacSha256 = Hmac<Sha256>;

/// Authentication configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// Secret key for JWT signing (KEEP SECURE!)
    pub jwt_secret: String,
    /// Token lifetime in seconds (default: 1 hour)
    pub token_ttl_seconds: i64,
    /// Refresh token lifetime in seconds (default: 30 days)
    pub refresh_ttl_seconds: i64,
    /// Argon2 memory cost in KB (default: 19456 = 19MB)
    pub argon2_memory_kb: u32,
    /// Argon2 iterations (default: 2)
    pub argon2_iterations: u32,
    /// Argon2 parallelism (default: 1)
    pub argon2_parallelism: u32,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            jwt_secret: std::env::var("JWT_SECRET")
                .unwrap_or_else(|_| {
                    warn!("JWT_SECRET not set, using insecure default — set JWT_SECRET env var!");
                    "CHANGE_ME_IN_PRODUCTION_please_use_a_long_random_secret".to_string()
                }),
            token_ttl_seconds: 3600,            // 1 hour
            refresh_ttl_seconds: 30 * 24 * 3600, // 30 days
            argon2_memory_kb: 19456,
            argon2_iterations: 2,
            argon2_parallelism: 1,
        }
    }
}

/// Auth error types
#[derive(Debug, Error)]
pub enum AuthError {
    #[error("invalid credentials")]
    InvalidCredentials,
    #[error("user already exists")]
    UserExists,
    #[error("user not found")]
    UserNotFound,
    #[error("invalid token")]
    InvalidToken,
    #[error("token expired")]
    TokenExpired,
    #[error("password too weak (min 8 chars)")]
    WeakPassword,
    #[error("database error: {0}")]
    Database(String),
    #[error("hashing error: {0}")]
    Hashing(String),
    #[error("internal error: {0}")]
    Internal(String),
}

#[cfg(feature = "postgres")]
impl From<sqlx::Error> for AuthError {
    fn from(e: sqlx::Error) -> Self {
        if let sqlx::Error::Database(db_err) = &e {
            if db_err.is_unique_violation() {
                return AuthError::UserExists;
            }
        }
        AuthError::Database(e.to_string())
    }
}

impl From<argon2::password_hash::Error> for AuthError {
    fn from(e: argon2::password_hash::Error) -> Self {
        AuthError::Hashing(e.to_string())
    }
}

/// Authenticated user record
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct AuthUser {
    pub id: Uuid,
    pub email: String,
    pub name: String,
    #[serde(skip_serializing)]
    pub password_hash: String,
    pub bio: Option<String>,
    pub avatar_url: Option<String>,
    pub is_active: bool,
    pub is_admin: bool,
    pub email_verified_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

/// JWT claims
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Claims {
    /// Subject (user ID)
    pub sub: String,
    /// Email
    pub email: String,
    /// Issued at (timestamp)
    pub iat: i64,
    /// Expiration time (timestamp)
    pub exp: i64,
    /// JWT ID (unique per token)
    pub jti: String,
}

/// Authentication session returned on login
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthSession {
    pub token: String,
    pub refresh_token: String,
    pub user: AuthUser,
    pub expires_at: DateTime<Utc>,
}

/// Authentication service
#[derive(Clone)]
pub struct AuthService {
    config: Arc<AuthConfig>,
    argon2: Arc<Argon2<'static>>,
}

impl AuthService {
    /// Create a new auth service with the given configuration
    pub fn new(config: AuthConfig) -> Self {
        let argon2 = Argon2::new(
            argon2::Algorithm::Argon2id,
            argon2::Version::V0x13,
            argon2::Params::new(config.argon2_memory_kb, config.argon2_iterations, config.argon2_parallelism, None)
                .expect("invalid Argon2 params"),
        );
        Self {
            config: Arc::new(config),
            argon2: Arc::new(argon2),
        }
    }

    /// Get the auth configuration
    pub fn config(&self) -> &AuthConfig {
        &self.config
    }

    /// Hash a password using Argon2id
    pub fn hash_password(&self, password: &str) -> Result<String, AuthError> {
        let salt = SaltString::generate(&mut OsRng);
        let hash = self.argon2.hash_password(password.as_bytes(), &salt)?;
        Ok(hash.to_string())
    }

    /// Verify a password against a hash in constant time
    pub fn verify_password(&self, password: &str, hash: &str) -> Result<bool, AuthError> {
        let parsed = PasswordHash::new(hash)?;
        // verify_password runs in constant time internally
        match self.argon2.verify_password(password.as_bytes(), &parsed) {
            Ok(()) => Ok(true),
            Err(argon2::password_hash::Error::Password) => Ok(false),
            Err(e) => Err(e.into()),
        }
    }

    /// Validate password strength (basic)
    pub fn validate_password_strength(password: &str) -> Result<(), AuthError> {
        if password.len() < 8 {
            return Err(AuthError::WeakPassword);
        }
        Ok(())
    }

    /// Generate a JWT token for a user
    pub fn generate_token(&self, user: &AuthUser) -> Result<(String, DateTime<Utc>), AuthError> {
        let now = Utc::now();
        let exp = now + Duration::seconds(self.config.token_ttl_seconds);
        let jti = Uuid::new_v4().to_string();

        let claims = Claims {
            sub: user.id.to_string(),
            email: user.email.clone(),
            iat: now.timestamp(),
            exp: exp.timestamp(),
            jti,
        };

        let header = URL_SAFE_NO_PAD.encode(serde_json::json!({"alg":"HS256","typ":"JWT"}).to_string());
        let payload = URL_SAFE_NO_PAD.encode(serde_json::to_vec(&claims).map_err(|e| AuthError::Internal(e.to_string()))?);

        let signing_input = format!("{}.{}", header, payload);
        let mut mac = HmacSha256::new_from_slice(self.config.jwt_secret.as_bytes())
            .map_err(|e| AuthError::Internal(e.to_string()))?;
        mac.update(signing_input.as_bytes());
        let signature = URL_SAFE_NO_PAD.encode(mac.finalize().into_bytes());

        let token = format!("{}.{}", signing_input, signature);
        Ok((token, exp))
    }

    /// Generate a refresh token (longer-lived)
    pub fn generate_refresh_token(&self) -> String {
        // 32 bytes of randomness, base64-encoded
        let bytes: [u8; 32] = rand::random();
        URL_SAFE_NO_PAD.encode(bytes)
    }

    /// Verify and decode a JWT token
    pub fn verify_token(&self, token: &str) -> Result<Claims, AuthError> {
        let parts: Vec<&str> = token.split('.').collect();
        if parts.len() != 3 {
            return Err(AuthError::InvalidToken);
        }

        let signing_input = format!("{}.{}", parts[0], parts[1]);
        let signature = URL_SAFE_NO_PAD.decode(parts[2])
            .map_err(|_| AuthError::InvalidToken)?;

        let mut mac = HmacSha256::new_from_slice(self.config.jwt_secret.as_bytes())
            .map_err(|e| AuthError::Internal(e.to_string()))?;
        mac.update(signing_input.as_bytes());
        let expected = mac.finalize().into_bytes();

        // Constant-time comparison
        if signature.ct_eq(&expected).into() {
            // Signature valid — decode claims
            let claims_bytes = URL_SAFE_NO_PAD.decode(parts[1])
                .map_err(|_| AuthError::InvalidToken)?;
            let claims: Claims = serde_json::from_slice(&claims_bytes)
                .map_err(|_| AuthError::InvalidToken)?;

            // Check expiration
            if Utc::now().timestamp() > claims.exp {
                return Err(AuthError::TokenExpired);
            }

            Ok(claims)
        } else {
            Err(AuthError::InvalidToken)
        }
    }

    /// Register a new user
    #[cfg(feature = "postgres")]
    pub async fn register(
        &self,
        pool: &PgPool,
        email: &str,
        name: &str,
        password: &str,
    ) -> Result<AuthUser, AuthError> {
        Self::validate_password_strength(password)?;
        let password_hash = self.hash_password(password)?;
        let email = email.to_lowercase();

        let user: AuthUser = sqlx::query_as(
            r#"INSERT INTO users (email, name, password_hash)
               VALUES ($1, $2, $3)
               RETURNING id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at"#,
        )
        .bind(&email)
        .bind(name)
        .bind(&password_hash)
        .fetch_one(pool)
        .await?;

        info!(user_id = %user.id, "New user registered");
        Ok(user)
    }

    /// Authenticate a user and return a session
    #[cfg(feature = "postgres")]
    pub async fn login(
        &self,
        pool: &PgPool,
        email: &str,
        password: &str,
    ) -> Result<AuthSession, AuthError> {
        let email = email.to_lowercase();

        let user: AuthUser = sqlx::query_as(
            "SELECT id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at FROM users WHERE email = $1",
        )
        .bind(&email)
        .fetch_optional(pool)
        .await?
        .ok_or(AuthError::InvalidCredentials)?;

        if !user.is_active {
            return Err(AuthError::InvalidCredentials);
        }

        if !self.verify_password(password, &user.password_hash)? {
            // Constant-time: same error as user-not-found to prevent user enumeration
            return Err(AuthError::InvalidCredentials);
        }

        let (token, expires_at) = self.generate_token(&user)?;
        let refresh_token = self.generate_refresh_token();

        // Store session in DB for revocation tracking
        sqlx::query(
            r#"INSERT INTO auth_sessions (id, user_id, refresh_token, expires_at)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(Uuid::new_v4())
        .bind(user.id)
        .bind(&refresh_token)
        .bind(expires_at + Duration::seconds(self.config.refresh_ttl_seconds))
        .execute(pool)
        .await?;

        info!(user_id = %user.id, "User logged in");
        Ok(AuthSession {
            token,
            refresh_token,
            user,
            expires_at,
        })
    }

    /// Get user from a JWT token
    #[cfg(feature = "postgres")]
    pub async fn user_from_token(
        &self,
        pool: &PgPool,
        token: &str,
    ) -> Result<AuthUser, AuthError> {
        let claims = self.verify_token(token)?;

        let user_id: Uuid = claims.sub.parse()
            .map_err(|_| AuthError::InvalidToken)?;

        let user: AuthUser = sqlx::query_as(
            "SELECT id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at FROM users WHERE id = $1 AND is_active = TRUE",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AuthError::UserNotFound)?;

        Ok(user)
    }

    /// Logout (revoke all sessions for a user)
    #[cfg(feature = "postgres")]
    pub async fn logout(&self, pool: &PgPool, user_id: Uuid) -> Result<(), AuthError> {
        sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;
        info!(%user_id, "User logged out");
        Ok(())
    }

    /// Change password (requires current password verification)
    #[cfg(feature = "postgres")]
    pub async fn change_password(
        &self,
        pool: &PgPool,
        user_id: Uuid,
        current_password: &str,
        new_password: &str,
    ) -> Result<(), AuthError> {
        Self::validate_password_strength(new_password)?;

        let user: AuthUser = sqlx::query_as(
            "SELECT id, email, name, password_hash, bio, avatar_url, is_active, is_admin, email_verified_at, created_at, updated_at FROM users WHERE id = $1",
        )
        .bind(user_id)
        .fetch_optional(pool)
        .await?
        .ok_or(AuthError::UserNotFound)?;

        if !self.verify_password(current_password, &user.password_hash)? {
            return Err(AuthError::InvalidCredentials);
        }

        let new_hash = self.hash_password(new_password)?;
        sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
            .bind(&new_hash)
            .bind(user_id)
            .execute(pool)
            .await?;

        // Revoke all existing sessions
        sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
            .bind(user_id)
            .execute(pool)
            .await?;

        info!(%user_id, "User changed password");
        Ok(())
    }
}

/// Extract the Bearer token from an Authorization header
pub fn extract_bearer_token(auth_header: &str) -> Option<&str> {
    if auth_header.len() < 7 || !auth_header[..7].eq_ignore_ascii_case("Bearer ") {
        return None;
    }
    Some(&auth_header[7..])
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_password_hashing() {
        let auth = AuthService::new(AuthConfig::default());
        let hash = auth.hash_password("test_password_123").unwrap();
        assert!(auth.verify_password("test_password_123", &hash).unwrap());
        assert!(!auth.verify_password("wrong_password", &hash).unwrap());
    }

    #[test]
    fn test_password_strength() {
        assert!(AuthService::validate_password_strength("short").is_err());
        assert!(AuthService::validate_password_strength("longenough").is_ok());
    }

    #[test]
    fn test_bearer_extraction() {
        assert_eq!(
            extract_bearer_token("Bearer abc123"),
            Some("abc123")
        );
        assert_eq!(extract_bearer_token("Basic abc"), None);
        assert_eq!(extract_bearer_token(""), None);
    }

    #[test]
    fn test_token_generation_and_verification() {
        let auth = AuthService::new(AuthConfig {
            jwt_secret: "test_secret".to_string(),
            ..Default::default()
        });
        let user = AuthUser {
            id: Uuid::new_v4(),
            email: "test@example.com".to_string(),
            name: "Test".to_string(),
            password_hash: String::new(),
            bio: None,
            avatar_url: None,
            is_active: true,
            is_admin: false,
            email_verified_at: None,
            created_at: Utc::now(),
            updated_at: Utc::now(),
        };
        let (token, _) = auth.generate_token(&user).unwrap();
        let claims = auth.verify_token(&token).unwrap();
        assert_eq!(claims.sub, user.id.to_string());
        assert_eq!(claims.email, user.email);
    }
}
