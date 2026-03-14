use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// "none" | "userpass" | "token" | "ssh"
    pub auth_type: String,
    pub username: String,
    pub password: String,
    /// Personal / OAuth2 access token (used when auth_type == "token")
    #[serde(default)]
    pub token: String,
    pub ssh_key_path: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            auth_type: "none".into(),
            username: String::new(),
            password: String::new(),
            token: String::new(),
            ssh_key_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoConfig {
    pub local_path: String,
    pub remote_url: String,
    pub branch: String,
    /// Missing in old config files → deserialize as Default (auth_type "none")
    #[serde(default)]
    pub auth: AuthConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub http_proxy: String,
    pub https_proxy: String,
    pub no_proxy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub repo_a: RepoConfig,
    pub repo_b: RepoConfig,
    /// Missing in old config files → deserialize as Default (disabled)
    #[serde(default)]
    pub proxy: ProxyConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncEvent {
    pub level: String,   // "info" | "warn" | "error" | "success"
    pub message: String,
    pub timestamp: String, // "HH:MM:SS"
}

impl SyncEvent {
    fn now() -> String {
        chrono::Local::now().format("%H:%M:%S").to_string()
    }

    pub fn info(msg: impl Into<String>) -> Self {
        Self { level: "info".into(), message: msg.into(), timestamp: Self::now() }
    }

    #[allow(dead_code)]
    pub fn warn(msg: impl Into<String>) -> Self {
        Self { level: "warn".into(), message: msg.into(), timestamp: Self::now() }
    }

    #[allow(dead_code)]
    pub fn error(msg: impl Into<String>) -> Self {
        Self { level: "error".into(), message: msg.into(), timestamp: Self::now() }
    }

    pub fn success(msg: impl Into<String>) -> Self {
        Self { level: "success".into(), message: msg.into(), timestamp: Self::now() }
    }
}
