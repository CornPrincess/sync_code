//! Web server mode for sync-code.
//!
//! Exposes the same Rust commands as the Tauri app via HTTP + SSE streaming,
//! so Windows users who cannot install or run the desktop app can run:
//!
//!   cargo run --manifest-path src-tauri/Cargo.toml --bin web-server --features web
//!
//! and then open http://localhost:8080 in any browser.

use std::{
    convert::Infallible,
    future::Future,
    path::{Path, PathBuf},
    sync::Arc,
};

use axum::{
    body::Body,
    extract::{Query, State},
    http::StatusCode,
    response::IntoResponse,
    routing::{get, post},
    Json, Router,
};
use serde::Deserialize;
use tokio::sync::{mpsc, Mutex};
use tokio_stream::wrappers::ReceiverStream;
use tower_http::{
    cors::{Any, CorsLayer},
    services::{ServeDir, ServeFile},
};

use sync_code_lib::{
    commands::{
        git::{
            checkout_and_pull as git_checkout_and_pull,
            checkout_branch as git_checkout_branch,
            discard_changes,
            list_branches as git_list_branches,
            refresh_branches as git_refresh_branches,
            BranchList,
            LogFn,
        },
        sync::{do_sync, run_commit_and_push},
    },
    error::{AppError, Result as AppResult},
    models::{AppConfig, AuthConfig, ProxyConfig, SyncEvent},
};

// ---------------------------------------------------------------------------
// App state
// ---------------------------------------------------------------------------

#[derive(Clone)]
struct AppState {
    sync_lock: Arc<Mutex<bool>>,
}

// ---------------------------------------------------------------------------
// Config helpers (filesystem, no AppHandle)
// ---------------------------------------------------------------------------

fn web_config_path() -> PathBuf {
    let base: PathBuf = if cfg!(target_os = "windows") {
        std::env::var("APPDATA")
            .map(PathBuf::from)
            .unwrap_or_else(|_| PathBuf::from("."))
    } else if cfg!(target_os = "macos") {
        std::env::var("HOME")
            .map(|h| PathBuf::from(h).join("Library").join("Application Support"))
            .unwrap_or_else(|_| PathBuf::from("."))
    } else {
        std::env::var("XDG_CONFIG_HOME")
            .map(PathBuf::from)
            .unwrap_or_else(|_| {
                std::env::var("HOME")
                    .map(|h| PathBuf::from(h).join(".config"))
                    .unwrap_or_else(|_| PathBuf::from("."))
            })
    };
    base.join("sync-code").join("config.json")
}

fn load_config_web() -> Result<AppConfig, AppError> {
    let path = web_config_path();
    if !path.exists() {
        return Ok(AppConfig::default());
    }
    let data = std::fs::read_to_string(&path)?;
    Ok(serde_json::from_str(&data)?)
}

fn save_config_web(config: &AppConfig) -> Result<(), AppError> {
    let path = web_config_path();
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(&path, serde_json::to_string_pretty(config)?)?;
    Ok(())
}

