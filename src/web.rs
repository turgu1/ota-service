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
    pub config_path: String,
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

/// Password validation request
#[derive(Debug, Deserialize)]
pub struct ValidatePasswordRequest {
    password: String,
}

/// Config update request
#[derive(Debug, Deserialize)]
pub struct ConfigUpdateRequest {
    section: String,
    key: String,
    value: String,
    admin_password: Option<String>,
}

/// Generic error response
#[derive(Debug, Serialize)]
pub struct ErrorResponse {
    error: String,
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
    pub edit_session_timeout: u64,
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
        .route(
            "/api/config/validate-password",
            post(validate_password_handler),
        )
        .route("/api/config/update", post(update_config_handler))
        .route("/api/restart", post(restart_handler))
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

    // Reload config from file to get latest changes
    let config = match Configuration::from_file(&state.config_path) {
        Ok(cfg) => cfg,
        Err(e) => {
            error!("Failed to reload config from file: {}", e);
            // Fall back to cached config if reload fails
            state.config.clone()
        }
    };

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
            edit_session_timeout: config.web.edit_session_timeout,
        },
    };

    Json(response).into_response()
}

/// Validate admin password
async fn validate_password_handler(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<ValidatePasswordRequest>,
) -> Response {
    // Check if user is authenticated
    if !is_authenticated(&session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Not authenticated".to_string(),
            }),
        )
            .into_response();
    }

    // Validate password
    if payload.password == state.config.web.password {
        (
            StatusCode::OK,
            Json(LoginResponse {
                success: true,
                message: "Password validated".to_string(),
            }),
        )
            .into_response()
    } else {
        (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Invalid password".to_string(),
            }),
        )
            .into_response()
    }
}

/// Update configuration
async fn update_config_handler(
    State(state): State<AppState>,
    session: Session,
    Json(payload): Json<ConfigUpdateRequest>,
) -> Response {
    // Check if user is authenticated
    if !is_authenticated(&session).await {
        return (
            StatusCode::UNAUTHORIZED,
            Json(ErrorResponse {
                error: "Not authenticated".to_string(),
            }),
        )
            .into_response();
    }

    // For password changes, validate admin password
    let is_password_field = payload.key.contains("password")
        || payload.key.contains("token")
        || payload.key.contains("key");
    if is_password_field {
        if let Some(admin_password) = &payload.admin_password {
            if admin_password != &state.config.web.password {
                return (
                    StatusCode::UNAUTHORIZED,
                    Json(ErrorResponse {
                        error: "Invalid admin password".to_string(),
                    }),
                )
                    .into_response();
            }
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Admin password required for password changes".to_string(),
                }),
            )
                .into_response();
        }
    }

    info!(
        "Config update requested: section={}, key={}, value={}",
        payload.section, payload.key, payload.value
    );

    // Read the current config file
    let config_contents = match std::fs::read_to_string(&state.config_path) {
        Ok(contents) => contents,
        Err(e) => {
            error!("Failed to read config file: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read configuration file: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Parse YAML
    let mut yaml_value: serde_yml::Value = match serde_yml::from_str(&config_contents) {
        Ok(value) => value,
        Err(e) => {
            error!("Failed to parse config file: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to parse configuration file: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Update the specific field in YAML
    if let Some(section) = yaml_value.as_mapping_mut() {
        if let Some(section_value) =
            section.get_mut(&serde_yml::Value::String(payload.section.clone()))
        {
            if let Some(section_map) = section_value.as_mapping_mut() {
                // Parse the value based on the key type
                let new_value = match parse_config_value(&payload.key, &payload.value) {
                    Ok(v) => v,
                    Err(e) => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: format!("Invalid value: {}", e),
                            }),
                        )
                            .into_response();
                    }
                };

                section_map.insert(serde_yml::Value::String(payload.key.clone()), new_value);
            } else {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(ErrorResponse {
                        error: format!("Section '{}' is not a mapping", payload.section),
                    }),
                )
                    .into_response();
            }
        } else {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Section '{}' not found", payload.section),
                }),
            )
                .into_response();
        }
    } else {
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: "Configuration file is not a valid YAML mapping".to_string(),
            }),
        )
            .into_response();
    }

    // Write updated YAML back to file
    let updated_yaml = match serde_yml::to_string(&yaml_value) {
        Ok(yaml) => yaml,
        Err(e) => {
            error!("Failed to serialize config: {}", e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to serialize configuration: {}", e),
                }),
            )
                .into_response();
        }
    };

    if let Err(e) = std::fs::write(&state.config_path, updated_yaml) {
        error!("Failed to write config file: {}", e);
        return (
            StatusCode::INTERNAL_SERVER_ERROR,
            Json(ErrorResponse {
                error: format!("Failed to write configuration file: {}", e),
            }),
        )
            .into_response();
    }

    // Reload configuration in memory
    match Configuration::from_file(&state.config_path) {
        Ok(_new_config) => {
            // Note: The config is updated in the file, but the running service
            // needs to be restarted for most changes to take effect (MQTT, database, etc.)
            info!("Configuration file updated successfully. Service restart may be required for changes to take full effect.");

            (
                StatusCode::OK,
                Json(LoginResponse {
                    success: true,
                    message: "Configuration updated successfully. Restart the service for changes to take full effect.".to_string(),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to reload config: {}", e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Configuration file updated but validation failed: {}. Please check the file manually.", e),
                }),
            )
                .into_response()
        }
    }
}

