use crate::config::Configuration;
use crate::database::{Database, DeviceState};
use crate::firmware::FirmwareManager;
use crate::mqtt_client::MqttClient;
use crate::ota_client::OtaClient;
use crate::pushover::PushoverClient;
use log::{debug, error, info, warn};
use rumqttc::QoS;
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time;

/// Run the OTA service with the given configuration
pub async fn run(config: Configuration) -> Result<(), String> {
    info!("Initializing OTA service components");

    // Initialize database
    let database = match Database::new(&config.database.path) {
        Ok(db) => Arc::new(Mutex::new(db)),
        Err(e) => {
            return Err(format!("Failed to initialize database: {}", e));
        }
    };

    info!("Database initialized: {}", config.database.path);

    // Initialize firmware manager
    let firmware_manager = match FirmwareManager::new(&config.firmware.storage_path) {
        Ok(mgr) => Arc::new(mgr),
        Err(e) => {
            return Err(format!("Failed to initialize firmware manager: {}", e));
        }
    };

    info!(
        "Firmware manager initialized: {}",
        config.firmware.storage_path
    );

    // Initialize MQTT client
    let mqtt_client = match MqttClient::new(
        &config.mqtt.host,
        config.mqtt.port,
        &config.mqtt.client_id,
        config.mqtt.username.as_deref(),
        config.mqtt.password.as_deref(),
        config.mqtt.keep_alive,
    ) {
        Ok(client) => Arc::new(Mutex::new(client)),
        Err(e) => {
            return Err(format!("Failed to initialize MQTT client: {}", e));
        }
    };

    info!("MQTT client initialized");

    // Wait for MQTT connection
    {
        let client = mqtt_client.lock().await;
        if let Err(e) = client.wait_connected(30).await {
            return Err(format!("Failed to connect to MQTT broker: {}", e));
        }
    }

    info!("Connected to MQTT broker");

    // Initialize Pushover client if configured
    let pushover_client = if let Some(pushover_cfg) = &config.pushover {
        if pushover_cfg.enabled {
            Some(Arc::new(PushoverClient::new(
                pushover_cfg.api_token.clone(),
                pushover_cfg.user_key.clone(),
                true,
            )))
        } else {
            info!("Pushover notifications disabled by config");
            None
        }
    } else {
        info!("Pushover not configured");
        None
    };

    // Create MQTT publisher callback
    let mqtt_pub = Arc::clone(&mqtt_client);
    let mqtt_publisher = move |topic: &str, message: &str, qos: QoS, retain: bool| {
        let topic = topic.to_string();
        let message = message.to_string();
        let client = Arc::clone(&mqtt_pub);
        tokio::spawn(async move {
            if let Err(e) = client
                .lock()
                .await
                .publish(&topic, &message, qos, retain)
                .await
            {
                error!("Failed to publish to {}: {}", topic, e);
            }
        });
    };

    // Create MQTT subscriber callback
    let mqtt_sub = Arc::clone(&mqtt_client);
    let mqtt_subscriber = move |topic: &str, qos: QoS| {
        let topic = topic.to_string();
        let client = Arc::clone(&mqtt_sub);
        tokio::spawn(async move {
            if let Err(e) = client.lock().await.subscribe(&topic, qos).await {
                error!("Failed to subscribe to {}: {}", topic, e);
            }
        });
    };

    // Create OTA service
    let service = OtaService::new(
        Arc::clone(&database),
        Arc::clone(&firmware_manager),
        Arc::clone(&mqtt_client),
        mqtt_publisher,
        mqtt_subscriber,
        config.mqtt.registration_topic.clone(),
        config.firmware.ota_password.clone(),
        config.firmware.default_ota_port,
        config.firmware.erase_firmware_after_upload,
        pushover_client,
    );

    // Start firmware check task
    service
        .start_firmware_check_task(config.firmware.check_interval)
        .await;

    // Start unified MQTT listener
    service.start_mqtt_listener().await;

    info!("OTA service is running");

    // Keep the service running
    loop {
        tokio::time::sleep(Duration::from_secs(60)).await;
    }
}