// ---------------------------------------------------------------------------
// Streaming helper: run op(log) and pipe SyncEvent + final result as SSE
// ---------------------------------------------------------------------------
//
// Wire format (newline-delimited, each message ends with \n\n):
//   data: {"type":"log",    "payload":{...SyncEvent}}\n\n
//   data: {"type":"result", "payload":<T>}\n\n   ← success
//   data: {"type":"error",  "payload":"msg"}\n\n ← failure
//
async fn stream_op<F, Fut, T>(
    sync_lock: Arc<Mutex<bool>>,
    use_lock: bool,
    op: F,
) -> impl IntoResponse
where
    F: FnOnce(LogFn) -> Fut + Send + 'static,
    Fut: Future<Output = AppResult<T>> + Send + 'static,
    T: serde::Serialize + Send + 'static,
{
    if use_lock {
        let mut locked = sync_lock.lock().await;
        if *locked {
            return (StatusCode::CONFLICT, "Sync already in progress").into_response();
        }
        *locked = true;
    }

    // Channel: LogFn (sync) → SSE forwarder (async)
    let (event_tx, mut event_rx) = mpsc::unbounded_channel::<SyncEvent>();
    // Oneshot: operation result → SSE forwarder
    let (result_tx, result_rx) = tokio::sync::oneshot::channel::<AppResult<T>>();

    let log: LogFn = Arc::new(move |event: SyncEvent| {
        let _ = event_tx.send(event);
    });

    // Spawn operation task
    let lock_clone = sync_lock.clone();
    tokio::spawn(async move {
        let result = op(log).await;
        if use_lock {
            *lock_clone.lock().await = false;
        }
        let _ = result_tx.send(result);
    });

    // Body channel: SSE forwarder → HTTP response body
    let (body_tx, body_rx) = mpsc::channel::<Result<Vec<u8>, Infallible>>(256);

    tokio::spawn(async move {
        let mut result_rx = result_rx;
        loop {
            tokio::select! {
                Some(event) = event_rx.recv() => {
                    let line = format!(
                        "data: {}\n\n",
                        serde_json::json!({"type":"log","payload":event})
                    );
                    if body_tx.send(Ok(line.into_bytes())).await.is_err() { break; }
                }
                result = &mut result_rx => {
                    // Drain any remaining buffered log events first
                    while let Ok(event) = event_rx.try_recv() {
                        let line = format!(
                            "data: {}\n\n",
                            serde_json::json!({"type":"log","payload":event})
                        );
                        if body_tx.send(Ok(line.into_bytes())).await.is_err() { return; }
                    }
                    let json = match result {
                        Ok(Ok(v)) => serde_json::json!({"type":"result","payload":v}),
                        Ok(Err(e)) => {
                            let msg: String = e.to_string();
                            serde_json::json!({"type":"error","payload":msg})
                        },
                        Err(_) => serde_json::json!({"type":"error","payload":"operation dropped"}),
                    };
                    let line = format!("data: {json}\n\n");
                    let _ = body_tx.send(Ok(line.into_bytes())).await;
                    break;
                }
            }
        }
    });

    axum::http::Response::builder()
        .header("Content-Type", "text/event-stream")
        .header("Cache-Control", "no-cache")
        .header("X-Accel-Buffering", "no")
        .body(Body::from_stream(ReceiverStream::new(body_rx)))
        .unwrap()
        .into_response()
}

// ---------------------------------------------------------------------------
// Handlers — config
// ---------------------------------------------------------------------------

async fn handle_load_config() -> impl IntoResponse {
    match load_config_web() {
        Ok(c) => Json(c).into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

async fn handle_save_config(Json(config): Json<AppConfig>) -> impl IntoResponse {
    match save_config_web(&config) {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e.to_string()).into_response(),
    }
}

// ---------------------------------------------------------------------------
// Handlers — sync
// ---------------------------------------------------------------------------

async fn handle_start_sync(
    State(state): State<AppState>,
    Json(config): Json<AppConfig>,
) -> impl IntoResponse {
    stream_op(state.sync_lock, true, move |log| async move {
        do_sync(&log, &config).await
    })
    .await
}

#[derive(Deserialize)]
struct CommitBody {
    config: AppConfig,
    commit_message: String,
    paths_to_stage: Vec<String>,
}

async fn handle_commit_and_push(
    State(state): State<AppState>,
    Json(body): Json<CommitBody>,
) -> impl IntoResponse {
    stream_op(state.sync_lock, false, move |log| async move {
        run_commit_and_push(&log, &body.config, body.commit_message, body.paths_to_stage).await
    })
    .await
}

async fn handle_discard_sync(Json(config): Json<AppConfig>) -> impl IntoResponse {
    let path_a = PathBuf::from(&config.repo_a.local_path);
    match discard_changes(&path_a).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => {
            let msg: String = e.to_string();
            (StatusCode::INTERNAL_SERVER_ERROR, msg).into_response()
        }
    }
}

// ---------------------------------------------------------------------------
// Handlers — branches
// ---------------------------------------------------------------------------

#[derive(Deserialize)]
struct ListBranchesQuery {
    path: String,
}

async fn handle_list_branches(Query(q): Query<ListBranchesQuery>) -> impl IntoResponse {
    Json::<BranchList>(git_list_branches(q.path).await).into_response()
}

