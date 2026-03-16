use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize)]
pub struct FileChange {
    /// "added" | "modified" | "deleted" | "renamed" | "copied" | "unknown"
    pub status: String,
    pub path: String,
    /// Only set for renamed / copied files
    pub old_path: Option<String>,
}

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
    /// Hosting platform: "github" | "gitlab" | "codeup" (default "github")
    #[serde(default = "default_platform")]
    pub platform: String,
    /// When true, use zip_path as source instead of git pull (Repo B only)
    #[serde(default)]
    pub use_zip: bool,
    /// Absolute path to the zip file when use_zip is true
    #[serde(default)]
    pub zip_path: String,
    /// "local" | "api" — which ZIP source tab is selected (frontend only, persisted for UX)
    #[serde(default = "default_zip_source_mode")]
    pub zip_source_mode: String,
}

fn default_platform() -> String {
    "github".into()
}

fn default_zip_source_mode() -> String {
    "local".into()
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct ProxyConfig {
    pub enabled: bool,
    pub http_proxy: String,
    pub https_proxy: String,
    pub no_proxy: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoPreset {
    pub id: String,
    pub name: String,
    pub config: RepoConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub repo_a: RepoConfig,
    pub repo_b: RepoConfig,
    /// Missing in old config files → deserialize as Default (disabled)
    #[serde(default)]
    pub proxy: ProxyConfig,
    /// Saved presets for Repo A (target)
    #[serde(default)]
    pub repo_a_presets: Vec<RepoPreset>,
    /// Saved presets for Repo B (source)
    #[serde(default)]
    pub repo_b_presets: Vec<RepoPreset>,
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
