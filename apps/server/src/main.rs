use axum::{
    extract::{
        ws::{Message, WebSocketUpgrade},
        State,
    },
    http::{header, HeaderMap, Method, StatusCode},
    middleware::{self, Next},
    response::{IntoResponse, Response},
    routing::{get, post},
    Json, Router,
};
use kairos_application::{
    models::{Action, Command},
    Application,
};
use serde::Deserialize;
use serde_json::json;
use std::{
    collections::HashMap,
    path::PathBuf,
    sync::{atomic::Ordering, Arc},
    time::{Duration, Instant},
};
use tokio::sync::Mutex;
use tower_http::services::{ServeDir, ServeFile};
use uuid::Uuid;

#[derive(Clone)]
struct AppState {
    app: Arc<Mutex<Application>>,
    kill: Arc<std::sync::atomic::AtomicBool>,
    owner_token: Arc<String>,
    sessions: Arc<Mutex<HashMap<String, Instant>>>,
    login_attempts: Arc<Mutex<Vec<Instant>>>,
    secure_cookie: bool,
}
type ApiError = (StatusCode, Json<serde_json::Value>);
fn error(status: StatusCode, message: &str) -> ApiError {
    (status, Json(json!({"error":message})))
}
fn session_cookie(headers: &HeaderMap) -> Option<&str> {
    headers
        .get(header::COOKIE)?
        .to_str()
        .ok()?
        .split(';')
        .find_map(|part| part.trim().strip_prefix("kairos_session="))
}
fn same_origin(headers: &HeaderMap) -> bool {
    let Some(origin) = headers.get(header::ORIGIN) else {
        return true;
    };
    let Some(host) = headers.get(header::HOST).and_then(|s| s.to_str().ok()) else {
        return false;
    };
    let Ok(origin) = origin.to_str() else {
        return false;
    };
    origin == format!("http://{host}") || origin == format!("https://{host}")
}
async fn authorize(
    State(state): State<AppState>,
    request: axum::extract::Request,
    next: Next,
) -> Response {
    if !same_origin(request.headers()) {
        return error(StatusCode::FORBIDDEN, "Cross-origin request rejected").into_response();
    }
    if request.method() != Method::GET
        && request
            .headers()
            .get("x-kairos-command")
            .and_then(|v| v.to_str().ok())
            != Some("1")
    {
        return error(StatusCode::FORBIDDEN, "Command header required").into_response();
    }
    let valid = if let Some(cookie) = session_cookie(request.headers()) {
        state
            .sessions
            .lock()
            .await
            .get(cookie)
            .is_some_and(|created| created.elapsed() < Duration::from_secs(3600))
    } else {
        false
    };
    if !valid {
        return error(
            StatusCode::UNAUTHORIZED,
            "Sign in with the local owner token",
        )
        .into_response();
    }
    next.run(request).await
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Login {
    token: String,
}
async fn login(
    State(state): State<AppState>,
    headers: HeaderMap,
    Json(input): Json<Login>,
) -> Result<impl IntoResponse, ApiError> {
    if !same_origin(&headers)
        || headers
            .get("x-kairos-command")
            .and_then(|v| v.to_str().ok())
            != Some("1")
    {
        return Err(error(StatusCode::FORBIDDEN, "Invalid login origin"));
    }
    let mut attempts = state.login_attempts.lock().await;
    attempts.retain(|at| at.elapsed() < Duration::from_secs(60));
    if attempts.len() >= 5 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Too many login attempts; retry in one minute",
        ));
    }
    attempts.push(Instant::now());
    let expected = state.owner_token.as_bytes();
    let actual = input.token.as_bytes();
    let difference = expected
        .iter()
        .zip(actual)
        .fold(0u8, |acc, (a, b)| acc | (a ^ b));
    if expected.len() != actual.len() || difference != 0 {
        return Err(error(StatusCode::UNAUTHORIZED, "Invalid owner token"));
    }
    attempts.clear();
    let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
    let mut sessions = state.sessions.lock().await;
    sessions.retain(|_, time| time.elapsed() < Duration::from_secs(3600));
    if sessions.len() >= 32 {
        return Err(error(
            StatusCode::TOO_MANY_REQUESTS,
            "Session limit reached",
        ));
    }
    sessions.insert(token.clone(), Instant::now());
    let cookie = format!(
        "kairos_session={token}; HttpOnly; SameSite=Strict; Path=/; Max-Age=3600{}",
        if state.secure_cookie { "; Secure" } else { "" }
    );
    Ok((
        [(header::SET_COOKIE, cookie)],
        Json(json!({"role":"Owner"})),
    ))
}
async fn logout(State(state): State<AppState>, headers: HeaderMap) -> impl IntoResponse {
    if let Some(cookie) = session_cookie(&headers) {
        state.sessions.lock().await.remove(cookie);
    }
    (
        [(
            header::SET_COOKIE,
            "kairos_session=; HttpOnly; SameSite=Strict; Path=/; Max-Age=0",
        )],
        Json(json!({"ok":true})),
    )
}
async fn snapshot(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.app.lock().await.snapshot())
}
async fn command(
    State(state): State<AppState>,
    Json(command): Json<Command>,
) -> Result<impl IntoResponse, ApiError> {
    if matches!(command.action, Action::Kill) {
        state.kill.store(true, Ordering::SeqCst);
    }
    state
        .app
        .lock()
        .await
        .command(command)
        .await
        .map(Json)
        .map_err(|message| error(StatusCode::UNPROCESSABLE_ENTITY, &message))
}
async fn ready(State(state): State<AppState>) -> impl IntoResponse {
    let app = state.app.lock().await;
    let snapshot = app.snapshot();
    let ready = snapshot
        .readiness
        .iter()
        .filter(|r| r.requirement == "Required")
        .all(|r| r.status == "Ready");
    (
        if ready {
            StatusCode::OK
        } else {
            StatusCode::SERVICE_UNAVAILABLE
        },
        Json(
            json!({"ready":ready,"mode":snapshot.state.mode,"capabilities":snapshot.capabilities,"checks":snapshot.readiness}),
        ),
    )
}
async fn websocket(
    State(state): State<AppState>,
    headers: HeaderMap,
    ws: WebSocketUpgrade,
) -> impl IntoResponse {
    let session = session_cookie(&headers).unwrap_or_default().to_owned();
    ws.max_message_size(4096).on_upgrade(move |mut socket|async move {
        let mut interval=tokio::time::interval(Duration::from_secs(1)); let mut sequence=None;
        loop {
            tokio::select! {
                _=interval.tick()=>{
                    if !state.sessions.lock().await.get(&session).is_some_and(|created| created.elapsed() < Duration::from_secs(3600)) { break; }
                    let snapshot=state.app.lock().await.snapshot();
                    if sequence != Some(snapshot.state.sequence) {
                        sequence=Some(snapshot.state.sequence);
                        let event=json!({"type":"runtime.snapshot","version":1,"sequence":snapshot.state.sequence,"payload":snapshot});
                        if !matches!(tokio::time::timeout(Duration::from_secs(3),socket.send(Message::Text(event.to_string().into()))).await,Ok(Ok(()))) { break; }
                    }
                }
                message=socket.recv()=> { if matches!(message,None|Some(Err(_))|Some(Ok(Message::Close(_)))) { break; } }
            }
        }
    })
}
#[tokio::main]
async fn main() -> Result<(), Box<dyn std::error::Error>> {
    tracing_subscriber::fmt()
        .with_env_filter(tracing_subscriber::EnvFilter::from_default_env())
        .init();
    let directory =
        PathBuf::from(std::env::var("KAIROS_DATA_DIR").unwrap_or_else(|_| ".kairos-data".into()));
    let app = Application::open(&directory).map_err(std::io::Error::other)?;
    #[cfg(feature = "test-support")]
    let app = {
        let mut app = app;
        if let Ok(base) = std::env::var("KAIROS_TEST_UPSTREAM") {
            app.use_test_services(&base);
        }
        app
    };
    let token_path = directory.join("owner-token");
    let owner_token = match std::env::var("KAIROS_OWNER_TOKEN") {
        Ok(token) if token.len() >= 32 => token,
        Ok(_) => return Err("KAIROS_OWNER_TOKEN must contain at least 32 characters".into()),
        Err(_) => {
            if token_path.exists() {
                std::fs::read_to_string(&token_path)?.trim().to_string()
            } else {
                let token = format!("{}{}", Uuid::new_v4().simple(), Uuid::new_v4().simple());
                use std::io::Write;
                let mut file = std::fs::OpenOptions::new()
                    .write(true)
                    .create_new(true)
                    .open(&token_path)?;
                #[cfg(unix)]
                {
                    use std::os::unix::fs::PermissionsExt;
                    file.set_permissions(std::fs::Permissions::from_mode(0o600))?;
                }
                file.write_all(token.as_bytes())?;
                file.sync_all()?;
                token
            }
        }
    };
    let state = AppState {
        kill: app.kill.clone(),
        app: Arc::new(Mutex::new(app)),
        owner_token: Arc::new(owner_token),
        sessions: Arc::new(Mutex::new(HashMap::new())),
        login_attempts: Arc::new(Mutex::new(Vec::new())),
        secure_cookie: std::env::var("KAIROS_SECURE_COOKIE").as_deref() == Ok("true"),
    };
    let timer = state.clone();
    let supervisor = tokio::spawn(async move {
        let mut interval = tokio::time::interval(Duration::from_secs(5));
        interval.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
        loop {
            interval.tick().await;
            if let Err(error) = timer.app.lock().await.tick().await {
                tracing::error!("Supervisor: {error}");
            }
        }
    });
    let protected = Router::new()
        .route("/api/v1/runtime", get(snapshot))
        .route("/api/v1/runtime/readiness", get(ready))
        .route("/api/v1/commands", post(command))
        .route("/api/v1/auth/logout", post(logout))
        .route("/ws", get(websocket))
        .route_layer(middleware::from_fn_with_state(state.clone(), authorize));
    let dist = std::env::var("KAIROS_WEB_DIR").unwrap_or_else(|_| "dist".into());
    let router = Router::new()
        .merge(protected)
        .route("/api/v1/auth/login", post(login))
        .route(
            "/health/live",
            get(|| async { Json(json!({"alive":true})) }),
        )
        .route("/health/ready", get(ready))
        .fallback_service(
            ServeDir::new(&dist).not_found_service(ServeFile::new(format!("{dist}/index.html"))),
        )
        .layer(axum::extract::DefaultBodyLimit::max(16_384))
        .with_state(state);
    let bind = std::env::var("KAIROS_BIND").unwrap_or_else(|_| "127.0.0.1:8080".into());
    let listener = tokio::net::TcpListener::bind(&bind).await?;
    println!(
        "Kairos Agent listening on http://{bind}. Owner token file: {}",
        token_path.display()
    );
    axum::serve(listener, router)
        .with_graceful_shutdown(async {
            let _ = tokio::signal::ctrl_c().await;
        })
        .await?;
    supervisor.abort();
    Ok(())
}
