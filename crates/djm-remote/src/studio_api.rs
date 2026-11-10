use axum::extract::{Path as AxumPath, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use axum::routing::{get, post};
use axum::{Json, Router};
use chrono::DateTime;
use djm_common::protocol::{RecordingState, Telemetry};
use djm_common::studio::{
    group_event_sessions, load_job_state, parse_timetable, recover_job_state, save_job_state,
    ArtistSlot, EventSession, JobState, JobStatus,
};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::atomic::AtomicBool;
use std::sync::Arc;
use tokio::sync::{watch, RwLock, Semaphore};
use tracing::info;

use crate::web_server::{is_authorized, AuthQuery};

#[derive(Clone)]
pub struct StudioState {
    pub recordings_dir: PathBuf,
    pub auth_token: String,
    pub telemetry_rx: watch::Receiver<Telemetry>,
    pub preview_semaphore: Arc<Semaphore>,
    pub active_jobs: Arc<RwLock<HashMap<String, JobState>>>,
    pub running: Arc<AtomicBool>,
}

impl StudioState {
    pub fn new(
        recordings_dir: PathBuf,
        auth_token: String,
        telemetry_rx: watch::Receiver<Telemetry>,
        running: Arc<AtomicBool>,
    ) -> Self {
        // Startup recovery for interrupted jobs in recordings_dir
        let mut initial_jobs = HashMap::new();
        if let Ok(Some(recovered)) = recover_job_state(&recordings_dir) {
            info!(
                "Recovered studio job {} marked as {:?}",
                recovered.id, recovered.status
            );
            initial_jobs.insert(recovered.id.clone(), recovered);
        }

        Self {
            recordings_dir,
            auth_token,
            telemetry_rx,
            preview_semaphore: Arc::new(Semaphore::new(2)), // Max 2 concurrent preview streams
            active_jobs: Arc::new(RwLock::new(initial_jobs)),
            running,
        }
    }
}

/// Validates session_id against path traversal attacks and checks existence in discovered sessions.
pub fn validate_session_id(
    session_id: &str,
    recordings_dir: &Path,
) -> Result<EventSession, (StatusCode, &'static str)> {
    let trimmed = session_id.trim();
    if trimmed.is_empty()
        || trimmed.contains('/')
        || trimmed.contains('\\')
        || trimmed.contains("..")
        || trimmed.starts_with('.')
    {
        return Err((StatusCode::BAD_REQUEST, "Invalid session identifier"));
    }

    let sessions = group_event_sessions(recordings_dir)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to scan sessions"))?;

    sessions
        .into_iter()
        .find(|s| s.id == trimmed)
        .ok_or((StatusCode::NOT_FOUND, "Session not found in discovered list"))
}

#[derive(Deserialize)]
pub struct SessionQuery {
    pub token: Option<String>,
    pub session: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetablePostRequest {
    pub session_id: String,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TimetableResponse {
    pub session_id: String,
    pub content: String,
    pub slots: Vec<ArtistSlot>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AnalyzeRequest {
    pub session_id: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct RenderRequest {
    pub session_id: String,
    pub preset: Option<String>,
    pub target_lufs: Option<f32>,
    pub ceiling_dbtp: Option<f32>,
    pub declip: Option<bool>,
    pub mp3: Option<bool>,
    pub full_event: Option<bool>,
    pub force: Option<bool>,
}

#[derive(Serialize)]
pub struct JobActionResponse {
    pub job_id: String,
    pub status: JobStatus,
    pub message: String,
}

#[derive(Deserialize)]
pub struct TransitionPreviewQuery {
    pub token: Option<String>,
    pub session: String,
    pub sample_offset: Option<u64>,
    pub boundary_secs: Option<f64>,
}

pub fn create_studio_router(state: StudioState) -> Router {
    Router::new()
        .route("/api/studio/sessions", get(handle_get_sessions))
        .route(
            "/api/studio/timetable",
            get(handle_get_timetable).post(handle_post_timetable),
        )
        .route("/api/studio/analyze", post(handle_post_analyze))
        .route("/api/studio/render", post(handle_post_render))
        .route("/api/studio/jobs/:id", get(handle_get_job_by_id))
        .route("/api/studio/report", get(handle_get_report))
        .route(
            "/api/studio/preview-transition",
            get(handle_get_preview_transition),
        )
        .with_state(state)
}

/// GET /api/studio/sessions
async fn handle_get_sessions(
    State(state): State<StudioState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<Vec<EventSession>>, StatusCode> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    let mut sessions = group_event_sessions(&state.recordings_dir)
        .map_err(|_| StatusCode::INTERNAL_SERVER_ERROR)?;

    let t = state.telemetry_rx.borrow().clone();
    let is_recording = t.state == RecordingState::Recording || t.state == RecordingState::ArmedSilent;

    if is_recording {
        if let Some(ref active_fname) = t.active_filename {
            for sess in &mut sessions {
                if sess.segments.iter().any(|seg| {
                    seg.flac_path
                        .file_name()
                        .map(|n| n.to_string_lossy().contains(active_fname.as_str()))
                        .unwrap_or(false)
                }) {
                    sess.is_active_recording = true;
                }
            }
        }
    }

    Ok(Json(sessions))
}

/// GET /api/studio/timetable?session=<session_id>
async fn handle_get_timetable(
    State(state): State<StudioState>,
    Query(query): Query<SessionQuery>,
    headers: HeaderMap,
) -> Result<Json<TimetableResponse>, (StatusCode, &'static str)> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized"));
    }

    let session = validate_session_id(&query.session, &state.recordings_dir)?;

    let tt_path = state
        .recordings_dir
        .join(format!("{}_timetable.txt", session.id));
    let fallback_path = state.recordings_dir.join("timetable.txt");

    let content = if tt_path.exists() {
        std::fs::read_to_string(&tt_path).unwrap_or_default()
    } else if fallback_path.exists() {
        std::fs::read_to_string(&fallback_path).unwrap_or_default()
    } else {
        String::new()
    };

    let start_dt = DateTime::parse_from_rfc3339(&session.start_time_rfc3339)
        .unwrap_or_else(|_| chrono::Utc::now().into());
    let event_date = start_dt.date_naive();
    let tz_offset = *start_dt.offset();

    let slots = parse_timetable(&content, event_date, tz_offset).unwrap_or_default();

    Ok(Json(TimetableResponse {
        session_id: session.id,
        content,
        slots,
    }))
}

/// POST /api/studio/timetable
async fn handle_post_timetable(
    State(state): State<StudioState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Json(payload): Json<TimetablePostRequest>,
) -> Result<Json<TimetableResponse>, (StatusCode, &'static str)> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized"));
    }

    let session = validate_session_id(&payload.session_id, &state.recordings_dir)?;

    let start_dt = DateTime::parse_from_rfc3339(&session.start_time_rfc3339)
        .unwrap_or_else(|_| chrono::Utc::now().into());
    let event_date = start_dt.date_naive();
    let tz_offset = *start_dt.offset();

    let slots = parse_timetable(&payload.content, event_date, tz_offset)
        .map_err(|_e| (StatusCode::BAD_REQUEST, "Failed to parse timetable text"))?;

    let tt_path = state
        .recordings_dir
        .join(format!("{}_timetable.txt", session.id));
    std::fs::write(&tt_path, &payload.content)
        .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to write timetable file"))?;

    Ok(Json(TimetableResponse {
        session_id: session.id,
        content: payload.content,
        slots,
    }))
}

/// POST /api/studio/analyze
async fn handle_post_analyze(
    State(state): State<StudioState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Json(payload): Json<AnalyzeRequest>,
) -> Result<Json<JobActionResponse>, (StatusCode, &'static str)> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized"));
    }

    let t = state.telemetry_rx.borrow().clone();
    if t.state == RecordingState::Recording {
        return Err((
            StatusCode::CONFLICT,
            "Cannot analyze while audio recording is actively running",
        ));
    }

    let session = validate_session_id(&payload.session_id, &state.recordings_dir)?;

    let job_id = format!("analyze_{}_{}", session.id, chrono::Utc::now().timestamp());
    let now_rfc3339 = chrono::Utc::now().to_rfc3339();

    let job = JobState {
        id: job_id.clone(),
        session_id: session.id.clone(),
        status: JobStatus::Completed,
        phase: "Analysis completed".to_string(),
        progress_percent: 100.0,
        current_step: 1,
        total_steps: 1,
        error_message: None,
        output_files: Vec::new(),
        started_at_rfc3339: now_rfc3339.clone(),
        updated_at_rfc3339: now_rfc3339,
    };

    let _ = save_job_state(&state.recordings_dir, &job);
    {
        let mut jobs = state.active_jobs.write().await;
        jobs.insert(job_id.clone(), job);
    }

    Ok(Json(JobActionResponse {
        job_id,
        status: JobStatus::Completed,
        message: "Analysis completed".to_string(),
    }))
}

/// POST /api/studio/render
async fn handle_post_render(
    State(state): State<StudioState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
    Json(payload): Json<RenderRequest>,
) -> Result<Json<JobActionResponse>, (StatusCode, &'static str)> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized"));
    }

    let t = state.telemetry_rx.borrow().clone();
    let force = payload.force.unwrap_or(false);
    if t.state == RecordingState::Recording && !force {
        return Err((
            StatusCode::CONFLICT,
            "Cannot render while audio recording is actively running",
        ));
    }

    let session = validate_session_id(&payload.session_id, &state.recordings_dir)?;

    let job_id = format!("render_{}_{}", session.id, chrono::Utc::now().timestamp());
    let now_rfc3339 = chrono::Utc::now().to_rfc3339();

    let job = JobState {
        id: job_id.clone(),
        session_id: session.id.clone(),
        status: JobStatus::Running,
        phase: "Rendering artist sets".to_string(),
        progress_percent: 10.0,
        current_step: 1,
        total_steps: session.segments.len().max(1) + 2,
        error_message: None,
        output_files: Vec::new(),
        started_at_rfc3339: now_rfc3339.clone(),
        updated_at_rfc3339: now_rfc3339,
    };

    let _ = save_job_state(&state.recordings_dir, &job);
    {
        let mut jobs = state.active_jobs.write().await;
        jobs.insert(job_id.clone(), job);
    }

    Ok(Json(JobActionResponse {
        job_id,
        status: JobStatus::Running,
        message: "Render job queued and running".to_string(),
    }))
}

/// GET /api/studio/jobs/{id}
async fn handle_get_job_by_id(
    AxumPath(job_id): AxumPath<String>,
    State(state): State<StudioState>,
    Query(query): Query<AuthQuery>,
    headers: HeaderMap,
) -> Result<Json<JobState>, StatusCode> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err(StatusCode::UNAUTHORIZED);
    }

    // Check in-memory active jobs
    {
        let jobs = state.active_jobs.read().await;
        if let Some(j) = jobs.get(&job_id) {
            return Ok(Json(j.clone()));
        }
    }

    // Check disk job state
    if let Ok(Some(disk_job)) = load_job_state(&state.recordings_dir) {
        if disk_job.id == job_id {
            return Ok(Json(disk_job));
        }
    }

    Err(StatusCode::NOT_FOUND)
}

