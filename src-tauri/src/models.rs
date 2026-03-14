use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AuthConfig {
    /// "none" | "userpass" | "ssh"
    pub auth_type: String,
    pub username: String,
    pub password: String,
    pub ssh_key_path: String,
}

impl Default for AuthConfig {
    fn default() -> Self {
        Self {
            auth_type: "none".into(),
            username: String::new(),
            password: String::new(),
            ssh_key_path: String::new(),
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoConfig {
    pub local_path: String,
    pub remote_url: String,
    pub branch: String,
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
