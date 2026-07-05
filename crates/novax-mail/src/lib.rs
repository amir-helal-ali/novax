//! NovaX Mail Service
//!
//! SMTP email sending for email verification and password reset.
//! Configurable via environment variables.
//!
//! ## Configuration
//! - `SMTP_HOST` — SMTP server host (default: localhost)
//! - `SMTP_PORT` — SMTP server port (default: 587)
//! - `SMTP_USERNAME` — SMTP username
//! - `SMTP_PASSWORD` — SMTP password
//! - `SMTP_FROM` — From email address (default: noreply@novax.local)
//! - `SMTP_FROM_NAME` — From name (default: NovaX)
//! - `APP_BASE_URL` — Base URL for links (default: http://localhost:3000)

use std::sync::Arc;

use lettre::{
    message::header::ContentType,
    transport::smtp::authentication::Credentials,
    AsyncSmtpTransport, AsyncTransport, Message, Tokio1Executor,
};
use serde::{Deserialize, Serialize};
use thiserror::Error;
use tracing::{info, warn};

/// Mail configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MailConfig {
    pub smtp_host: String,
    pub smtp_port: u16,
    pub smtp_username: Option<String>,
    pub smtp_password: Option<String>,
    pub from_email: String,
    pub from_name: String,
    pub app_base_url: String,
    /// If true, emails are not sent — links are logged instead (for dev)
    pub dev_mode: bool,
}

impl Default for MailConfig {
    fn default() -> Self {
        Self {
            smtp_host: std::env::var("SMTP_HOST").unwrap_or_else(|_| "localhost".to_string()),
            smtp_port: std::env::var("SMTP_PORT").ok().and_then(|p| p.parse().ok()).unwrap_or(587),
            smtp_username: std::env::var("SMTP_USERNAME").ok(),
            smtp_password: std::env::var("SMTP_PASSWORD").ok(),
            from_email: std::env::var("SMTP_FROM").unwrap_or_else(|_| "noreply@novax.local".to_string()),
            from_name: std::env::var("SMTP_FROM_NAME").unwrap_or_else(|_| "NovaX".to_string()),
            app_base_url: std::env::var("APP_BASE_URL").unwrap_or_else(|_| "http://localhost:3000".to_string()),
            dev_mode: std::env::var("NOVAX_ENV").map(|e| e == "development").unwrap_or(true),
        }
    }
}

/// Mail error
#[derive(Debug, Error)]
pub enum MailError {
    #[error("smtp error: {0}")]
    Smtp(String),
    #[error("address parse error: {0}")]
    Address(String),
}

impl From<lettre::transport::smtp::Error> for MailError {
    fn from(e: lettre::transport::smtp::Error) -> Self {
        MailError::Smtp(e.to_string())
    }
}

impl From<lettre::address::AddressError> for MailError {
    fn from(e: lettre::address::AddressError) -> Self {
        MailError::Address(e.to_string())
    }
}

/// Mail service for sending transactional emails
#[derive(Clone)]
pub struct MailService {
    config: Arc<MailConfig>,
    transport: Option<Arc<AsyncSmtpTransport<Tokio1Executor>>>,
}

impl MailService {
    /// Create a new mail service
    pub fn new(config: MailConfig) -> Self {
        let transport = if config.dev_mode {
            None
        } else {
            let mut builder = AsyncSmtpTransport::<Tokio1Executor>::relay(&config.smtp_host)
                .expect("invalid SMTP host")
                .port(config.smtp_port);

            if let (Some(user), Some(pass)) = (&config.smtp_username, &config.smtp_password) {
                builder = builder.credentials(Credentials::new(user.clone(), pass.clone()));
            }

            Some(Arc::new(builder.build()))
        };

        Self {
            config: Arc::new(config),
            transport,
        }
    }

    /// Get config
    pub fn config(&self) -> &MailConfig {
        &self.config
    }

