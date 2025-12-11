use axum::{
    extract::{
        ws::{Message, WebSocket, WebSocketUpgrade},
        State,
    },
    http::StatusCode,
    response::{Html, IntoResponse, Response},
    routing::{get, post},
    Form, Json, Router,
};
use log::{error, info};
use serde::{Deserialize, Serialize};
use std::sync::Arc;
use tokio::sync::Mutex;
use tower_sessions::{Expiry, MemoryStore, Session, SessionManagerLayer};

use crate::{
    config::{Configuration, WebConfig},
    database::{Database, Device, UploadHistoryEntry},
};

const SESSION_USER_KEY: &str = "user_id";

/// Web server state
#[derive(Clone)]
pub struct AppState {
    pub database: Arc<Mutex<Database>>,
    pub config: Configuration,
}

/// Login form data
#[derive(Debug, Deserialize)]
pub struct LoginForm {
    username: String,
    password: String,
    remember: Option<String>,
}

/// Login response
#[derive(Debug, Serialize)]
pub struct LoginResponse {
    success: bool,
    message: String,
}

/// Device list response
#[derive(Debug, Serialize)]
pub struct DevicesResponse {
    devices: Vec<Device>,
    upload_history: Vec<UploadHistoryEntry>,
}

/// Configuration response with sensitive data masked
#[derive(Debug, Serialize)]
pub struct ConfigResponse {
    mqtt: MaskedMqttConfig,
    database: DatabaseConfigView,
    service: ServiceConfigView,
    firmware: MaskedFirmwareConfig,
    pushover: Option<MaskedPushoverConfig>,
    web: MaskedWebConfig,
}

#[derive(Debug, Serialize)]
pub struct MaskedMqttConfig {
    pub host: String,
    pub port: u16,
    pub client_id: String,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keep_alive: u64,
    pub registration_topic: String,
}

#[derive(Debug, Serialize)]
pub struct DatabaseConfigView {
    pub path: String,
    pub pool_size: u32,
}

#[derive(Debug, Serialize)]
pub struct ServiceConfigView {
    pub name: String,
    pub log_level: String,
    pub log_file_path: Option<String>,
}

#[derive(Debug, Serialize)]
pub struct MaskedFirmwareConfig {
    pub storage_path: String,
    pub max_concurrent_updates: u32,
    pub update_timeout: u64,
    pub check_interval: u64,
    pub ota_password: Option<String>,
    pub default_ota_port: u16,
    pub erase_firmware_after_upload: bool,
}

#[derive(Debug, Serialize)]
pub struct MaskedPushoverConfig {
    pub api_token: String,
    pub user_key: String,
    pub device: Option<String>,
    pub priority: i8,
    pub enabled: bool,
}

#[derive(Debug, Serialize)]
pub struct MaskedWebConfig {
    pub port: u16,
    pub username: String,
    pub password: String,
    pub refresh_period: u64,
}

/// Create and configure the web server router
pub fn create_router(state: AppState) -> Router {
    // Create session store
    let session_store = MemoryStore::default();
    let session_layer = SessionManagerLayer::new(session_store)
        .with_secure(false)
        .with_expiry(Expiry::OnInactivity(time::Duration::hours(24)));

    Router::new()
        .route("/", get(index_handler))
        .route("/login", post(login_handler))
        .route("/logout", post(logout_handler))
        .route("/api/devices", get(devices_handler))
        .route("/api/config", get(config_handler))
        .route("/ws", get(websocket_handler))
        .layer(session_layer)
        .with_state(state)
}

/// Serve the main HTML page
async fn index_handler() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

/// Handle login requests
async fn login_handler(
    State(state): State<AppState>,
    session: Session,
    Form(form): Form<LoginForm>,
) -> Json<LoginResponse> {
    // Validate credentials
    if form.username == state.config.web.username && form.password == state.config.web.password {
        // Store user in session
        if let Err(e) = session
            .insert(SESSION_USER_KEY, form.username.clone())
            .await
        {
            error!("Failed to store session: {}", e);
            return Json(LoginResponse {
                success: false,
                message: "Failed to create session".to_string(),
            });
        }

        // Set session expiry based on remember me
        if form.remember.is_some() {
            session.set_expiry(Some(Expiry::OnInactivity(time::Duration::days(30))));
        }

        info!("User {} logged in", form.username);
        Json(LoginResponse {
            success: true,
            message: "Login successful".to_string(),
        })
    } else {
        Json(LoginResponse {
            success: false,
            message: "Invalid username or password".to_string(),
        })
    }
}

/// Handle logout requests
async fn logout_handler(session: Session) -> Json<LoginResponse> {
    if let Err(e) = session.delete().await {
        error!("Failed to delete session: {}", e);
    }
    Json(LoginResponse {
        success: true,
        message: "Logged out".to_string(),
    })
}

/// Check if user is authenticated
async fn is_authenticated(session: &Session) -> bool {
    session
        .get::<String>(SESSION_USER_KEY)
        .await
        .ok()
        .flatten()
        .is_some()
}

