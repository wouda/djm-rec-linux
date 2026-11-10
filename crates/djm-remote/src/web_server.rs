use anyhow::Result;
use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::{Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::{Html, IntoResponse, Response};
use axum::routing::{get, post};
use axum::{Json, Router};
use djm_common::protocol::{ControlCommand, RecordingState, Telemetry};
use futures_util::{SinkExt, StreamExt};
use serde::Deserialize;
use std::net::SocketAddr;
use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::{mpsc, watch};
use tower_http::cors::CorsLayer;
use tracing::info;

use djm_common::prodj::ProDjTelemetry;

#[derive(Clone)]
pub struct WebState {
    pub auth_token: String,
    pub recordings_dir: PathBuf,
    pub telemetry_rx: watch::Receiver<Telemetry>,
    pub prodj_rx: watch::Receiver<Option<ProDjTelemetry>>,
    pub command_tx: mpsc::Sender<ControlCommand>,
    pub running: Arc<AtomicBool>,
}

#[derive(Deserialize)]
pub struct AuthQuery {
    pub token: Option<String>,
}

pub struct WebServer {
    bind_addr: String,
    state: WebState,
}

impl WebServer {
    pub fn new(
        bind_addr: String,
        auth_token: String,
        recordings_dir: PathBuf,
        telemetry_rx: watch::Receiver<Telemetry>,
        prodj_rx: watch::Receiver<Option<ProDjTelemetry>>,
        command_tx: mpsc::Sender<ControlCommand>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            bind_addr,
            state: WebState {
                auth_token,
                recordings_dir,
                telemetry_rx,
                prodj_rx,
                command_tx,
                running,
            },
        }
    }

    pub fn create_router(state: WebState) -> Router {
        let studio_state = crate::studio_api::StudioState::new(
            state.recordings_dir.clone(),
            state.auth_token.clone(),
            state.telemetry_rx.clone(),
            state.running.clone(),
        );
        let studio_router = crate::studio_api::create_studio_router(studio_state);

        Router::new()
            .route("/", get(handle_dashboard_html))
            .route("/ws", get(handle_websocket_upgrade))
            .route("/api/status", get(handle_api_status))
            .route("/api/prodj/status", get(handle_api_prodj_status))
            .route("/api/command", post(handle_api_command))
            .route("/api/audio/test-stream", get(handle_audio_test_stream))
            .route("/api/audio/test-stream.flac", get(handle_audio_test_stream))
            .route("/api/audio/live-stream", get(handle_audio_live_stream))
            .route("/api/audio/live-stream.mp3", get(handle_audio_live_stream))
            .layer(CorsLayer::permissive())
            .with_state(state)
            .merge(studio_router)
    }

    pub async fn run(&self) -> Result<()> {
        let app = Self::create_router(self.state.clone());

        let addr: SocketAddr = self.bind_addr.parse()?;
        info!("Web Remote Dashboard listening at http://{}", addr);

        let listener = tokio::net::TcpListener::bind(addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

async fn handle_dashboard_html() -> Html<&'static str> {
    Html(DASHBOARD_HTML)
}

async fn handle_api_status(
    State(state): State<WebState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<Telemetry>, StatusCode> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let mut t = state.telemetry_rx.borrow().clone();
    t.prodj = state.prodj_rx.borrow().clone();
    Ok(Json(t))
}

async fn handle_api_prodj_status(
    State(state): State<WebState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<Option<ProDjTelemetry>>, StatusCode> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let p = state.prodj_rx.borrow().clone();
    Ok(Json(p))
}

async fn handle_api_command(
    State(state): State<WebState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Json(cmd): Json<ControlCommand>,
) -> Result<StatusCode, StatusCode> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }
    let _ = state.command_tx.send(cmd).await;
    Ok(StatusCode::OK)
}

struct ProcessStream {
    stream: tokio_util::io::ReaderStream<tokio::process::ChildStdout>,
    _child: tokio::process::Child,
}

impl futures_util::Stream for ProcessStream {
    type Item = std::io::Result<axum::body::Bytes>;

    fn poll_next(
        mut self: std::pin::Pin<&mut Self>,
        cx: &mut std::task::Context<'_>,
    ) -> std::task::Poll<Option<Self::Item>> {
        std::pin::Pin::new(&mut self.stream).poll_next(cx)
    }
}