/// Parse configuration value based on key type
fn parse_config_value(key: &str, value: &str) -> Result<serde_yml::Value, String> {
    // Remove 's' suffix for time-related fields
    let cleaned_value = if key.ends_with("_interval")
        || key.ends_with("_timeout")
        || key.ends_with("_alive")
        || key.ends_with("_period")
    {
        value.trim_end_matches('s')
    } else {
        value
    };

    // Determine type based on key name
    match key {
        // Integer fields
        "port" | "pool_size" | "max_concurrent_updates" | "default_ota_port" => cleaned_value
            .parse::<u64>()
            .map(|v| serde_yml::Value::Number(serde_yml::Number::from(v)))
            .map_err(|_| format!("Invalid integer value for {}", key)),
        // Time duration fields (stored as u64)
        "keep_alive"
        | "check_interval"
        | "update_timeout"
        | "refresh_period"
        | "edit_session_timeout" => cleaned_value
            .parse::<u64>()
            .map(|v| serde_yml::Value::Number(serde_yml::Number::from(v)))
            .map_err(|_| format!("Invalid duration value for {}", key)),
        // Boolean fields
        "erase_firmware_after_upload" | "enabled" => match cleaned_value.to_lowercase().as_str() {
            "true" | "yes" | "1" => Ok(serde_yml::Value::Bool(true)),
            "false" | "no" | "0" => Ok(serde_yml::Value::Bool(false)),
            _ => Err(format!("Invalid boolean value for {}", key)),
        },
        // Priority field (i8)
        "priority" => cleaned_value
            .parse::<i64>()
            .map(|v| serde_yml::Value::Number(serde_yml::Number::from(v)))
            .map_err(|_| format!("Invalid priority value")),
        // Everything else is a string (including passwords, paths, etc.)
        _ => {
            if cleaned_value == "None" || cleaned_value.is_empty() {
                Ok(serde_yml::Value::Null)
            } else {
                Ok(serde_yml::Value::String(cleaned_value.to_string()))
            }
        }
    }
}

/// Restart the application
async fn restart_handler(session: Session) -> Response {
    // Check if user is authenticated
    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    info!("Service restart requested by user");

    // Spawn a task to exit the process after a short delay
    // This allows the response to be sent back to the client
    tokio::spawn(async {
        tokio::time::sleep(tokio::time::Duration::from_millis(500)).await;
        info!("Initiating service restart...");
        std::process::exit(0);
    });

    (
        StatusCode::OK,
        Json(LoginResponse {
            success: true,
            message: "Service restart initiated".to_string(),
        }),
    )
        .into_response()
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
    config_path: String,
    database: Arc<Mutex<Database>>,
) -> Result<(), Box<dyn std::error::Error>> {
    let addr = format!("0.0.0.0:{}", config.web.port);
    info!("Starting web server on {}", addr);

    let state = AppState {
        database,
        config,
        config_path,
    };
    let app = create_router(state);

    let listener = tokio::net::TcpListener::bind(&addr).await?;
    info!("Web server listening on {}", addr);

    axum::serve(listener, app).await?;

    Ok(())
}