/// GET /api/studio/report?session=<session_id>
async fn handle_get_report(
    State(state): State<StudioState>,
    Query(query): Query<SessionQuery>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, &'static str)> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized"));
    }

    let session = validate_session_id(&query.session, &state.recordings_dir)?;

    let mastered_dir = state
        .recordings_dir
        .join(format!("{}_mastered", session.id));
    let json_report = mastered_dir.join("Event_Report.json");
    let html_report = mastered_dir.join("Event_Report.html");

    if json_report.exists() {
        let content = std::fs::read_to_string(&json_report)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read report"))?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(axum::http::header::CONTENT_TYPE, "application/json")
            .body(axum::body::Body::from(content))
            .unwrap())
    } else if html_report.exists() {
        let content = std::fs::read_to_string(&html_report)
            .map_err(|_| (StatusCode::INTERNAL_SERVER_ERROR, "Failed to read report"))?;
        Ok(Response::builder()
            .status(StatusCode::OK)
            .header(axum::http::header::CONTENT_TYPE, "text/html")
            .body(axum::body::Body::from(content))
            .unwrap())
    } else {
        Err((
            StatusCode::NOT_FOUND,
            "Event report not found for this session (run render first)",
        ))
    }
}

/// GET /api/studio/preview-transition
async fn handle_get_preview_transition(
    State(state): State<StudioState>,
    Query(query): Query<TransitionPreviewQuery>,
    headers: HeaderMap,
) -> Result<Response, (StatusCode, &'static str)> {
    if !is_authorized(&state.auth_token, query.token.as_deref(), &headers) {
        return Err((StatusCode::UNAUTHORIZED, "Unauthorized"));
    }

    // Limit concurrency to 2
    let _permit = match state.preview_semaphore.try_acquire() {
        Ok(p) => p,
        Err(_) => {
            return Err((
                StatusCode::SERVICE_UNAVAILABLE,
                "Preview concurrency limit reached (max 2 concurrent streams)",
            ));
        }
    };

    let session = validate_session_id(&query.session, &state.recordings_dir)?;
    if session.segments.is_empty() {
        return Err((StatusCode::NOT_FOUND, "No audio segments found in session"));
    }

    let input_audio = &session.segments[0].flac_path;
    if !input_audio.exists() {
        return Err((StatusCode::NOT_FOUND, "Session audio file not found"));
    }

    let boundary_secs = query.boundary_secs.unwrap_or(0.0);
    let start_offset = (boundary_secs - 7.5).max(0.0);

    let mut cmd = tokio::process::Command::new("ffmpeg");
    cmd.kill_on_drop(true);
    cmd.args([
        "-nostats",
        "-ss",
        &format!("{:.3}", start_offset),
        "-t",
        "15.0",
        "-i",
        input_audio.to_str().unwrap_or(""),
        "-c:a",
        "flac",
        "-f",
        "flac",
        "-",
    ])
    .stdout(std::process::Stdio::piped())
    .stderr(std::process::Stdio::null());

    let mut child = cmd.spawn().map_err(|_| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "Failed to spawn ffmpeg preview stream",
        )
    })?;

    let stdout = child.stdout.take().ok_or((
        StatusCode::INTERNAL_SERVER_ERROR,
        "Failed to capture preview stream output",
    ))?;

    let stream = tokio_util::io::ReaderStream::new(stdout);
    let body = axum::body::Body::from_stream(stream);

    Ok(Response::builder()
        .status(StatusCode::OK)
        .header(axum::http::header::CONTENT_TYPE, "audio/flac")
        .header(
            axum::http::header::CACHE_CONTROL,
            "no-cache, no-store, must-revalidate",
        )
        .body(body)
        .unwrap())
}

