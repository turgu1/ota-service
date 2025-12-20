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
    config::Configuration,
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

/// Add device request
#[derive(Debug, Deserialize)]
pub struct AddDeviceRequest {
    device_id: String,
    firmware_version: Option<String>,
    project_folder: Option<String>,
    main_filename: Option<String>,
}

/// Update device request
#[derive(Debug, Deserialize)]
pub struct UpdateDeviceRequest {
    old_device_id: String,
    new_device_id: String,
    project_folder: Option<String>,
    main_filename: Option<String>,
    firmware_version: Option<String>,
}

/// Remove device request
#[derive(Debug, Deserialize)]
pub struct RemoveDeviceRequest {
    device_id: String,
}

/// Rebuild device firmware request
#[derive(Debug, Deserialize)]
pub struct RebuildDeviceRequest {
    device_id: String,
}

/// Rebuild response
#[derive(Debug, Serialize)]
pub struct RebuildResponse {
    success: bool,
    output: String,
    firmware_path: Option<String>,
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
    home_assistant: Option<HomeAssistantConfigView>,
    esphome_projects: Option<EsphomeProjectsConfigView>,
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
    pub max_concurrent_updates: u32,
    pub check_interval: u64,
    pub ota_password: Option<String>,
    pub default_ota_port: u16,
}

#[derive(Debug, Serialize)]
pub struct MaskedFirmwareConfig {
    pub storage_path: String,
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
pub struct HomeAssistantConfigView {
    pub enabled: bool,
    pub discovery_prefix: String,
    pub node_id: String,
    pub device_name: String,
    pub manufacturer: Option<String>,
    pub model: Option<String>,
    pub update_interval: u64,
}

#[derive(Debug, Serialize)]
pub struct EsphomeProjectsConfigView {
    pub enable: bool,
    pub projects_folder: Option<String>,
    pub default_main_filename: String,
    pub esphome_venv_folder: Option<String>,
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
        .route("/api/devices/add", post(add_device_handler))
        .route("/api/devices/update", post(update_device_handler))
        .route("/api/devices/remove", post(remove_device_handler))
        .route("/api/devices/rebuild", post(rebuild_device_handler))
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

/// Add a new device (requires authentication)
async fn add_device_handler(
    State(state): State<AppState>,
    session: Session,
    Json(request): Json<AddDeviceRequest>,
) -> Response {
    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    // Validate device_id
    let device_id = request.device_id.trim();
    if device_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Device ID cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    let mut db = state.database.lock().await;

    // Check if device already exists
    if let Ok(Some(_)) = db.get_device(device_id) {
        return (
            StatusCode::CONFLICT,
            Json(ErrorResponse {
                error: format!("Device '{}' already exists", device_id),
            }),
        )
            .into_response();
    }

    // Create a new device with minimal information
    let device = Device {
        device_id: device_id.to_string(),
        ip_address: String::new(),
        mac_address: String::new(),
        firmware_version: request
            .firmware_version
            .filter(|s| !s.trim().is_empty())
            .unwrap_or_default(),
        last_updated: "1970-01-01T00:00:00Z".to_string(), // Unix epoch - indicates never updated
        ota_readiness_topic: String::new(),
        ota_mode_topic: String::new(),
        uses_deep_sleep: false,
        ota_port: None,
        state: crate::database::DeviceState::Idle,
        fail_count: 0,
        update_count: 0,
        rssi: 0,
        project_folder: request.project_folder.filter(|s| !s.trim().is_empty()),
        main_filename: request.main_filename.filter(|s| !s.trim().is_empty()),
    };

    match db.upsert_device(&device) {
        Ok(_) => {
            info!("Device '{}' added via web interface", device_id);
            (StatusCode::OK, Json(device)).into_response()
        }
        Err(e) => {
            error!("Failed to add device '{}': {}", device_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to add device: {}", e),
                }),
            )
                .into_response()
        }
    }
}