/// Get all devices (requires authentication)
async fn devices_handler(State(state): State<AppState>, session: Session) -> Response {
    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    let db = state.database.lock().await;
    let devices = match db.get_all_devices() {
        Ok(devices) => devices,
        Err(e) => {
            error!("Failed to get devices: {}", e);
            return (StatusCode::INTERNAL_SERVER_ERROR, "Failed to get devices").into_response();
        }
    };

    let upload_history = match db.get_recent_upload_history(50) {
        Ok(history) => history,
        Err(e) => {
            error!("Failed to get upload history: {}", e);
            Vec::new()
        }
    };

    Json(DevicesResponse {
        devices,
        upload_history,
    })
    .into_response()
}

/// Get configuration with sensitive data masked (requires authentication)
async fn config_handler(State(state): State<AppState>, session: Session) -> Response {
    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    let config = &state.config;

    // Mask sensitive data
    fn mask_string(s: &str) -> String {
        if s.is_empty() {
            String::new()
        } else {
            "•".repeat(s.len().min(8))
        }
    }

    fn mask_ip(host: &str) -> String {
        // Check if it looks like an IP address
        if host.split('.').count() == 4 && host.chars().all(|c| c.is_numeric() || c == '.') {
            let parts: Vec<&str> = host.split('.').collect();
            if parts.len() == 4 {
                return format!("{}.{}.•.•", parts[0], parts[1]);
            }
        }
        host.to_string()
    }

    let response = ConfigResponse {
        mqtt: MaskedMqttConfig {
            host: mask_ip(&config.mqtt.host),
            port: config.mqtt.port,
            client_id: config.mqtt.client_id.clone(),
            username: config.mqtt.username.as_ref().map(|u| mask_string(u)),
            password: config
                .mqtt
                .password
                .as_ref()
                .map(|_| mask_string("password")),
            keep_alive: config.mqtt.keep_alive,
            registration_topic: config.mqtt.registration_topic.clone(),
        },
        database: DatabaseConfigView {
            path: config.database.path.clone(),
            pool_size: config.database.pool_size,
        },
        service: ServiceConfigView {
            name: config.service.name.clone(),
            log_level: config.service.log_level.clone(),
            log_file_path: config.service.log_file_path.clone(),
        },
        firmware: MaskedFirmwareConfig {
            storage_path: config.firmware.storage_path.clone(),
            max_concurrent_updates: config.firmware.max_concurrent_updates,
            update_timeout: config.firmware.update_timeout,
            check_interval: config.firmware.check_interval,
            ota_password: config
                .firmware
                .ota_password
                .as_ref()
                .map(|_| mask_string("password")),
            default_ota_port: config.firmware.default_ota_port,
            erase_firmware_after_upload: config.firmware.erase_firmware_after_upload,
        },
        pushover: config.pushover.as_ref().map(|p| MaskedPushoverConfig {
            api_token: mask_string(&p.api_token),
            user_key: mask_string(&p.user_key),
            device: p.device.clone(),
            priority: p.priority,
            enabled: p.enabled,
        }),
        web: MaskedWebConfig {
            port: config.web.port,
            username: mask_string(&config.web.username),
            password: mask_string("password"),
            refresh_period: config.web.refresh_period,
        },
    };

    Json(response).into_response()
}

/// WebSocket handler for real-time updates
async fn websocket_handler(
    ws: WebSocketUpgrade,
    State(state): State<AppState>,
    session: Session,
) -> Response {
    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    ws.on_upgrade(move |socket| handle_websocket(socket, state))
}

/// Handle WebSocket connection
async fn handle_websocket(mut socket: WebSocket, state: AppState) {
    info!("WebSocket connection established");

    let refresh_period = state.config.web.refresh_period;

    loop {
        // Get current device list and upload history
        let (devices, upload_history) = {
            let db = state.database.lock().await;
            let devices = match db.get_all_devices() {
                Ok(devices) => devices,
                Err(e) => {
                    error!("Failed to get devices: {}", e);
                    break;
                }
            };
            let upload_history = match db.get_recent_upload_history(50) {
                Ok(history) => history,
                Err(e) => {
                    error!("Failed to get upload history: {}", e);
                    Vec::new()
                }
            };
            (devices, upload_history)
        };

        // Send devices and upload history to client
        let response = DevicesResponse {
            devices,
            upload_history,
        };
        if let Ok(json) = serde_json::to_string(&response) {
            if socket.send(Message::Text(json.into())).await.is_err() {
                break;
            }
        }

        // Wait for the refresh period or check for close message
        tokio::select! {
            _ = tokio::time::sleep(tokio::time::Duration::from_secs(refresh_period)) => {
                // Timer expired, continue to next iteration
            }
            msg = socket.recv() => {
                // Check if client sent close message
                match msg {
                    Some(Ok(Message::Close(_))) => break,
                    None | Some(Err(_)) => break,
                    _ => {} // Other message types, ignore and continue
                }
            }
        }
    }

    info!("WebSocket connection closed");
}

/// Start the web server
pub async fn start_web_server(
    config: Configuration,
    database: Arc<Mutex<Database>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("0.0.0.0:{}", config.web.port);
    info!("Starting web server on {}", addr);

    let state = AppState { database, config };
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Web server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
