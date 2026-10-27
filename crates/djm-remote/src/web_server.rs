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

#[derive(Clone)]
pub struct WebState {
    pub auth_token: String,
    pub recordings_dir: PathBuf,
    pub telemetry_rx: watch::Receiver<Telemetry>,
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
        command_tx: mpsc::Sender<ControlCommand>,
        running: Arc<AtomicBool>,
    ) -> Self {
        Self {
            bind_addr,
            state: WebState {
                auth_token,
                recordings_dir,
                telemetry_rx,
                command_tx,
                running,
            },
        }
    }

    pub fn create_router(state: WebState) -> Router {
        Router::new()
            .route("/", get(handle_dashboard_html))
            .route("/ws", get(handle_websocket_upgrade))
            .route("/api/status", get(handle_api_status))
            .route("/api/command", post(handle_api_command))
            .route("/api/audio/test-stream", get(handle_audio_test_stream))
            .route("/api/audio/live-stream", get(handle_audio_live_stream))
            .layer(CorsLayer::permissive())
            .with_state(state)
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
    let t = state.telemetry_rx.borrow().clone();
    Ok(Json(t))
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

    // Spawn ffmpeg to read the last 15 seconds from the active FLAC file and pipe out 24-bit FLAC
    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.kill_on_drop(true);
    cmd.args([
        "-sseof",
        "-15",
        "-i",
        filepath.to_str().unwrap_or(""),
        "-t",
        "15",
        "-c:a",
        "flac",
        "-f",
        "flac",
        "-",
    ])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let stdout = child.stdout.take().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let stream = tokio_util::io::ReaderStream::new(stdout);
    let body = axum::body::Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "audio/flac")
        .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store")
        .body(body)
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

    let mut child = cmd.spawn().map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;
    let stdout = child.stdout.take().ok_or(StatusCode::INTERNAL_SERVER_ERROR)?;

    let stream = tokio_util::io::ReaderStream::new(stdout);
    let body = axum::body::Body::from_stream(stream);

    Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "audio/mpeg")
        .header(axum::http::header::CACHE_CONTROL, "no-cache, no-store")
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
    let cmd_tx = state.command_tx.clone();
    let running = state.running.clone();

    // Outbound stream: Telemetry JSON frames
    let send_task = async move {
        while running.load(Ordering::Relaxed) {
            if tel_rx.changed().await.is_ok() {
                let telemetry = tel_rx.borrow().clone();
                if let Ok(json) = serde_json::to_string(&telemetry) {
                    if let Err(_) = sender.send(Message::Text(json)).await {
                        break;
                    }
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

fn is_authorized(expected: &str, query_token: Option<&str>, headers: &HeaderMap) -> bool {
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

const DASHBOARD_HTML: &str = r##"<!DOCTYPE html>
<html lang="en">
<head>
<meta charset="utf-8"/>
<meta name="viewport" content="width=device-width, initial-scale=1.0, maximum-scale=1.0, user-scalable=no"/>
<title>djm-box Remote Booth Monitor</title>
<style>
  :root {
    --bg: #090a0f;
    --card: #131722;
    --border: #232a3b;
    --text: #e2e8f0;
    --accent: #00f0ff;
    --green: #00ff66;
    --yellow: #ffd000;
    --red: #ff3344;
  }
  * { box-sizing: border-box; margin: 0; padding: 0; font-family: -apple-system, BlinkMacSystemFont, "Segoe UI", Roboto, monospace; }
  body { background: var(--bg); color: var(--text); padding: 12px; display: flex; flex-direction: column; gap: 12px; min-height: 100vh; }
  .card { background: var(--card); border: 1px solid var(--border); border-radius: 12px; padding: 16px; }
  .header { display: flex; justify-content: space-between; align-items: center; }
  .badge { font-weight: 700; font-size: 0.8rem; padding: 6px 12px; border-radius: 20px; text-transform: uppercase; white-space: nowrap; }
  .badge.mixer-on { background: #16a34a; color: #fff; }
  .badge.mixer-off { background: #dc2626; color: #fff; animation: pulse 2s infinite; }
  .badge.recording { background: var(--green); color: #000; animation: pulse 1.5s infinite; }
  .badge.armed { background: var(--yellow); color: #000; }
  .badge.disconnected { background: #334155; color: #94a3b8; }
  @keyframes pulse { 0% { opacity: 1; } 50% { opacity: 0.7; } 100% { opacity: 1; } }
  .mixer-info { font-size: 0.9rem; color: #94a3b8; }
  .meters { display: flex; flex-direction: column; gap: 12px; }
  .meter-label { display: flex; justify-content: space-between; font-size: 0.85rem; font-weight: 600; margin-bottom: 4px; }
  .bar-container { background: #000; height: 28px; border-radius: 6px; overflow: hidden; position: relative; border: 1px solid var(--border); }
  .bar-fill { height: 100%; width: 0%; transition: width 0.05s ease-out; background: linear-gradient(90deg, #00ff66 0%, #ffd000 68%, #ff3344 95%); }
  .ref-line { position: absolute; left: 68.3%; top: 0; bottom: 0; width: 2px; background: #fff; box-shadow: 0 0 6px #fff; z-index: 2; }
  .ref-text { position: absolute; left: 68.5%; top: 4px; font-size: 0.7rem; color: #fff; transform: translateX(-50%); font-weight: 700; z-index: 3; text-shadow: 0 1px 2px #000; }
  .stats-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; font-size: 0.9rem; }
  .stat-item { background: #0a0d14; padding: 10px; border-radius: 8px; border: 1px solid #1a2233; }
  .stat-val { font-size: 1.05rem; font-weight: 700; color: var(--accent); margin-top: 4px; }
  .btn-grid { display: grid; grid-template-columns: 1fr 1fr; gap: 10px; margin-top: 6px; }
  button { background: #1e2638; color: #fff; border: 1px solid var(--border); padding: 14px; border-radius: 10px; font-weight: 700; font-size: 0.95rem; cursor: pointer; transition: all 0.1s; }
  button:active { transform: scale(0.97); background: var(--accent); color: #000; }
  button:disabled { opacity: 0.4; cursor: not-allowed; }
  .btn-test { background: #16a34a; display: flex; align-items: center; justify-content: center; gap: 8px; font-weight: 700; }
  .btn-test:disabled { opacity: 0.45; cursor: not-allowed; background: #1e2638; color: #94a3b8; }
  .clip-alert { display: none; background: var(--red); color: #fff; font-weight: 700; text-align: center; padding: 8px; border-radius: 8px; margin-top: 8px; }
  .alert-banner { display: none; background: #450a0a; border: 1px solid #ef4444; color: #fecaca; padding: 12px; border-radius: 8px; font-weight: 600; font-size: 0.9rem; text-align: center; }
  .alert-banner.offline { background: #451a03; border-color: #f59e0b; color: #fef3c7; }
</style>
</head>
<body>
  <div class="card header">
    <div>
      <h2 style="font-size: 1.25rem; letter-spacing: -0.5px; display: flex; align-items: center; gap: 8px;">
        djm-box remote <span style="font-size: 0.75rem; background: #1e2638; border: 1px solid var(--border); padding: 2px 6px; border-radius: 4px; color: #94a3b8;">v0.2.4</span>
      </h2>
      <div id="mixerModel" class="mixer-info" style="margin-top: 4px;">Connecting to daemon...</div>
    </div>
    <div style="display: flex; flex-direction: column; gap: 6px; align-items: flex-end;">
      <div id="mixerBadge" class="badge mixer-off">Checking Mixer...</div>
      <div id="recBadge" class="badge disconnected">Offline</div>
    </div>
  </div>

  <div id="serverOfflineBanner" class="alert-banner offline">
    ⚠️ <strong>Appliance Disconnected</strong> &mdash; Reconnecting to djm-box on Wi-Fi...
  </div>

  <div id="mixerAlertBanner" class="alert-banner">
    ⚠️ <strong>No Pioneer DJM Mixer Detected</strong> &mdash; Connect mixer via USB-B cable to begin recording.
  </div>

  <div class="card meters">
    <div>
      <div class="meter-label"><span>LEFT (Master + Mic)</span><span id="txtPeakL">-∞ dBFS</span></div>
      <div class="bar-container">
        <div class="ref-line"></div>
        <div class="ref-text">0dB MIXER</div>
        <div id="barL" class="bar-fill"></div>
      </div>
    </div>
    <div>
      <div class="meter-label"><span>RIGHT (Master + Mic)</span><span id="txtPeakR">-∞ dBFS</span></div>
      <div class="bar-container">
        <div class="ref-line"></div>
        <div class="ref-text">0dB MIXER</div>
        <div id="barR" class="bar-fill"></div>
      </div>
    </div>
    <div id="clipAlert" class="clip-alert">⚠️ DIGITAL CLIPPING DETECTED</div>
  </div>

  <!-- Dedicated Dual Audio Monitor Card: 15s Test (FLAC) & Continuous Live (MP3) -->
  <div class="card" id="testAudioCard">
    <div style="display: flex; justify-content: space-between; align-items: center; margin-bottom: 8px;">
      <span style="font-weight: 700; font-size: 0.9rem; display: flex; align-items: center; gap: 6px;">
        🎧 Audio Monitor
      </span>
      <span id="testCountdownText" style="font-size: 0.8rem; font-weight: 700; color: var(--accent);">Ready</span>
    </div>
    <div id="testProgressBarContainer" style="display: none; background: #000; height: 6px; border-radius: 3px; overflow: hidden; margin-bottom: 10px; border: 1px solid var(--border);">
      <div id="testProgressFill" style="background: var(--accent); height: 100%; width: 0%; transition: width 0.1s linear;"></div>
    </div>
    <div style="display: grid; grid-template-columns: 1fr 1fr; gap: 10px;">
      <button id="btnTestAudio" class="btn-test" onclick="toggleTestAudio()" disabled>
        ▶ Test (15s FLAC)
      </button>
      <button id="btnLiveAudio" class="btn-test" style="background: #0284c7;" onclick="toggleLiveAudio()" disabled>
        🔴 Live Stream (MP3)
      </button>
    </div>
  </div>

  <div class="card">
    <div class="stats-grid">
      <div class="stat-item" style="grid-column: 1 / -1;"><div>Active File</div><div id="statFile" class="stat-val" style="font-size: 0.85rem; word-break: break-all;">No active file</div></div>
      <div class="stat-item"><div>Duration</div><div id="statTimer" class="stat-val">00:00:00</div></div>
      <div class="stat-item"><div>File Size</div><div id="statSize" class="stat-val">0 MB</div></div>
      <div class="stat-item"><div>Hardware Pad</div><div id="statPad" class="stat-val">-19 dB</div></div>
      <div class="stat-item"><div>Mic Routing</div><div class="stat-val" style="color: #c084fc;">REC OUT (+Mic)</div></div>
      <div class="stat-item"><div>Free Storage</div><div id="statDisk" class="stat-val">-- GB</div></div>
      <div class="stat-item"><div>Time Remaining</div><div id="statHours" class="stat-val">-- hrs</div></div>
    </div>
  </div>

  <div class="card">
    <div class="btn-grid">
      <button onclick="sendCmd({DropBookmark: null})">📍 + Bookmark</button>
      <button onclick="sendCmd('SplitFile')">✂️ Split File</button>
      <button onclick="sendCmd({SetHardwarePad: -19})">🛡️ Lock -19dB</button>
      <button onclick="sendCmd('ResetPeaks')">🔄 Reset Peaks</button>
    </div>
  </div>

<script>
  let token = new URLSearchParams(window.location.search).get('token') || localStorage.getItem('djm_token') || '';
  if (!token) {
    token = prompt('Enter djm-box 6-digit PIN:') || '';
    if (token) localStorage.setItem('djm_token', token);
  }

  let ws;
  let audioEl = null;
  let activeAudioMode = null;
  let testCountdownTimer = null;
  let liveTimer = null;
  let liveSeconds = 0;
  let isConnected = false;

  function connectWS() {
    const proto = window.location.protocol === 'https:' ? 'wss:' : 'ws:';
    ws = new WebSocket(`${proto}//${window.location.host}/ws?token=${encodeURIComponent(token)}`);
    
    ws.onopen = () => {
      isConnected = true;
      document.getElementById('serverOfflineBanner').style.display = 'none';
      setButtonsEnabled(true);
    };

    ws.onmessage = (e) => {
      const t = JSON.parse(e.data);
      updateUI(t);
    };

    ws.onclose = () => {
      isConnected = false;
      document.getElementById('serverOfflineBanner').style.display = 'block';
      document.getElementById('recBadge').textContent = 'Offline';
      document.getElementById('recBadge').className = 'badge disconnected';
      setButtonsEnabled(false);
      stopAudio();
      setTimeout(connectWS, 1500);
    };
  }
  connectWS();

  function setButtonsEnabled(enabled) {
    const buttons = document.querySelectorAll('.btn-grid button');
    buttons.forEach(b => {
      b.disabled = !enabled;
      b.style.opacity = enabled ? '1' : '0.4';
    });
  }

  function toggleTestAudio() {
    if (activeAudioMode === 'test') {
      stopAudio();
      return;
    }
    stopAudio();

    const btn = document.getElementById('btnTestAudio');
    const fill = document.getElementById('testProgressFill');
    const pContainer = document.getElementById('testProgressBarContainer');
    const countText = document.getElementById('testCountdownText');

    audioEl = new Audio(`/api/audio/test-stream?token=${encodeURIComponent(token)}`);
    audioEl.play().catch(err => {
      alert("Audio stream playback failed: " + err);
      stopAudio();
    });

    activeAudioMode = 'test';
    btn.textContent = "⏹ Stop (15s)";
    btn.style.background = "#dc2626";
    pContainer.style.display = "block";
    fill.style.width = "0%";
    countText.textContent = `Testing 24-bit FLAC... 15s`;

    const startTs = Date.now();
    testCountdownTimer = setInterval(() => {
      const elapsed = (Date.now() - startTs) / 1000;
      const left = Math.max(0, 15 - elapsed);
      fill.style.width = `${Math.min(100, (elapsed / 15) * 100)}%`;
      countText.textContent = `Testing 24-bit FLAC... ${Math.ceil(left)}s`;

      if (left <= 0) {
        stopAudio();
      }
    }, 100);

    audioEl.onended = () => stopAudio();
    audioEl.onerror = () => stopAudio();
  }

  function toggleLiveAudio() {
    if (activeAudioMode === 'live') {
      stopAudio();
      return;
    }
    stopAudio();

    const btn = document.getElementById('btnLiveAudio');
    const countText = document.getElementById('testCountdownText');

    audioEl = new Audio(`/api/audio/live-stream?token=${encodeURIComponent(token)}`);
    audioEl.play().catch(err => {
      alert("Live stream failed: " + err);
      stopAudio();
    });

    activeAudioMode = 'live';
    btn.textContent = "⏹ Stop Live";
    btn.style.background = "#dc2626";
    liveSeconds = 0;
    countText.textContent = `🔴 LIVE (00:00)`;

    liveTimer = setInterval(() => {
      liveSeconds++;
      const mm = String(Math.floor(liveSeconds / 60)).padStart(2, '0');
      const ss = String(liveSeconds % 60).padStart(2, '0');
      countText.textContent = `🔴 LIVE (${mm}:${ss})`;
    }, 1000);

    audioEl.onerror = () => stopAudio();
  }

  function stopAudio() {
    if (testCountdownTimer) {
      clearInterval(testCountdownTimer);
      testCountdownTimer = null;
    }
    if (liveTimer) {
      clearInterval(liveTimer);
      liveTimer = null;
    }
    if (audioEl) {
      audioEl.pause();
      audioEl.src = "";
      audioEl = null;
    }
    activeAudioMode = null;

    const btnTest = document.getElementById('btnTestAudio');
    const btnLive = document.getElementById('btnLiveAudio');
    const pContainer = document.getElementById('testProgressBarContainer');
    const countText = document.getElementById('testCountdownText');

    if (btnTest) {
      btnTest.textContent = "▶ Test (15s FLAC)";
      btnTest.style.background = "#16a34a";
    }
    if (btnLive) {
      btnLive.textContent = "🔴 Live Stream (MP3)";
      btnLive.style.background = "#0284c7";
    }
    if (pContainer) pContainer.style.display = "none";
    if (countText) countText.textContent = "Ready";
  }

  function sendCmd(cmd) {
    if (ws && ws.readyState === WebSocket.OPEN) {
      ws.send(JSON.stringify(cmd));
      navigator.vibrate && navigator.vibrate(30);
    }
  }

  function cdbToPct(cdb) {
    const db = cdb / 100.0;
    if (db <= -60.0) return 0;
    if (db >= 0.0) return 100;
    return ((db + 60.0) / 60.0) * 100;
  }

  function updateUI(t) {
    const isMixer = t.state !== 'Disconnected' && t.mixer_model && t.mixer_model !== 'No Mixer';

    // Mixer Name / Sample Rate
    if (isMixer) {
      document.getElementById('mixerModel').textContent = `${t.mixer_model} @ ${t.sample_rate || 96000} Hz`;
    } else {
      document.getElementById('mixerModel').textContent = 'Waiting for USB-B mixer connection...';
    }

    // Mixer Badge & Banner (only display mixer unplugged warning when server is connected)
    const mBadge = document.getElementById('mixerBadge');
    const mBanner = document.getElementById('mixerAlertBanner');
    if (isMixer) {
      mBadge.textContent = '✓ MIXER: ' + t.mixer_model;
      mBadge.className = 'badge mixer-on';
      if (mBanner) mBanner.style.display = 'none';
    } else {
      mBadge.textContent = '✗ NO MIXER (Plug USB-B)';
      mBadge.className = 'badge mixer-off';
      if (mBanner) mBanner.style.display = isConnected ? 'block' : 'none';
    }

    // Recording Badge
    const rBadge = document.getElementById('recBadge');
    if (t.state === 'Recording') {
      rBadge.textContent = '● RECORDING TO DISK';
      rBadge.className = 'badge recording';
    } else if (t.state === 'ArmedSilent') {
      rBadge.textContent = '⏸ ARMED IN RAM (> -65dB)';
      rBadge.className = 'badge armed';
    } else if (t.state === 'Paused') {
      rBadge.textContent = '⏸ PAUSED';
      rBadge.className = 'badge armed';
    } else {
      rBadge.textContent = '○ IDLE (Not Recording)';
      rBadge.className = 'badge disconnected';
    }

    // Audio monitor buttons disabled state if not actively recording
    const btnTest = document.getElementById('btnTestAudio');
    const btnLive = document.getElementById('btnLiveAudio');
    const countText = document.getElementById('testCountdownText');
    if (!activeAudioMode) {
      const canPlay = t.state === 'Recording';
      if (btnTest) {
        btnTest.disabled = !canPlay;
        btnTest.style.opacity = canPlay ? '1' : '0.45';
      }
      if (btnLive) {
        btnLive.disabled = !canPlay;
        btnLive.style.opacity = canPlay ? '1' : '0.45';
      }
      if (countText) {
        countText.textContent = canPlay ? "Ready" : (t.state === 'ArmedSilent' ? 'Armed in RAM (> -65dB)' : 'Idle');
      }
    }

    // VU Meters
    const pctL = cdbToPct(t.peak_l_centi_db);
    const pctR = cdbToPct(t.peak_r_centi_db);
    document.getElementById('barL').style.width = pctL + '%';
    document.getElementById('barR').style.width = pctR + '%';
    document.getElementById('txtPeakL').textContent = (t.peak_l_centi_db / 100.0).toFixed(1) + ' dBFS';
    document.getElementById('txtPeakR').textContent = (t.peak_r_centi_db / 100.0).toFixed(1) + ' dBFS';

    // Clips
    const clipBox = document.getElementById('clipAlert');
    if (t.clip_count > 0) {
      clipBox.style.display = 'block';
      clipBox.textContent = `⚠️ DIGITAL CLIPS: ${t.clip_count}`;
    } else {
      clipBox.style.display = 'none';
    }

    // File target
    const fileElem = document.getElementById('statFile');
    if (fileElem) {
      if (t.active_filename) {
        fileElem.textContent = t.active_filename;
        fileElem.style.color = 'var(--green)';
      } else if (isMixer) {
        fileElem.textContent = 'Armed in RAM (Waiting for music to start file)';
        fileElem.style.color = 'var(--yellow)';
      } else {
        fileElem.textContent = 'No active file';
        fileElem.style.color = '#94a3b8';
      }
    }

    // Stats
    const s = t.active_file_duration_secs || 0;
    const hh = String(Math.floor(s / 3600)).padStart(2, '0');
    const mm = String(Math.floor((s % 3600) / 60)).padStart(2, '0');
    const ss = String(s % 60).padStart(2, '0');
    document.getElementById('statTimer').textContent = `${hh}:${mm}:${ss}`;
    document.getElementById('statSize').textContent = `${t.active_file_size_mb || 0} MB`;
    document.getElementById('statPad').textContent = `${t.hw_pad_db} dB`;
    const freeGb = (t.free_disk_gb_tenths || 0) / 10.0;
    document.getElementById('statDisk').textContent = `${freeGb.toFixed(1)} GB`;
    const hrsElem = document.getElementById('statHours');
    if (hrsElem) {
      const hrs = (freeGb / 1.1).toFixed(0);
      hrsElem.textContent = `~${hrs} hrs`;
    }
  }
</script>
</body>
</html>"##;

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
    async fn test_audio_stream_endpoint_unauthorized() {
        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
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
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
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
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
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
        let (cmd_tx, _cmd_rx) = mpsc::channel(16);
        let running = Arc::new(AtomicBool::new(true));

        let state = WebState {
            auth_token: "secret123".to_string(),
            recordings_dir: PathBuf::from("/tmp"),
            telemetry_rx: tel_rx,
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
}