#[derive(Deserialize)]
struct RefreshBranchesBody {
    local_path: String,
    proxy: Option<ProxyConfig>,
}

async fn handle_refresh_branches(
    State(_state): State<AppState>,
    Json(body): Json<RefreshBranchesBody>,
) -> impl IntoResponse {
    Json::<BranchList>(git_refresh_branches(body.local_path, body.proxy).await).into_response()
}

#[derive(Deserialize)]
struct CheckoutBody {
    local_path: String,
    branch: String,
}

async fn handle_checkout_branch(Json(body): Json<CheckoutBody>) -> impl IntoResponse {
    match git_checkout_branch(body.local_path, body.branch).await {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => { let m: String = e.to_string(); (StatusCode::INTERNAL_SERVER_ERROR, m).into_response() }
    }
}

#[derive(Deserialize)]
struct CheckoutPullBody {
    local_path: String,
    branch: String,
    remote_url: String,
    auth: AuthConfig,
    proxy: ProxyConfig,
    platform: String,
}

async fn handle_checkout_and_pull(Json(body): Json<CheckoutPullBody>) -> impl IntoResponse {
    match git_checkout_and_pull(
        body.local_path,
        body.branch,
        body.remote_url,
        body.auth,
        body.proxy,
        body.platform,
    )
    .await
    {
        Ok(()) => StatusCode::NO_CONTENT.into_response(),
        Err(e) => { let m: String = e.to_string(); (StatusCode::INTERNAL_SERVER_ERROR, m).into_response() }
    }
}

// ---------------------------------------------------------------------------
// Locate pre-built frontend dist/ directory
// ---------------------------------------------------------------------------

fn find_dist() -> PathBuf {
    for candidate in &["dist", "../dist"] {
        let p = Path::new(candidate);
        if p.join("index.html").exists() {
            return p.to_path_buf();
        }
    }
    PathBuf::from("dist") // fallback; tower-http will return 404 for missing files
}

// ---------------------------------------------------------------------------
// main
// ---------------------------------------------------------------------------

#[tokio::main]
async fn main() {
    let dist = find_dist();

    if !dist.join("index.html").exists() {
        eprintln!("ERROR: Frontend not found at '{}' or '../{0}'.", dist.display());
        eprintln!("Please run `npm run build` from the repository root first.");
        std::process::exit(1);
    }

    let state = AppState {
        sync_lock: Arc::new(Mutex::new(false)),
    };

    let cors = CorsLayer::new()
        .allow_origin(Any)
        .allow_methods(Any)
        .allow_headers(Any);

    let api = Router::new()
        .route("/config", get(handle_load_config).post(handle_save_config))
        .route("/sync/start", post(handle_start_sync))
        .route("/sync/commit", post(handle_commit_and_push))
        .route("/sync/discard", post(handle_discard_sync))
        .route("/branches", get(handle_list_branches))
        .route("/branches/refresh", post(handle_refresh_branches))
        .route("/branches/checkout", post(handle_checkout_branch))
        .route("/branches/checkout-pull", post(handle_checkout_and_pull));

    let serve_dir =
        ServeDir::new(&dist).fallback(ServeFile::new(dist.join("index.html")));

    let app = Router::new()
        .nest("/api", api)
        .fallback_service(serve_dir)
        .layer(cors)
        .with_state(state);

    println!("sync-code web server listening at http://localhost:8080");
    println!("Serving frontend from: {}", std::fs::canonicalize(&dist).unwrap_or(dist).display());
    println!("Press Ctrl+C to stop.\n");

    open_browser("http://localhost:8080");

    let listener = tokio::net::TcpListener::bind("127.0.0.1:8080").await.unwrap();
    axum::serve(listener, app).await.unwrap();
}

fn open_browser(url: &str) {
    #[cfg(target_os = "windows")]
    { let _ = std::process::Command::new("cmd").args(["/c", "start", url]).spawn(); }
    #[cfg(target_os = "macos")]
    { let _ = std::process::Command::new("open").arg(url).spawn(); }
    #[cfg(target_os = "linux")]
    { let _ = std::process::Command::new("xdg-open").arg(url).spawn(); }
}