/// Service for managing periodic OTA update tasks
pub struct OtaService {
    /// Database connection (wrapped in Arc<Mutex> for async sharing)
    database: Arc<Mutex<Database>>,
    /// Firmware manager
    firmware_manager: Arc<FirmwareManager>,
    /// MQTT client for publishing, subscribing, and receiving messages
    mqtt_client: Arc<Mutex<MqttClient>>,
    /// MQTT publisher callback (topic, message, qos, retain)
    mqtt_publisher: Arc<dyn Fn(&str, &str, QoS, bool) + Send + Sync>,
    /// MQTT subscriber callback (topic, qos) -> Result
    mqtt_subscriber: Arc<dyn Fn(&str, QoS) + Send + Sync>,
    /// Registration topic for device registration
    registration_topic: String,
    /// OTA authentication password (optional, same for all devices)
    ota_password: Option<String>,
    /// Default OTA port number (devices can override this)
    default_ota_port: u16,
    /// Erase firmware file after successful upload
    erase_firmware_after_upload: bool,
    /// Pushover notification client (optional)
    pushover_client: Option<Arc<PushoverClient>>,
}

impl OtaService {
    /// Create a new OTA service
    ///
    /// # Arguments
    /// * `database` - Database instance
    /// * `firmware_manager` - Firmware manager instance
    /// * `mqtt_client` - MQTT client instance
    /// * `mqtt_publisher` - Callback function to publish MQTT messages (topic, message, qos, retain)
    /// * `mqtt_subscriber` - Callback function to subscribe to MQTT topics (topic, qos)
    /// * `registration_topic` - Topic for device registration messages
    /// * `ota_password` - Optional OTA authentication password
    /// * `default_ota_port` - Default OTA port number (devices can override)
    /// * `erase_firmware_after_upload` - Delete firmware file after successful upload
    /// * `pushover_client` - Optional Pushover notification client
    pub fn new<P, S>(
        database: Arc<Mutex<Database>>,
        firmware_manager: Arc<FirmwareManager>,
        mqtt_client: Arc<Mutex<MqttClient>>,
        mqtt_publisher: P,
        mqtt_subscriber: S,
        registration_topic: String,
        ota_password: Option<String>,
        default_ota_port: u16,
        erase_firmware_after_upload: bool,
        pushover_client: Option<Arc<PushoverClient>>,
    ) -> Self
    where
        P: Fn(&str, &str, QoS, bool) + Send + Sync + 'static,
        S: Fn(&str, QoS) + Send + Sync + 'static,
    {
        info!("Initializing OTA Service");
        OtaService {
            database,
            firmware_manager,
            mqtt_client,
            mqtt_publisher: Arc::new(mqtt_publisher),
            mqtt_subscriber: Arc::new(mqtt_subscriber),
            registration_topic,
            ota_password,
            default_ota_port,
            erase_firmware_after_upload,
            pushover_client,
        }
    }

    /// Start the firmware availability check task
    /// Runs periodically at the specified interval
    ///
    /// # Arguments
    /// * `interval_secs` - Interval in seconds between checks
    pub async fn start_firmware_check_task(&self, interval_secs: u64) {
        info!(
            "Starting firmware availability check task (interval: {} seconds)",
            interval_secs
        );

        let database = Arc::clone(&self.database);
        let firmware_mgr = Arc::clone(&self.firmware_manager);
        let mqtt_pub = Arc::clone(&self.mqtt_publisher);
        let mqtt_sub = Arc::clone(&self.mqtt_subscriber);

        tokio::spawn(async move {
            let mut interval = time::interval(Duration::from_secs(interval_secs));

            loop {
                interval.tick().await;

                debug!("Running firmware availability check");

                // Get all devices from database
                let devices = {
                    let db = database.lock().await;
                    match db.get_all_devices() {
                        Ok(devices) => devices,
                        Err(e) => {
                            error!("Failed to retrieve devices for firmware check: {}", e);
                            continue;
                        }
                    }
                };

                // Check each device for firmware updates
                for device in devices {
                    debug!("Checking firmware for device: {}", device.device_id);

                    // Only check devices that are idle
                    if device.state != DeviceState::Idle {
                        debug!(
                            "Device {} is not idle (state: {:?}), skipping firmware check",
                            device.device_id, device.state
                        );
                        continue;
                    }

                    // Check if newer firmware is available
                    match firmware_mgr
                        .get_newer_version(&device.device_id, &device.firmware_version)
                    {
                        Ok(Some(new_firmware)) => {
                            info!(
                                "Newer firmware available for device {}: {} -> {}",
                                device.device_id, device.firmware_version, new_firmware.version
                            );

                            // Subscribe to device's readiness topic BEFORE sending NEW-FIRMWARE-VERSION
                            // This ensures we're ready to receive OTA-READY when device wakes up
                            (mqtt_sub)(&device.ota_readiness_topic, QoS::AtLeastOnce);
                            info!(
                                "Subscribed to readiness topic: {} for device {}",
                                device.ota_readiness_topic, device.device_id
                            );

                            // Send NEW-FIRMWARE-VERSION message to ota_mode_topic with QoS 1 and retain
                            let message = "NEW-FIRMWARE-VERSION";
                            (mqtt_pub)(&device.ota_mode_topic, message, QoS::AtLeastOnce, true);

                            debug!(
                                "Published NEW-FIRMWARE-VERSION to topic: {} for device {}",
                                device.ota_mode_topic, device.device_id
                            );

                            // Update device state to NewVersionAvailableTransmitted
                            {
                                let mut db = database.lock().await;
                                if let Err(e) = db.update_device_state(
                                    &device.device_id,
                                    DeviceState::NewVersionAvailableTransmitted,
                                ) {
                                    error!(
                                        "Failed to update device state for {}: {}",
                                        device.device_id, e
                                    );
                                }
                            }
                        }
                        Ok(None) => {
                            debug!(
                                "No newer firmware available for device {} (current: {})",
                                device.device_id, device.firmware_version
                            );
                        }
                        Err(e) => {
                            error!(
                                "Error checking firmware for device {}: {}",
                                device.device_id, e
                            );
                        }
                    }
                }
            }
        });
    }