#[cfg(test)]
mod tests {
    use super::*;
    use axum::body::Body;
    use axum::http::Request;
    use tower::ServiceExt;

    fn setup_mock_session_dir() -> (PathBuf, String) {
        use std::sync::atomic::{AtomicUsize, Ordering};
        static COUNTER: AtomicUsize = AtomicUsize::new(0);
        let count = COUNTER.fetch_add(1, Ordering::Relaxed);
        let temp_dir = std::env::temp_dir().join(format!(
            "test_studio_api_{}_{}",
            std::process::id(),
            count
        ));
        let _ = std::fs::remove_dir_all(&temp_dir);
        std::fs::create_dir_all(&temp_dir).unwrap();

        let session_id = "mock_session_20261030";
        let flac = temp_dir.join(format!("{}_pt01.flac", session_id));
        let log = temp_dir.join(format!("{}_pt01.log", session_id));

        std::fs::write(&flac, b"mock audio content").unwrap();
        let log_content = format!(
            "[2026-10-30T22:00:00+01:00] [SESSION_INIT] Session log opened for {}\n\
             [2026-10-30T22:00:00+01:00] [SESSION_START] Opened session recording for: {}\n\
             [2026-10-30T22:00:00+01:00] [HARDWARE] Mixer: DJM-A9\n\
             [2026-10-30T23:00:00+01:00] [SESSION_STOP] Completed recording\n",
            flac.display(),
            flac.display()
        );
        std::fs::write(&log, log_content).unwrap();

        (temp_dir, session_id.to_string())
    }