async fn handle_audio_test_stream(
    State(state): State<WebState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let t = state.telemetry_rx.borrow().clone();
    if t.state != RecordingState::Recording {
        return Err(StatusCode::CONFLICT);
    }

    let filename = match t.active_filename {
        Some(ref f) if !f.is_empty() => f.clone(),
        _ => return Err(StatusCode::CONFLICT),
    };

    let filepath = state.recordings_dir.join(&filename);
    if !filepath.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.kill_on_drop(true);
    cmd.args([
        "-sseof",
        "-15",
        "-i",
        filepath.to_str().unwrap_or(""),
        "-t",
        "15",
        "-af",
        "asetnsamples=4096",
        "-c:a",
        "flac",
        "-f",
        "flac",
        "-",
    ])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::piped());

    let output = cmd.output().await.map_err(|e| {
        tracing::error!("Failed to spawn ffmpeg for test-stream: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;

    if !output.status.success() || output.stdout.is_empty() {
        tracing::error!(
            "ffmpeg test-stream failed (exit {:?}): {}",
            output.status.code(),
            String::from_utf8_lossy(&output.stderr)
        );
        return Err(StatusCode::INTERNAL_SERVER_ERROR);
    }

    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "audio/flac")
        .header(axum::http::header::CONTENT_LENGTH, output.stdout.len().to_string())
        .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store")
        .header(axum::http::header::ACCEPT_RANGES, "bytes")
        .body(axum::body::Body::from(output.stdout))
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn handle_audio_live_stream(
    State(state): State<WebState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Response, StatusCode> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let t = state.telemetry_rx.borrow().clone();
    if t.state != RecordingState::Recording {
        return Err(StatusCode::CONFLICT);
    }

    let filename = match t.active_filename {
        Some(ref f) if !f.is_empty() => f.clone(),
        _ => return Err(StatusCode::CONFLICT),
    };

    let filepath = state.recordings_dir.join(&filename);
    if !filepath.exists() {
        return Err(StatusCode::NOT_FOUND);
    }

    // Follow the active file and stream continuous 192kbps MP3
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.kill_on_drop(true);

    cmd.args([
        "-follow",
        "1",
        "-re",
        "-i",
        filepath.to_str().unwrap_or(""),
        "-ar",
        "48000",
        "-c:a",
        "libmp3lame",
        "-b:a",
        "192k",
        "-f",
        "mp3",
        "-",
    ])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().map_err(|e| {
        tracing::error!("Failed to spawn ffmpeg for live-stream: {}", e);
        StatusCode::INTERNAL_SERVER_ERROR
    })?;
    let stdout = child.stdout.take().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let process_stream = ProcessStream {
        stream: tokio_util::io::ReaderStream::new(stdout),
        _child: child,
    };
    let body = axum::body::Body::from_stream(process_stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "audio/mpeg")
        .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store, must-revalidate")
        .header(axum::http::header::PRAGMA, "no-cache")
        .header("ice-audio-info", "ice-samplerate=48000;ice-bitrate=192;ice-channels=2")
        .body(body)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)
}

async fn handle_websocket_upgrade(
    ws: WebSocketUpgrade,
    State(state): State<WebState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Response {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return (StatusCode::UNAUTHORIZED, "Invalid Auth Token").into_response();
    }

    ws.on_upgrade(|socket| handle_websocket_session(socket, state))
}

async fn handle_websocket_session(socket: WebSocket, state: WebState) {
    let (mut sender, mut receiver) = socket.split();
    let mut tel_rx = state.telemetry_rx.clone();
    let mut prodj_rx = state.prodj_rx.clone();
    let cmd_tx = state.command_tx.clone();
    let running = state.running.clone();

    // Outbound stream: Telemetry JSON frames
    let send_task = async move {
        while running.load(Ordering::Relaxed) {
            tokio::select! {
                res = tel_rx.changed() => {
                    if res.is_err() { break; }
                }
                res = prodj_rx.changed() => {
                    if res.is_err() { break; }
                }
            }
            let mut telemetry = tel_rx.borrow().clone();
            telemetry.prodj = prodj_rx.borrow().clone();
            if let Ok(json) = serde_json::to_string(&telemetry) {
                if sender.send(Message::Text(json)).await.is_err() {
                    break;
                }
            }
            tokio::time::sleep(Duration::from_millis(16)).await; // up to 60 FPS
        }
    };

    // Inbound stream: Commands
    let recv_task = async move {
        while let Some(Ok(msg)) = receiver.next().await {
            if let Message::Text(text) = msg {
                if let Ok(cmd) = serde_json::from_str::<ControlCommand>(&text) {
                    let _ = cmd_tx.send(cmd).await;
                }
            }
        }
    };

    tokio::select! {
        _ = send_task => {},
        _ = recv_task => {},
    }
}

fn constant_time_eq(a: &[u8], b: &[u8]) -> bool {
    if a.len() != b.len() {
        return false;
    }
    let mut res = 0u8;
    for (x, y) in a.iter().zip(b.iter()) {
        res |= x ^ y;
    }
    res == 0
}

pub(crate) fn is_authorized(expected: &str, query_token: Option<&str>, headers: &HeaderMap) -> bool {
    let expected = expected.trim().as_bytes();
    if let Some(tok) = query_token {
        if constant_time_eq(tok.trim().as_bytes(), expected) {
            return true;
        }
    }
    if let Some(auth_header) = headers.get("authorization") {
        if let Ok(s) = auth_header.to_str() {
            if s.starts_with("Bearer ") {
                let tok = s.trim_start_matches("Bearer ").trim().as_bytes();
                if constant_time_eq(tok, expected) {
                    return true;
                }
            }
        }
    }
    false
}

const DASHBOARD_HTML: &str = include_str!("../assets/dashboard.html");

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    #[test]
    fn test_auth_check_token_query_and_header() {
        assert!(is_authorized("849201", Some("849201"), &HeaderMap::new()));
        assert!(!is_authorized("849201", Some("wrong"), &HeaderMap::new()));

        let mut headers = HeaderMap::new();
        headers.insert("authorization", "Bearer 849201".parse().unwrap());
        assert!(is_authorized("849201", None, &headers));
    }

    #[tokio::test]
    async fn test_dashboard_html_serves_studio_ui() {
        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let (_prodj_tx, prodj_rx) = watch::channel(None);
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        let req = Request::builder()
            .uri("/")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let html = String::from_utf8(body.to_vec()).unwrap();
        assert!(html.contains("Post-Event Mastering Studio"));
        assert!(html.contains("timetableText"));
        assert!(html.contains("usbClustersList"));
        assert!(html.contains("splitReviewBody"));
        assert!(html.contains("btnStartRender"));
        assert!(html.contains("renderProgressBar"));
    }

    #[tokio::test]
    async fn test_audio_stream_endpoint_unauthorized() {
        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let (_prodj_tx, prodj_rx) = watch::channel(None);
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        let req = Request::builder()
            .uri("/api/audio/test-stream?token=wrong")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_audio_stream_endpoint_conflict_when_not_recording() {
        let (_tel_tx, tel_rx) = watch::channel(Telemetry {
            state: RecordingState::ArmedSilent,
            ..Default::default()
        });
        let (_prodj_tx, prodj_rx) = watch::channel(None);
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        let req = Request::builder()
            .uri("/api/audio/test-stream?token=secret123")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_audio_live_stream_endpoint_unauthorized() {
        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let (_prodj_tx, prodj_rx) = watch::channel(None);
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        let req = Request::builder()
            .uri("/api/audio/live-stream?token=wrong")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    }

    #[tokio::test]
    async fn test_audio_live_stream_endpoint_conflict_when_not_recording() {
        let (_tel_tx, tel_rx) = watch::channel(Telemetry {
            state: RecordingState::ArmedSilent,
            ..Default::default()
        });
        let (_prodj_tx, prodj_rx) = watch::channel(None);
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        let req = Request::builder()
            .uri("/api/audio/live-stream?token=secret123")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);
    }

    #[tokio::test]
    async fn test_audio_stream_endpoint_recording_flac_success() {
        let tmp_dir = std::env::temp_dir();
        let test_flac = tmp_dir.join("test_recording_active.flac");

        // Generate a 20-second test FLAC file with ffmpeg if available
        let status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=1000:duration=20",
                "-c:a",
                "flac",
                test_flac.to_str().unwrap(),
            ])
            .output();

        if status.is_err() || !status.as_ref().unwrap().status.success() {
            // Skip test if ffmpeg is unavailable in test environment
            return;
        }

        let (_tel_tx, tel_rx) = watch::channel(Telemetry {
            state: RecordingState::Recording,
            active_filename: Some("test_recording_active.flac".to_string()),
            active_file_duration_secs: 20,
            ..Default::default()
        });
        let (_prodj_tx, prodj_rx) = watch::channel(None);
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: tmp_dir.clone(),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        let req = Request::builder()
            .uri("/api/audio/test-stream?token=secret123")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "audio/flac"
        );
        let content_length: usize = resp
            .headers()
            .get(axum::http::header::CONTENT_LENGTH)
            .unwrap()
            .to_str()
            .unwrap()
            .parse()
            .unwrap();
        assert!(content_length > 1000);

        let body_bytes = axum::body::to_bytes(resp.into_body(), 10 * 1024 * 1024)
            .await
            .unwrap();
        assert_eq!(body_bytes.len(), content_length);

        let _ = std::fs::remove_file(test_flac);
    }

    #[tokio::test]
    async fn test_audio_live_stream_endpoint_recording_mp3_success() {
        let tmp_dir = std::env::temp_dir();
        let test_flac = tmp_dir.join("test_recording_live.flac");

        let status = std::process::Command::new("ffmpeg")
            .args([
                "-y",
                "-f",
                "lavfi",
                "-i",
                "sine=frequency=1000:duration=5",
                "-c:a",
                "flac",
                test_flac.to_str().unwrap(),
            ])
            .output();

        if status.is_err() || !status.as_ref().unwrap().status.success() {
            return;
        }

        let (_tel_tx, tel_rx) = watch::channel(Telemetry {
            state: RecordingState::Recording,
            active_filename: Some("test_recording_live.flac".to_string()),
            active_file_duration_secs: 5,
            ..Default::default()
        });
        let (_prodj_tx, prodj_rx) = watch::channel(None);
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: tmp_dir.clone(),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        let req = Request::builder()
            .uri("/api/audio/live-stream?token=secret123")
            .body(Body::empty())
            .unwrap();

        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        assert_eq!(
            resp.headers()
                .get(axum::http::header::CONTENT_TYPE)
                .unwrap(),
            "audio/mpeg"
        );

        let _ = std::fs::remove_file(test_flac);
    }

    #[tokio::test]
    async fn test_prodj_status_and_degraded_mode() {
        use djm_common::prodj::ProDjDeckStatus;

        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let mut prodj = ProDjTelemetry::default();
        prodj.is_degraded = true;
        prodj.decks.push(ProDjDeckStatus {
            player_number: 1,
            is_on_air: true,
            is_playing: true,
            track_title: None, // degraded: unavailable
            track_artist: None,
            bpm: None,
            degraded: true,
            ..Default::default()
        });

        let (_prodj_tx, prodj_rx) = watch::channel(Some(prodj));
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
            prodj_rx,
            command_tx: cmd_tx,
            running,
        };

        let app = WebServer::create_router(state);

        // Check /api/prodj/status
        let req = Request::builder()
            .uri("/api/prodj/status?token=secret123")
            .body(Body::empty())
            .unwrap();

        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);
        let body = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await.unwrap();
        let p_res: Option<ProDjTelemetry> = serde_json::from_slice(&body).unwrap();
        assert!(p_res.is_some());
        let p = p_res.unwrap();
        assert!(p.is_degraded);
        assert_eq!(p.decks.len(), 1);
        assert_eq!(p.decks[0].track_title, None);

        // Check /api/status includes prodj
        let req2 = Request::builder()
            .uri("/api/status?token=secret123")
            .body(Body::empty())
            .unwrap();

        let resp2 = app.oneshot(req2).await.unwrap();
        assert_eq!(resp2.status(), StatusCode::OK);
        let body2 = axum::body::to_bytes(resp2.into_body(), 1024 * 1024).await.unwrap();
        let t_res: Telemetry = serde_json::from_slice(&body2).unwrap();
        assert!(t_res.prodj.is_some());
        assert!(t_res.prodj.unwrap().is_degraded);
    }
}
