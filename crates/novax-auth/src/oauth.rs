//! OAuth2 providers (Google + GitHub)
//!
//! Provides OAuth2 flows for social login. Configurable via OAuthConfig.

use serde::{Deserialize, Serialize};

use crate::AuthError;

/// OAuth provider configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthConfig {
    pub google: Option<OAuthProviderConfig>,
    pub github: Option<OAuthProviderConfig>,
    /// Redirect URL base (e.g. http://localhost:3000)
    pub redirect_base: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthProviderConfig {
    pub client_id: String,
    pub client_secret: String,
    pub enabled: bool,
}

impl Default for OAuthConfig {
    fn default() -> Self {
        Self {
            google: std::env::var("GOOGLE_OAUTH_CLIENT_ID").ok().map(|client_id| {
                OAuthProviderConfig {
                    client_id,
                    client_secret: std::env::var("GOOGLE_OAUTH_CLIENT_SECRET").unwrap_or_default(),
                    enabled: true,
                }
            }),
            github: std::env::var("GITHUB_OAUTH_CLIENT_ID").ok().map(|client_id| {
                OAuthProviderConfig {
                    client_id,
                    client_secret: std::env::var("GITHUB_OAUTH_CLIENT_SECRET").unwrap_or_default(),
                    enabled: true,
                }
            }),
            redirect_base: std::env::var("OAUTH_REDIRECT_BASE")
                .unwrap_or_else(|_| "http://localhost:3000".to_string()),
        }
    }
}

impl OAuthConfig {
    /// Check if Google OAuth is enabled
    pub fn google_enabled(&self) -> bool {
        self.google.as_ref().is_some_and(|c| c.enabled && !c.client_id.is_empty())
    }
    /// Check if GitHub OAuth is enabled
    pub fn github_enabled(&self) -> bool {
        self.github.as_ref().is_some_and(|c| c.enabled && !c.client_id.is_empty())
    }
    /// Is any OAuth provider enabled?
    pub fn any_enabled(&self) -> bool {
        self.google_enabled() || self.github_enabled()
    }
}

/// OAuth provider type
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum OAuthProvider {
    Google,
    Github,
}

impl OAuthProvider {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Google => "google",
            Self::Github => "github",
        }
    }

    pub fn from_str(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "google" => Some(Self::Google),
            "github" => Some(Self::Github),
            _ => None,
        }
    }

    /// Authorization URL (where user is redirected to grant access)
    pub fn auth_url(&self) -> &'static str {
        match self {
            Self::Google => "https://accounts.google.com/o/oauth2/v2/auth",
            Self::Github => "https://github.com/login/oauth/authorize",
        }
    }

    /// Token URL (where we exchange code for access token)
    pub fn token_url(&self) -> &'static str {
        match self {
            Self::Google => "https://oauth2.googleapis.com/token",
            Self::Github => "https://github.com/login/oauth/access_token",
        }
    }

    /// User info URL (where we fetch user profile with access token)
    pub fn user_info_url(&self) -> &'static str {
        match self {
            Self::Google => "https://www.googleapis.com/oauth2/v2/userinfo",
            Self::Github => "https://api.github.com/user",
        }
    }

    /// Default scopes
    pub fn default_scopes(&self) -> &'static str {
        match self {
            Self::Google => "openid email profile",
            Self::Github => "read:user user:email",
        }
    }
}

/// Build the authorization URL for redirecting the user
pub fn build_auth_url(
    provider: OAuthProvider,
    config: &OAuthConfig,
    state: &str,
) -> Result<String, AuthError> {
    let provider_config = match provider {
        OAuthProvider::Google => config.google.as_ref(),
        OAuthProvider::Github => config.github.as_ref(),
    }
    .ok_or_else(|| AuthError::Internal(format!("{} OAuth not configured", provider.as_str())))?;

    if !provider_config.enabled {
        return Err(AuthError::Internal(format!("{} OAuth disabled", provider.as_str())));
    }

    let redirect_uri = format!(
        "{}/auth/oauth/{}/callback",
        config.redirect_base.trim_end_matches('/'),
        provider.as_str()
    );

    let url = format!(
        "{}?client_id={}&redirect_uri={}&response_type=code&scope={}&state={}",
        provider.auth_url(),
        urlencoding::encode(&provider_config.client_id),
        urlencoding::encode(&redirect_uri),
        urlencoding::encode(provider.default_scopes()),
        urlencoding::encode(state),
    );

    Ok(url)
}

/// OAuth user info returned from the provider
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OAuthUserInfo {
    pub provider: OAuthProvider,
    pub provider_user_id: String,
    pub email: String,
    pub name: String,
    pub avatar_url: Option<String>,
}

/// Generate a random state for CSRF protection
pub fn generate_state() -> String {
    let bytes: [u8; 16] = rand::random();
    hex::encode(&bytes[..])
}

// Use a simple hex encoder (avoid adding another dependency)
mod hex {
    pub fn encode(bytes: &[u8]) -> String {
        bytes.iter().map(|b| format!("{:02x}", b)).collect()
    }
}

// Minimal URL encoding (avoid adding another dependency)
mod urlencoding {
    pub fn encode(s: &str) -> String {
        let mut result = String::new();
        for c in s.chars() {
            match c {
                'A'..='Z' | 'a'..='z' | '0'..='9' | '-' | '.' | '_' | '~' => result.push(c),
                _ => {
                    let mut buf = [0u8; 4];
                    let bytes = c.encode_utf8(&mut buf).as_bytes();
                    for b in bytes {
                        result.push_str(&format!("%{:02X}", b));
                    }
                }
            }
        }
        result
    }
}