/// Update a device (requires authentication)
async fn update_device_handler(
    State(state): State<AppState>,
    session: Session,
    Json(request): Json<UpdateDeviceRequest>,
) -> Response {
    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    // Validate device IDs
    let old_device_id = request.old_device_id.trim();
    let new_device_id = request.new_device_id.trim();

    if old_device_id.is_empty() || new_device_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Device IDs cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    let mut db = state.database.lock().await;

    // Get the existing device
    let mut device = match db.get_device(old_device_id) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Device '{}' not found", old_device_id),
                }),
            )
                .into_response();
        }
        Err(e) => {
            error!("Failed to get device '{}': {}", old_device_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get device: {}", e),
                }),
            )
                .into_response();
        }
    };

    // If device ID changed, check if new ID already exists
    if old_device_id != new_device_id {
        if let Ok(Some(_)) = db.get_device(new_device_id) {
            return (
                StatusCode::CONFLICT,
                Json(ErrorResponse {
                    error: format!("Device '{}' already exists", new_device_id),
                }),
            )
                .into_response();
        }

        // Delete old device
        match db.delete_device(old_device_id) {
            Ok(_) => {
                info!("Old device '{}' deleted for rename", old_device_id);
            }
            Err(e) => {
                error!("Failed to delete old device '{}': {}", old_device_id, e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(ErrorResponse {
                        error: format!("Failed to delete old device: {}", e),
                    }),
                )
                    .into_response();
            }
        }
    }

    // Update device fields
    device.device_id = new_device_id.to_string();
    device.project_folder = request.project_folder.filter(|s| !s.trim().is_empty());
    device.main_filename = request.main_filename.filter(|s| !s.trim().is_empty());
    if let Some(firmware_version) = request.firmware_version.filter(|s| !s.trim().is_empty()) {
        device.firmware_version = firmware_version;
    }
    // Note: last_updated is not changed here - only updated on successful firmware upload

    // Save updated device
    match db.upsert_device(&device) {
        Ok(_) => {
            info!("Device '{}' updated via web interface", new_device_id);
            (StatusCode::OK, Json(device)).into_response()
        }
        Err(e) => {
            error!("Failed to update device '{}': {}", new_device_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to update device: {}", e),
                }),
            )
                .into_response()
        }
    }
}

/// Remove a device (requires authentication)
async fn remove_device_handler(
    State(state): State<AppState>,
    session: Session,
    Json(request): Json<RemoveDeviceRequest>,
) -> Response {
    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    let device_id = request.device_id.trim();

    if device_id.is_empty() {
        return (
            StatusCode::BAD_REQUEST,
            Json(ErrorResponse {
                error: "Device ID cannot be empty".to_string(),
            }),
        )
            .into_response();
    }

    let mut db = state.database.lock().await;

    // Check if device exists
    if db.get_device(device_id).ok().flatten().is_none() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Device '{}' not found", device_id),
            }),
        )
            .into_response();
    }

    // Delete the device
    match db.delete_device(device_id) {
        Ok(_) => {
            info!("Device '{}' removed via web interface", device_id);
            (StatusCode::OK, Json(serde_json::json!({ "success": true }))).into_response()
        }
        Err(e) => {
            error!("Failed to remove device '{}': {}", device_id, e);
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to remove device: {}", e),
                }),
            )
                .into_response()
        }
    }
}

/// Parse version string in X.Y.Z format into tuple of integers for comparison
fn parse_version(version: &str) -> Result<(u32, u32, u32), String> {
    let parts: Vec<&str> = version.split('.').collect();
    if parts.len() != 3 {
        return Err(format!("Invalid version format: {}", version));
    }

    let major = parts[0]
        .parse::<u32>()
        .map_err(|_| format!("Invalid major version: {}", parts[0]))?;
    let minor = parts[1]
        .parse::<u32>()
        .map_err(|_| format!("Invalid minor version: {}", parts[1]))?;
    let patch = parts[2]
        .parse::<u32>()
        .map_err(|_| format!("Invalid patch version: {}", parts[2]))?;

    Ok((major, minor, patch))
}