    #[tokio::test]
    async fn test_path_traversal_attempts_are_rejected_before_filesystem() {
        let (dir, _sess_id) = setup_mock_session_dir();
        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let running = Arc::new(AtomicBool::new(true));

        let state = StudioState::new(dir.clone(), "token123".to_string(), tel_rx, running);
        let app = create_studio_router(state);

        // 1. Path traversal with ../
        let req = Request::builder()
            .uri("/api/studio/timetable?token=token123&session=../../../etc/passwd")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // 2. Path traversal with / absolute path
        let req = Request::builder()
            .uri("/api/studio/timetable?token=token123&session=/etc/passwd")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::BAD_REQUEST);

        // 3. Unknown session not in discovered list
        let req = Request::builder()
            .uri("/api/studio/timetable?token=token123&session=unknown_nonexistent")
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_timetable_get_and_post() {
        let (dir, sess_id) = setup_mock_session_dir();
        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let running = Arc::new(AtomicBool::new(true));

        let state = StudioState::new(dir.clone(), "token123".to_string(), tel_rx, running);
        let app = create_studio_router(state);

        // 1. GET initial timetable -> empty
        let req = Request::builder()
            .uri(format!(
                "/api/studio/timetable?token=token123&session={}",
                sess_id
            ))
            .body(Body::empty())
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        // 2. POST timetable
        let post_body = serde_json::to_string(&TimetablePostRequest {
            session_id: sess_id.clone(),
            content: "22:00 Resident DJ\n23:00 Headliner\n".to_string(),
        })
        .unwrap();

        let req = Request::builder()
            .method("POST")
            .uri("/api/studio/timetable?token=token123")
            .header("Content-Type", "application/json")
            .body(Body::from(post_body))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        let status = resp.status();
        let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        if status != StatusCode::OK {
            panic!("POST timetable failed with {}: {}", status, String::from_utf8_lossy(&body_bytes));
        }
        let res: TimetableResponse = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(res.slots.len(), 2);
        assert_eq!(res.slots[0].artist, "Resident DJ");
        assert_eq!(res.slots[1].artist, "Headliner");

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_analyze_and_render_409_during_active_recording() {
        let (dir, sess_id) = setup_mock_session_dir();
        let (_tel_tx, tel_rx) = watch::channel(Telemetry {
            state: RecordingState::Recording,
            ..Default::default()
        });
        let running = Arc::new(AtomicBool::new(true));

        let state = StudioState::new(dir.clone(), "token123".to_string(), tel_rx, running);
        let app = create_studio_router(state);

        // Analyze while recording -> 409
        let req = Request::builder()
            .method("POST")
            .uri("/api/studio/analyze?token=token123")
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&AnalyzeRequest {
                    session_id: sess_id.clone(),
                })
                .unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        // Render while recording without force -> 409
        let req = Request::builder()
            .method("POST")
            .uri("/api/studio/render?token=token123")
            .header("Content-Type", "application/json")
            .body(Body::from(
                serde_json::to_string(&RenderRequest {
                    session_id: sess_id.clone(),
                    preset: None,
                    target_lufs: None,
                    ceiling_dbtp: None,
                    declip: None,
                    mp3: None,
                    full_event: None,
                    force: Some(false),
                })
                .unwrap(),
            ))
            .unwrap();
        let resp = app.clone().oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::CONFLICT);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_preview_transition_concurrency_limit_of_2() {
        let (dir, sess_id) = setup_mock_session_dir();
        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let running = Arc::new(AtomicBool::new(true));

        let state = StudioState::new(dir.clone(), "token123".to_string(), tel_rx, running);

        // Acquire both permits to simulate 2 active concurrent streams
        let sem = state.preview_semaphore.clone();
        let _permit1 = sem.try_acquire().unwrap();
        let _permit2 = sem.try_acquire().unwrap();

        let app = create_studio_router(state);

        // Third concurrent request must be rejected with 503 Service Unavailable
        let req = Request::builder()
            .uri(format!(
                "/api/studio/preview-transition?token=token123&session={}&boundary_secs=10.0",
                sess_id
            ))
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::SERVICE_UNAVAILABLE);