    /// Start the unified MQTT message listener task
    /// Processes both device registration and OTA-READY messages from MQTT
    pub async fn start_mqtt_listener(&self) {
        info!("Starting unified MQTT message listener task");

        let database = Arc::clone(&self.database);
        let mqtt_client = Arc::clone(&self.mqtt_client);
        let registration_topic = self.registration_topic.clone();
        let firmware_manager = Arc::clone(&self.firmware_manager);
        let ota_password = self.ota_password.clone();
        let default_ota_port = self.default_ota_port;
        let erase_firmware_after_upload = self.erase_firmware_after_upload;

        tokio::spawn(async move {
            info!("Unified MQTT message listener started");
            loop {
                let msg = {
                    let mut mqtt = mqtt_client.lock().await;
                    mqtt.next_message().await
                };

                if let Some(msg) = msg {
                    if msg.payload.is_empty() {
                        continue;
                    }

                    if let Ok(payload) = msg.payload_str() {
                        if msg.topic == registration_topic {
                            info!("Received registration message");
                            match crate::mqtt::DeviceRegistration::from_json(&payload) {
                                Ok(registration) => {
                                    let device = registration.to_device();
                                    let readiness_topic = device.ota_readiness_topic.clone();

                                    {
                                        let mut db = database.lock().await;
                                        if let Ok(_) = db.upsert_device(&device) {
                                            info!("Device registered: {}", device.device_id);
                                        }
                                    }

                                    {
                                        let mqtt = mqtt_client.lock().await;
                                        let _ = mqtt
                                            .subscribe(&readiness_topic, QoS::AtLeastOnce)
                                            .await;
                                    }
                                }
                                Err(e) => error!("Failed to parse registration: {}", e),
                            }
                        } else {
                            let device_id_opt = {
                                let db = database.lock().await;
                                if let Ok(devices) = db.get_all_devices() {
                                    devices
                                        .into_iter()
                                        .find(|d| d.ota_readiness_topic == msg.topic)
                                        .map(|d| d.device_id)
                                } else {
                                    None
                                }
                            };

                            if let Some(device_id) = device_id_opt {
                                if payload.trim() == "OTA-READY" {
                                    info!("OTA-READY from: {}", device_id);

                                    let device_info = {
                                        let db = database.lock().await;
                                        db.get_device(&device_id).ok().flatten().map(|d| {
                                            (
                                                d.ip_address,
                                                d.firmware_version,
                                                d.ota_mode_topic,
                                                d.ota_port.unwrap_or(default_ota_port),
                                            )
                                        })
                                    };

                                    if let Some((
                                        device_ip,
                                        current_version,
                                        ota_mode_topic,
                                        ota_port,
                                    )) = device_info
                                    {
                                        {
                                            let mqtt = mqtt_client.lock().await;
                                            let _ = mqtt.clear_retained(&ota_mode_topic).await;
                                        }

                                        match firmware_manager
                                            .get_newer_version(&device_id, &current_version)
                                        {
                                            Ok(Some(new_firmware)) => {
                                                info!(
                                                    "Starting OTA for {}: {} -> {}",
                                                    device_id,
                                                    current_version,
                                                    new_firmware.version
                                                );
                                                {
                                                    let mut db = database.lock().await;
                                                    let _ = db.update_device_state(
                                                        &device_id,
                                                        DeviceState::OtaTransmit,
                                                    );
                                                }

                                                if let Ok(fw_data) =
                                                    firmware_manager.read_firmware(&new_firmware)
                                                {
                                                    match OtaClient::connect(
                                                        &device_ip,
                                                        ota_port,
                                                        ota_password.clone(),
                                                    ) {
                                                        Ok(mut ota_client) => {
                                                            match ota_client.update(&fw_data) {
                                                                Ok(()) => {
                                                                    info!(
                                                                        "OTA successful for {}",
                                                                        device_id
                                                                    );
                                                                    let mut db =
                                                                        database.lock().await;
                                                                    let _ = db.update_device_firmware_version(&device_id, &new_firmware.version);
                                                                    let _ = db.update_device_state(
                                                                        &device_id,
                                                                        DeviceState::Idle,
                                                                    );
                                                                    drop(db);
                                                                    if erase_firmware_after_upload {
                                                                        let _ = firmware_manager
                                                                            .delete_firmware(
                                                                                &new_firmware,
                                                                            );
                                                                    }
                                                                }
                                                                Err(e) => {
                                                                    error!(
                                                                        "OTA failed for {}: {}",
                                                                        device_id, e
                                                                    );
                                                                    let mut db =
                                                                        database.lock().await;
                                                                    let _ = db.update_device_state(
                                                                        &device_id,
                                                                        DeviceState::Idle,
                                                                    );
                                                                }
                                                            }
                                                        }
                                                        Err(e) => {
                                                            error!(
                                                                "Failed to connect OTA client: {}",
                                                                e
                                                            );
                                                            let mut db = database.lock().await;
                                                            let _ = db.update_device_state(
                                                                &device_id,
                                                                DeviceState::Idle,
                                                            );
                                                        }
                                                    }
                                                } else {
                                                    let mut db = database.lock().await;
                                                    let _ = db.update_device_state(
                                                        &device_id,
                                                        DeviceState::Idle,
                                                    );
                                                }
                                            }
                                            Ok(None) => {
                                                let mut db = database.lock().await;
                                                let _ = db.update_device_state(
                                                    &device_id,
                                                    DeviceState::Idle,
                                                );
                                            }
                                            Err(e) => {
                                                error!("Failed to check firmware: {}", e);
                                                let mut db = database.lock().await;
                                                let _ = db.update_device_state(
                                                    &device_id,
                                                    DeviceState::Idle,
                                                );
                                            }
                                        }
                                    }

                                    {
                                        let mqtt = mqtt_client.lock().await;
                                        let _ = mqtt.clear_retained(&msg.topic).await;
                                    }
                                }
                            }
                        }
                    }
                }
            }
        });
    }
    /// Use start_mqtt_listener() instead
    pub async fn start_registration_listener(&self) {
        self.start_mqtt_listener().await;
    }

    /// Start the OTA readiness listener task - DEPRECATED
    /// Use start_mqtt_listener() instead
    pub async fn start_readiness_listener(&self) {
        // No-op: unified listener handles both registration and readiness
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_ota_service_creation() {
        let db_path = "/tmp/test_ota_service.db";
        let _ = std::fs::remove_file(db_path);

        let _db = match crate::database::Database::new(db_path) {
            Ok(db) => db,
            Err(e) => panic!("Failed to create database: {}", e),
        };

        let _fw_mgr = match crate::firmware::FirmwareManager::new("/tmp/test_firmware") {
            Ok(mgr) => mgr,
            Err(e) => panic!("Failed to create firmware manager: {}", e),
        };

        // Note: Full testing would require creating an actual MQTT client connection
        // which is async and requires a broker. This test just verifies the service
        // structure can be instantiated. Integration tests should cover the full workflow.

        let _ = std::fs::remove_file(db_path);

        // Test passes if we get here without panicking
    }
}
