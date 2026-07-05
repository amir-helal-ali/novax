//! Email verification and password reset tokens

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use sqlx::PgPool;
use uuid::Uuid;
use base64::Engine;

use crate::AuthError;

/// Email verification token
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct EmailVerificationToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

/// Password reset token
#[derive(Debug, Clone, Serialize, Deserialize, sqlx::FromRow)]
pub struct PasswordResetToken {
    pub id: Uuid,
    pub user_id: Uuid,
    pub token: String,
    pub expires_at: DateTime<Utc>,
    pub created_at: DateTime<Utc>,
    pub used_at: Option<DateTime<Utc>>,
}

/// Token expiration durations
const EMAIL_VERIFICATION_TTL_HOURS: i64 = 24;
const PASSWORD_RESET_TTL_HOURS: i64 = 1;

/// Generate a secure random token (32 bytes, base64-encoded)
pub fn generate_token() -> String {
    let bytes: [u8; 32] = rand::random();
    base64::engine::general_purpose::URL_SAFE_NO_PAD.encode(bytes)
}

#[cfg(feature = "postgres")]
impl crate::AuthService {
    /// Generate and store an email verification token for a user
    pub async fn create_email_verification_token(
        &self,
        pool: &PgPool,
        user_id: Uuid,
    ) -> Result<String, AuthError> {
        let token = generate_token();
        let expires_at = Utc::now() + Duration::hours(EMAIL_VERIFICATION_TTL_HOURS);

        sqlx::query(
            r#"INSERT INTO email_verification_tokens (id, user_id, token, expires_at)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(&token)
        .bind(expires_at)
        .execute(pool)
        .await?;

        tracing::info!(%user_id, "Email verification token created");
        Ok(token)
    }

    /// Verify email with token
    pub async fn verify_email(
        &self,
        pool: &PgPool,
        token: &str,
    ) -> Result<Uuid, AuthError> {
        let record: Option<EmailVerificationToken> = sqlx::query_as(
            "SELECT id, user_id, token, expires_at, created_at, used_at FROM email_verification_tokens WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        let record = record.ok_or(AuthError::InvalidToken)?;

        if record.used_at.is_some() {
            return Err(AuthError::InvalidToken);
        }
        if Utc::now() >= record.expires_at {
            return Err(AuthError::TokenExpired);
        }

        // Mark as used and update user
        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE email_verification_tokens SET used_at = NOW() WHERE id = $1")
            .bind(record.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE users SET email_verified_at = NOW(), updated_at = NOW() WHERE id = $1")
            .bind(record.user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        tracing::info!(user_id = %record.user_id, "Email verified");
        Ok(record.user_id)
    }

    /// Generate a password reset token for an email
    pub async fn create_password_reset_token(
        &self,
        pool: &PgPool,
        email: &str,
    ) -> Result<Option<String>, AuthError> {
        let email = email.to_lowercase();

        // Find user by email (don't reveal if user doesn't exist)
        let user_id: Option<(Uuid,)> = sqlx::query_as("SELECT id FROM users WHERE email = $1")
            .bind(&email)
            .fetch_optional(pool)
            .await?;

        let Some((user_id,)) = user_id else {
            // Don't reveal if email exists — return Ok(None)
            return Ok(None);
        };

        // Invalidate previous unused tokens for this user
        sqlx::query("UPDATE password_reset_tokens SET used_at = NOW() WHERE user_id = $1 AND used_at IS NULL")
            .bind(user_id)
            .execute(pool)
            .await?;

        let token = generate_token();
        let expires_at = Utc::now() + Duration::hours(PASSWORD_RESET_TTL_HOURS);

        sqlx::query(
            r#"INSERT INTO password_reset_tokens (id, user_id, token, expires_at)
               VALUES ($1, $2, $3, $4)"#,
        )
        .bind(Uuid::new_v4())
        .bind(user_id)
        .bind(&token)
        .bind(expires_at)
        .execute(pool)
        .await?;

        tracing::info!(%user_id, "Password reset token created");
        Ok(Some(token))
    }

    /// Reset password using token
    pub async fn reset_password(
        &self,
        pool: &PgPool,
        token: &str,
        new_password: &str,
    ) -> Result<Uuid, AuthError> {
        crate::AuthService::validate_password_strength(new_password)?;

        let record: Option<PasswordResetToken> = sqlx::query_as(
            "SELECT id, user_id, token, expires_at, created_at, used_at FROM password_reset_tokens WHERE token = $1",
        )
        .bind(token)
        .fetch_optional(pool)
        .await?;

        let record = record.ok_or(AuthError::InvalidToken)?;

        if record.used_at.is_some() {
            return Err(AuthError::InvalidToken);
        }
        if Utc::now() >= record.expires_at {
            return Err(AuthError::TokenExpired);
        }

        let password_hash = self.hash_password(new_password)?;

        let mut tx = pool.begin().await?;
        sqlx::query("UPDATE password_reset_tokens SET used_at = NOW() WHERE id = $1")
            .bind(record.id)
            .execute(&mut *tx)
            .await?;
        sqlx::query("UPDATE users SET password_hash = $1, updated_at = NOW() WHERE id = $2")
            .bind(&password_hash)
            .bind(record.user_id)
            .execute(&mut *tx)
            .await?;
        // Revoke all sessions
        sqlx::query("DELETE FROM auth_sessions WHERE user_id = $1")
            .bind(record.user_id)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;

        tracing::info!(user_id = %record.user_id, "Password reset successful");
        Ok(record.user_id)
    }
}