    /// Send an email
    async fn send(&self, to: &str, subject: &str, html_body: &str) -> Result<(), MailError> {
        if self.config.dev_mode {
            // Dev mode: just log the email
            warn!(
                to = to,
                subject = subject,
                "[DEV MAIL] Email not sent (dev mode). Content preview:\n{}",
                html_body.chars().take(200).collect::<String>()
            );
            return Ok(());
        }

        let from = format!("{} <{}>", self.config.from_name, self.config.from_email);
        let email = Message::builder()
            .from(from.parse()?)
            .to(to.parse()?)
            .subject(subject)
            .header(ContentType::TEXT_HTML)
            .body(html_body.to_string())
            .map_err(|e| MailError::Smtp(e.to_string()))?;

        if let Some(transport) = &self.transport {
            transport.send(email).await?;
            info!(to = to, subject = subject, "Email sent");
        } else {
            warn!(to = to, "No SMTP transport configured — email not sent");
        }

        Ok(())
    }

    /// Send email verification link
    pub async fn send_verification_email(
        &self,
        to: &str,
        name: &str,
        token: &str,
    ) -> Result<(), MailError> {
        let link = format!(
            "{}/auth/verify-email?token={}",
            self.config.app_base_url.trim_end_matches('/'),
            token
        );

        let html = format!(
            r#"<!DOCTYPE html>
<html><body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
<h2 style="color: #c79a3a;">مرحباً {}!</h2>
<p>شكراً لتسجيلك في NovaX. لتفعيل حسابك، اضغط على الرابط التالي:</p>
<p><a href="{}" style="display: inline-block; padding: 12px 28px; background: #c79a3a; color: #0f0f10; text-decoration: none; border-radius: 8px; font-weight: 600;">تأكيد البريد الإلكتروني</a></p>
<p>أو انسخ هذا الرابط في متصفحك:</p>
<p style="word-break: break-all; color: #666;">{}</p>
<hr style="border: none; border-top: 1px solid #ddd; margin: 24px 0;">
<p style="color: #999; font-size: 12px;">إذا لم تُنشئ هذا الحساب، تجاهل هذه الرسالة.<br>ينتهي الرابط خلال 24 ساعة.</p>
</body></html>"#,
            name, link, link
        );

        self.send(to, "تأكيد بريدك الإلكتروني — NovaX", &html).await
    }

    /// Send password reset link
    pub async fn send_password_reset_email(
        &self,
        to: &str,
        name: &str,
        token: &str,
    ) -> Result<(), MailError> {
        let link = format!(
            "{}/auth/reset-password?token={}",
            self.config.app_base_url.trim_end_matches('/'),
            token
        );

        let html = format!(
            r#"<!DOCTYPE html>
<html><body style="font-family: sans-serif; max-width: 600px; margin: 0 auto; padding: 20px;">
<h2 style="color: #c79a3a;">استعادة كلمة المرور</h2>
<p>مرحباً {}،</p>
<p>تلقينا طلباً لاستعادة كلمة مرورك في NovaX. اضغط على الرابط التالي لتعيين كلمة مرور جديدة:</p>
<p><a href="{}" style="display: inline-block; padding: 12px 28px; background: #c79a3a; color: #0f0f10; text-decoration: none; border-radius: 8px; font-weight: 600;">استعادة كلمة المرور</a></p>
<p>أو انسخ هذا الرابط:</p>
<p style="word-break: break-all; color: #666;">{}</p>
<hr style="border: none; border-top: 1px solid #ddd; margin: 24px 0;">
<p style="color: #999; font-size: 12px;">إذا لم تطلب استعادة كلمة المرور، تجاهل هذه الرسالة.<br>ينتهي الرابط خلال ساعة واحدة.</p>
</body></html>"#,
            name, link, link
        );

        self.send(to, "استعادة كلمة المرور — NovaX", &html).await
    }
}
