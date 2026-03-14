use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct RepoConfig {
    pub local_path: String,
    pub remote_url: String,
    pub branch: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, Default)]
pub struct AppConfig {
    pub repo_a: RepoConfig,
    pub repo_b: RepoConfig,
}

#[derive(Debug, Clone, Serialize)]
pub struct SyncEvent {
    pub level: String,   // "info" | "warn" | "error" | "success"
    pub message: String,
}

impl SyncEvent {
    pub fn info(msg: impl Into<String>) -> Self {
        Self { level: "info".into(), message: msg.into() }
    }

    #[allow(dead_code)]
    pub fn warn(msg: impl Into<String>) -> Self {
        Self { level: "warn".into(), message: msg.into() }
    }

    #[allow(dead_code)]
    pub fn error(msg: impl Into<String>) -> Self {
        Self { level: "error".into(), message: msg.into() }
    }

    pub fn success(msg: impl Into<String>) -> Self {
        Self { level: "success".into(), message: msg.into() }
    }
}