/// Rebuild device firmware (requires authentication)
async fn rebuild_device_handler(
    State(state): State<AppState>,
    session: Session,
    Json(request): Json<RebuildDeviceRequest>,
) -> Response {
    use std::path::PathBuf;
    use tokio::process::Command;

    if !is_authenticated(&session).await {
        return (StatusCode::UNAUTHORIZED, "Not authenticated").into_response();
    }

    let device_id = request.device_id.trim();

    // Get device from database
    let db = state.database.lock().await;
    let device = match db.get_device(device_id) {
        Ok(Some(d)) => d,
        Ok(None) => {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!("Device '{}' not found", device_id),
                }),
            )
                .into_response();
        }
        Err(e) => {
            error!("Failed to get device '{}': {}", device_id, e);
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to get device: {}", e),
                }),
            )
                .into_response();
        }
    };
    drop(db);

    // Check if device has project_folder
    let project_folder = match &device.project_folder {
        Some(pf) if !pf.is_empty() => pf,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "Device has no project_folder configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Check if esphome_projects is enabled
    let esphome_config = match &state.config.esphome_projects {
        Some(cfg) if cfg.enable => cfg,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "ESPHome projects functionality is not enabled".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Get projects folder
    let projects_folder = match &esphome_config.projects_folder {
        Some(pf) if !pf.is_empty() => pf,
        _ => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "ESPHome projects_folder is not configured".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Build full project path
    let mut project_path = PathBuf::from(projects_folder);
    project_path.push(project_folder);

    if !project_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("Project directory not found: {}", project_path.display()),
            }),
        )
            .into_response();
    }

    // Determine main filename
    let main_filename = device
        .main_filename
        .as_ref()
        .filter(|s| !s.is_empty())
        .unwrap_or(&esphome_config.default_main_filename);

    let yaml_path = project_path.join(main_filename);
    if !yaml_path.exists() {
        return (
            StatusCode::NOT_FOUND,
            Json(ErrorResponse {
                error: format!("ESPHome YAML file not found: {}", yaml_path.display()),
            }),
        )
            .into_response();
    }

    // Parse and validate the YAML file
    let yaml_content = match tokio::fs::read_to_string(&yaml_path).await {
        Ok(content) => content,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: format!("Failed to read YAML file: {}", e),
                }),
            )
                .into_response();
        }
    };

    let yaml_data: serde_yml::Value = match serde_yml::from_str(&yaml_content) {
        Ok(data) => data,
        Err(e) => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: format!("Failed to parse YAML file: {}", e),
                }),
            )
                .into_response();
        }
    };

    // Extract version from YAML (check project.version or substitutions.firmware_version)
    let yaml_version_raw = yaml_data
        .get("project")
        .and_then(|p| p.get("version"))
        .and_then(|v| v.as_str())
        .or_else(|| {
            yaml_data
                .get("substitutions")
                .and_then(|s| s.get("firmware_version"))
                .and_then(|v| v.as_str())
        });

    // Resolve substitutions in version string if present
    let yaml_version = if let Some(version_str) = yaml_version_raw {
        if version_str.contains("${") && version_str.contains("}") {
            // Handle substitution in version string
            let mut resolved_version = version_str.to_string();

            // Find all ${...} patterns and replace them
            while let Some(start) = resolved_version.find("${") {
                if let Some(end) = resolved_version[start..].find("}") {
                    let var_name = &resolved_version[start + 2..start + end];

                    // Look up the variable in substitutions
                    let substituted_value = yaml_data
                        .get("substitutions")
                        .and_then(|s| s.get(var_name))
                        .and_then(|v| v.as_str());

                    match substituted_value {
                        Some(value) => {
                            resolved_version =
                                resolved_version.replace(&format!("${{{}}}", var_name), value);
                        }
                        None => {
                            return (
                                StatusCode::BAD_REQUEST,
                                Json(ErrorResponse {
                                    error: format!(
                                        "Version string references undefined substitution variable '{}'. Please define it in the substitutions section.",
                                        var_name
                                    ),
                                }),
                            )
                                .into_response();
                        }
                    }
                } else {
                    break;
                }
            }
            Some(resolved_version)
        } else {
            Some(version_str.to_string())
        }
    } else {
        None
    };

    // Extract esphome.name field - this is required to locate the compiled firmware
    let esphome_name_raw = yaml_data
        .get("esphome")
        .and_then(|e| e.get("name"))
        .and_then(|n| n.as_str());

    let esphome_name = match esphome_name_raw {
        Some(name) => {
            // Check if the name contains ${...} substitution syntax
            if name.contains("${") && name.contains("}") {
                // Extract the variable name from ${variable_name}
                let start = name.find("${").unwrap() + 2;
                let end = name.find("}").unwrap();
                let var_name = &name[start..end];

                // Look up the variable in substitutions
                let substituted_value = yaml_data
                    .get("substitutions")
                    .and_then(|s| s.get(var_name))
                    .and_then(|v| v.as_str());

                match substituted_value {
                    Some(value) => {
                        // Replace ${variable_name} with the actual value
                        name.replace(&format!("${{{}}}", var_name), value)
                    }
                    None => {
                        return (
                            StatusCode::BAD_REQUEST,
                            Json(ErrorResponse {
                                error: format!(
                                    "ESPHome name references undefined substitution variable '{}'. Please define it in the substitutions section.",
                                    var_name
                                ),
                            }),
                        )
                            .into_response();
                    }
                }
            } else {
                name.to_string()
            }
        }
        None => {
            return (
                StatusCode::BAD_REQUEST,
                Json(ErrorResponse {
                    error: "No 'esphome.name' field found in YAML file. This field is required to locate the compiled firmware.".to_string(),
                }),
            )
                .into_response();
        }
    };

    // Validate version is greater than current version
    if let Some(yaml_ver) = &yaml_version {
        let current_version = &device.firmware_version;
        if !current_version.is_empty() {
            // Compare versions (simple string comparison for X.Y.Z format)
            if let (Ok(yaml_parts), Ok(current_parts)) =
                (parse_version(yaml_ver), parse_version(current_version))
            {
                if yaml_parts <= current_parts {
                    return (
                        StatusCode::BAD_REQUEST,
                        Json(ErrorResponse {
                            error: format!(
                                "Version validation failed: YAML version '{}' must be greater than current version '{}'. Please update the version in your ESPHome YAML configuration.",
                                yaml_ver, current_version
                            ),
                        }),
                    )
                        .into_response();
                }
            }
        }
    }

    info!(
        "Building firmware for device '{}' from project: {}",
        device_id,
        project_path.display()
    );

    // Build the esphome command, optionally with venv activation
    let output = if let Some(venv_folder) = &esphome_config.esphome_venv_folder {
        // If venv folder is specified, activate it before running esphome
        let venv_activate = PathBuf::from(venv_folder).join("bin/activate");

        if !venv_activate.exists() {
            return (
                StatusCode::NOT_FOUND,
                Json(ErrorResponse {
                    error: format!(
                        "Virtual environment activate script not found: {}",
                        venv_activate.display()
                    ),
                }),
            )
                .into_response();
        }

        let command = format!(
            "source {} && esphome compile {}",
            venv_activate.display(),
            main_filename
        );

        info!("Executing with venv: {}", command);

        match Command::new("bash")
            .arg("-c")
            .arg(&command)
            .current_dir(&project_path)
            .output()
            .await
        {
            Ok(output) => output,
            Err(e) => {
                error!("Failed to execute esphome command with venv: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(RebuildResponse {
                        success: false,
                        output: format!("Failed to execute esphome command with venv: {}", e),
                        firmware_path: None,
                    }),
                )
                    .into_response();
            }
        }
    } else {
        // No venv, run esphome directly
        match Command::new("esphome")
            .arg("compile")
            .arg(main_filename)
            .current_dir(&project_path)
            .output()
            .await
        {
            Ok(output) => output,
            Err(e) => {
                error!("Failed to execute esphome command: {}", e);
                return (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    Json(RebuildResponse {
                        success: false,
                        output: format!("Failed to execute esphome command: {}", e),
                        firmware_path: None,
                    }),
                )
                    .into_response();
            }
        }
    };

    let stdout = String::from_utf8_lossy(&output.stdout);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let combined_output = format!("{}\n{}", stdout, stderr);

    if !output.status.success() {
        return (
            StatusCode::OK,
            Json(RebuildResponse {
                success: false,
                output: combined_output,
                firmware_path: None,
            }),
        )
            .into_response();
    }

    // If successful, copy firmware to firmware folder
    // The firmware is located at: .esphome/build/<esphome_name>/.pioenvs/<esphome_name>/firmware.bin
    // where <esphome_name> is the value from the esphome.name field in the YAML

    let source_firmware = project_path
        .join(".esphome/build")
        .join(&esphome_name)
        .join(".pioenvs")
        .join(&esphome_name)
        .join("firmware.bin");

    if !source_firmware.exists() {
        return (
            StatusCode::OK,
            Json(RebuildResponse {
                success: true,
                output: format!("{}\n\nWARNING: Compilation succeeded but firmware binary not found at expected location: {}\nMake sure your ESPHome YAML has 'esphome.name' set to '{}'.", 
                    combined_output, source_firmware.display(), esphome_name),
                firmware_path: None,
            }),
        )
            .into_response();
    }

    // Copy to firmware folder with proper naming: <device_id> - <version>.bin
    // Use the version from YAML (the one that was just compiled)
    let firmware_filename = if let Some(version) = yaml_version {
        format!("{} - {}.bin", device_id, version)
    } else {
        format!("{} - unknown.bin", device_id)
    };

    let firmware_folder = PathBuf::from(&state.config.firmware.storage_path);
    let destination = firmware_folder.join(&firmware_filename);

    match tokio::fs::copy(&source_firmware, &destination).await {
        Ok(_) => {
            info!("Firmware copied to: {}", destination.display());
            (
                StatusCode::OK,
                Json(RebuildResponse {
                    success: true,
                    output: format!(
                        "{}\n\nSUCCESS: Firmware built and copied to: {}",
                        combined_output,
                        destination.display()
                    ),
                    firmware_path: Some(destination.display().to_string()),
                }),
            )
                .into_response()
        }
        Err(e) => {
            error!("Failed to copy firmware: {}", e);
            (
                StatusCode::OK,
                Json(RebuildResponse {
                    success: true,
                    output: format!(
                        "{}\n\nWARNING: Compilation succeeded but failed to copy firmware: {}",
                        combined_output, e
                    ),
                    firmware_path: None,
                }),
            )
                .into_response()
        }
    }
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
            max_concurrent_updates: config.service.max_concurrent_updates,
            check_interval: config.service.check_interval,
            ota_password: config
                .service
                .ota_password
                .as_ref()
                .map(|_| mask_string("password")),
            default_ota_port: config.service.default_ota_port,
        },
        firmware: MaskedFirmwareConfig {
            storage_path: config.firmware.storage_path.clone(),
            erase_firmware_after_upload: config.firmware.erase_firmware_after_upload,
        },
        pushover: config.pushover.as_ref().map(|p| MaskedPushoverConfig {
            api_token: mask_string(&p.api_token),
            user_key: mask_string(&p.user_key),
            device: p.device.clone(),
            priority: p.priority,
            enabled: p.enabled,
        }),
        home_assistant: config
            .home_assistant
            .as_ref()
            .map(|ha| HomeAssistantConfigView {
                enabled: ha.enabled,
                discovery_prefix: ha.discovery_prefix.clone(),
                node_id: ha.node_id.clone(),
                device_name: ha.device_name.clone(),
                manufacturer: ha.manufacturer.clone(),
                model: ha.model.clone(),
                update_interval: ha.update_interval,
            }),
        esphome_projects: config
            .esphome_projects
            .as_ref()
            .map(|ep| EsphomeProjectsConfigView {
                enable: ep.enable,
                projects_folder: ep.projects_folder.clone(),
                default_main_filename: ep.default_main_filename.clone(),
                esphome_venv_folder: ep.esphome_venv_folder.clone(),
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
        "keep_alive" | "check_interval" | "refresh_period" | "edit_session_timeout" => {
            cleaned_value
                .parse::<u64>()
                .map(|v| serde_yml::Value::Number(serde_yml::Number::from(v)))
                .map_err(|_| format!("Invalid duration value for {}", key))
        }
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