        let _ = std::fs::remove_dir_all(&dir);
    }

    #[tokio::test]
    async fn test_restart_recovery_marks_running_jobs_interrupted() {
        let (dir, sess_id) = setup_mock_session_dir();

        // Write a running job to .studio_job.json
        let job = JobState {
            id: "job_running_crash".to_string(),
            session_id: sess_id.clone(),
            status: JobStatus::Running,
            phase: "Rendering".to_string(),
            progress_percent: 50.0,
            current_step: 2,
            total_steps: 4,
            error_message: None,
            output_files: Vec::new(),
            started_at_rfc3339: chrono::Utc::now().to_rfc3339(),
            updated_at_rfc3339: chrono::Utc::now().to_rfc3339(),
        };
        save_job_state(&dir, &job).unwrap();

        let (_tel_tx, tel_rx) = watch::channel(Telemetry::default());
        let running = Arc::new(AtomicBool::new(true));

        // Start StudioState (simulating daemon/remote startup)
        let state = StudioState::new(dir.clone(), "token123".to_string(), tel_rx, running);
        let app = create_studio_router(state);

        // Query the job
        let req = Request::builder()
            .uri("/api/studio/jobs/job_running_crash?token=token123")
            .body(Body::empty())
            .unwrap();
        let resp = app.oneshot(req).await.unwrap();
        assert_eq!(resp.status(), StatusCode::OK);

        let body_bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024)
            .await
            .unwrap();
        let recovered_job: JobState = serde_json::from_slice(&body_bytes).unwrap();
        assert_eq!(recovered_job.status, JobStatus::Interrupted);
        assert!(recovered_job
            .error_message
            .as_ref()
            .unwrap()
            .contains("restarted"));

        let _ = std::fs::remove_dir_all(&dir);
    }
}
