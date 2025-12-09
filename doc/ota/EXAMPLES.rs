// OTA Service Code Examples
//
// This file contains 7 practical examples demonstrating various OTA functionality.
// These examples show real-world usage patterns and best practices.

#![allow(dead_code, unused_variables, unused_imports)]

use std::sync::Arc;
use tokio::sync::Mutex;

// Mock imports (replace with actual crate usage)
mod mock {
    use std::sync::Arc;
    use tokio::sync::Mutex;

    pub struct Configuration {
        pub mqtt: MqttConfig,
        pub database: DatabaseConfig,
        pub service: ServiceConfig,
        pub firmware: FirmwareConfig,
        pub pushover: Option<PushoverConfig>,
    }

    pub struct MqttConfig {
        pub host: String,
        pub port: u16,
        pub client_id: String,
        pub username: Option<String>,
        pub password: Option<String>,
        pub registration_topic: String,
    }

    pub struct DatabaseConfig {
        pub db_path: String,
    }

    pub struct ServiceConfig {
        pub firmware_dir: String,
    }

    pub struct FirmwareConfig {
        pub firmware_dir: String,
        pub ota_password: String,
        pub default_ota_port: u16,
        pub check_interval: u64,
        pub erase_firmware_after_upload: bool,
    }

    pub struct PushoverConfig {
        pub enabled: bool,
        pub api_token: String,
        pub user_key: String,
    }

    pub struct Database {
        // Mock
    }

    impl Database {
        pub fn new(_path: &str) -> Result<Self, String> {
            Ok(Database {})
        }

        pub async fn upsert_device(&mut self, _device: &Device) -> Result<(), String> {
            Ok(())
        }

        pub async fn get_device(&self, _id: &str) -> Result<Option<Device>, String> {
            Ok(None)
        }

        pub async fn get_all_devices(&self) -> Result<Vec<Device>, String> {
            Ok(vec![])
        }

        pub async fn update_device_firmware_version(
            &mut self,
            _id: &str,
            _version: &str,
        ) -> Result<(), String> {
            Ok(())
        }

        pub async fn update_device_state(
            &mut self,
            _id: &str,
            _state: DeviceState,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    pub struct MqttClient {
        // Mock
    }

    impl MqttClient {
        pub fn new(
            _host: &str,
            _port: u16,
            _client_id: &str,
            _username: Option<&str>,
            _password: Option<&str>,
            _keep_alive: u64,
        ) -> Result<Self, String> {
            Ok(MqttClient {})
        }

        pub async fn subscribe(&self, _topic: &str, _qos: QoS) -> Result<(), String> {
            Ok(())
        }

        pub async fn publish(
            &self,
            _topic: &str,
            _payload: &str,
            _qos: QoS,
            _retain: bool,
        ) -> Result<(), String> {
            Ok(())
        }
    }

    pub struct FirmwareManager {
        // Mock
    }

    impl FirmwareManager {
        pub fn new(_dir: &str) -> Self {
            FirmwareManager {}
        }

        pub fn get_newer_version(
            &self,
            _device_id: &str,
            _current: &str,
        ) -> Result<Option<FirmwareInfo>, String> {
            Ok(None)
        }

        pub fn read_firmware(&self, _info: &FirmwareInfo) -> Result<Vec<u8>, String> {
            Ok(vec![])
        }
    }

    pub struct FirmwareInfo {
        pub device_id: String,
        pub version: String,
        pub file_path: std::path::PathBuf,
    }

    pub struct OtaClient {
        // Mock
    }

    impl OtaClient {
        pub fn connect(_ip: &str, _port: u16, _password: String) -> Result<Self, String> {
            Ok(OtaClient {})
        }

        pub fn update(&mut self, _firmware: &[u8]) -> Result<(), String> {
            Ok(())
        }
    }

    pub struct Device {
        pub device_id: String,
        pub ip_address: String,
        pub mac_address: String,
        pub firmware_version: String,
        pub ota_port: Option<u16>,
        pub ota_readiness_topic: String,
        pub ota_mode_topic: String,
        pub device_state: DeviceState,
        pub last_seen: String,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum DeviceState {
        Idle,
        NewVersionAvailableTransmitted,
        OtaTransmit,
    }

    #[derive(Debug, Clone, Copy)]
    pub enum QoS {
        AtMostOnce,
        AtLeastOnce,
        ExactlyOnce,
    }
}

use mock::*;

// ============================================================================
// Example 1: Basic Service Setup
// ============================================================================
// Shows how to create and configure the OTA service with all components

async fn example_1_basic_service_setup() -> Result<(), String> {
    // Create configuration
    let config = Configuration {
        mqtt: MqttConfig {
            host: "localhost".to_string(),
            port: 1883,
            client_id: "ota-service".to_string(),
            username: Some("mqtt_user".to_string()),
            password: Some("mqtt_pass".to_string()),
            registration_topic: "home/ota/registration".to_string(),
        },
        database: DatabaseConfig {
            db_path: "/var/lib/ota-service/ota.db".to_string(),
        },
        service: ServiceConfig {
            firmware_dir: "/var/lib/ota-service/firmware".to_string(),
        },
        firmware: FirmwareConfig {
            firmware_dir: "/var/lib/ota-service/firmware".to_string(),
            ota_password: "secure123".to_string(),
            default_ota_port: 3232,
            check_interval: 300, // 5 minutes
            erase_firmware_after_upload: false,
        },
        pushover: None,
    };

    // Create database
    let database = Database::new(&config.database.db_path)?;
    let database = Arc::new(Mutex::new(database));

    // Create MQTT client
    let mqtt_client = MqttClient::new(
        &config.mqtt.host,
        config.mqtt.port,
        &config.mqtt.client_id,
        config.mqtt.username.as_deref(),
        config.mqtt.password.as_deref(),
        60, // keep_alive
    )?;
    let mqtt_client = Arc::new(Mutex::new(mqtt_client));

    // Create firmware manager
    let firmware_manager = FirmwareManager::new(&config.firmware.firmware_dir);
    let firmware_manager = Arc::new(firmware_manager);

    println!("OTA Service initialized successfully");

    Ok(())
}

// ============================================================================
// Example 2: Device Registration
// ============================================================================
// Shows how to handle device registration from MQTT

async fn example_2_device_registration() -> Result<(), String> {
    let database = Arc::new(Mutex::new(Database::new("ota.db")?));
    let mqtt_client = Arc::new(Mutex::new(MqttClient::new(
        "localhost",
        1883,
        "ota-service",
        None,
        None,
        60,
    )?));

    // Simulate receiving registration message
    let registration_json = r#"{
        "device_id": "esp32-001",
        "ip_address": "192.168.1.100",
        "mac_address": "AA:BB:CC:DD:EE:FF",
        "firmware_version": "1.0.0",
        "ota_port": 3232,
        "ota_readiness_topic": "home/esp32-001/ota/ready",
        "ota_mode_topic": "home/esp32-001/ota/mode"
    }"#;

    // Parse registration (in real code, use DeviceRegistration::from_json)
    let device = Device {
        device_id: "esp32-001".to_string(),
        ip_address: "192.168.1.100".to_string(),
        mac_address: "AA:BB:CC:DD:EE:FF".to_string(),
        firmware_version: "1.0.0".to_string(),
        ota_port: Some(3232),
        ota_readiness_topic: "home/esp32-001/ota/ready".to_string(),
        ota_mode_topic: "home/esp32-001/ota/mode".to_string(),
        device_state: DeviceState::Idle,
        last_seen: chrono::Utc::now().to_rfc3339(),
    };

    // Store in database
    {
        let mut db = database.lock().await;
        db.upsert_device(&device).await?;
    }

    // Subscribe to readiness topic
    {
        let mqtt = mqtt_client.lock().await;
        mqtt.subscribe(&device.ota_readiness_topic, QoS::AtLeastOnce)
            .await?;
    }

    println!("Device {} registered successfully", device.device_id);

    Ok(())
}

// ============================================================================
// Example 3: Firmware Version Check
// ============================================================================
// Shows how to check for newer firmware and notify device

async fn example_3_firmware_version_check() -> Result<(), String> {
    let database = Arc::new(Mutex::new(Database::new("ota.db")?));
    let mqtt_client = Arc::new(Mutex::new(MqttClient::new(
        "localhost",
        1883,
        "ota-service",
        None,
        None,
        60,
    )?));
    let firmware_manager = Arc::new(FirmwareManager::new("/var/lib/ota-service/firmware"));

    // Get all devices
    let devices = {
        let db = database.lock().await;
        db.get_all_devices().await?
    };

    // Check each device for newer firmware
    for device in devices {
        match firmware_manager.get_newer_version(&device.device_id, &device.firmware_version) {
            Ok(Some(new_firmware)) => {
                println!(
                    "New firmware available for {}: {} -> {}",
                    device.device_id, device.firmware_version, new_firmware.version
                );

                // Update device state
                {
                    let mut db = database.lock().await;
                    db.update_device_state(
                        &device.device_id,
                        DeviceState::NewVersionAvailableTransmitted,
                    )
                    .await?;
                }

                // Send MQTT notification
                {
                    let mqtt = mqtt_client.lock().await;
                    mqtt.publish(&device.ota_mode_topic, "ON", QoS::AtLeastOnce, true)
                        .await?;
                }

                println!("Notification sent to {}", device.device_id);
            }
            Ok(None) => {
                println!(
                    "{} is up to date ({})",
                    device.device_id, device.firmware_version
                );
            }
            Err(e) => {
                eprintln!("Failed to check firmware for {}: {}", device.device_id, e);
            }
        }
    }

    Ok(())
}

// ============================================================================
// Example 4: OTA Update Execution
// ============================================================================
// Shows the complete OTA update process

async fn example_4_ota_update_execution() -> Result<(), String> {
    let database = Arc::new(Mutex::new(Database::new("ota.db")?));
    let firmware_manager = Arc::new(FirmwareManager::new("/var/lib/ota-service/firmware"));
    let ota_password = "secure123".to_string();
    let default_port = 3232u16;

    // Get device information
    let device_id = "esp32-001";
    let device = {
        let db = database.lock().await;
        db.get_device(device_id).await?.ok_or("Device not found")?
    };

    // Check for newer firmware
    let new_firmware = firmware_manager
        .get_newer_version(&device.device_id, &device.firmware_version)?
        .ok_or("No newer firmware available")?;

    println!(
        "Starting OTA for {}: {} -> {}",
        device.device_id, device.firmware_version, new_firmware.version
    );

    // Update device state
    {
        let mut db = database.lock().await;
        db.update_device_state(&device.device_id, DeviceState::OtaTransmit)
            .await?;
    }

    // Read firmware binary
    let firmware_data = firmware_manager.read_firmware(&new_firmware)?;
    println!("Firmware size: {} bytes", firmware_data.len());

    // Get device connection info
    let device_ip = device.ip_address.clone();
    let ota_port = device.ota_port.unwrap_or(default_port);

    // Connect and perform OTA update
    match OtaClient::connect(&device_ip, ota_port, ota_password.clone()) {
        Ok(mut ota_client) => {
            match ota_client.update(&firmware_data) {
                Ok(()) => {
                    println!("OTA successful for {}", device.device_id);

                    // Update database
                    let mut db = database.lock().await;
                    db.update_device_firmware_version(&device.device_id, &new_firmware.version)
                        .await?;
                    db.update_device_state(&device.device_id, DeviceState::Idle)
                        .await?;
                }
                Err(e) => {
                    eprintln!("OTA failed for {}: {}", device.device_id, e);

                    // Reset state
                    let mut db = database.lock().await;
                    db.update_device_state(&device.device_id, DeviceState::Idle)
                        .await?;

                    return Err(e);
                }
            }
        }
        Err(e) => {
            eprintln!("Failed to connect to {}: {}", device.device_id, e);

            // Reset state
            let mut db = database.lock().await;
            db.update_device_state(&device.device_id, DeviceState::Idle)
                .await?;

            return Err(e);
        }
    }

    Ok(())
}

// ============================================================================
// Example 5: MQTT Message Handling
// ============================================================================
// Shows how to handle different MQTT messages (registration and OTA-READY)

async fn example_5_mqtt_message_handling() -> Result<(), String> {
    let database = Arc::new(Mutex::new(Database::new("ota.db")?));
    let mqtt_client = Arc::new(Mutex::new(MqttClient::new(
        "localhost",
        1883,
        "ota-service",
        None,
        None,
        60,
    )?));
    let firmware_manager = Arc::new(FirmwareManager::new("/var/lib/ota-service/firmware"));
    let registration_topic = "home/ota/registration".to_string();

    // Simulate receiving MQTT message
    let topic = "home/esp32-001/ota/ready";
    let payload = "OTA-READY";

    if topic == registration_topic {
        println!("Received registration message");
        // Handle registration (see Example 2)
    } else {
        // Check if it's an OTA-READY message
        if payload.trim() == "OTA-READY" {
            println!("Received OTA-READY from topic: {}", topic);

            // Find device by readiness topic
            let devices = {
                let db = database.lock().await;
                db.get_all_devices().await?
            };

            if let Some(device) = devices.into_iter().find(|d| d.ota_readiness_topic == topic) {
                println!("Matched device: {}", device.device_id);

                // Clear the OTA mode notification
                {
                    let mqtt = mqtt_client.lock().await;
                    mqtt.publish(&device.ota_mode_topic, "", QoS::AtLeastOnce, true)
                        .await?;
                }

                // Proceed with OTA update (see Example 4)
                println!("Starting OTA update for {}", device.device_id);
            } else {
                eprintln!("No device found for readiness topic: {}", topic);
            }
        }
    }

    Ok(())
}

// ============================================================================
// Example 6: Error Handling and Retry Logic
// ============================================================================
// Shows robust error handling and retry strategies

async fn example_6_error_handling_retry() -> Result<(), String> {
    let database = Arc::new(Mutex::new(Database::new("ota.db")?));
    let firmware_manager = Arc::new(FirmwareManager::new("/var/lib/ota-service/firmware"));
    let ota_password = "secure123".to_string();

    let device_id = "esp32-001";
    let device = {
        let db = database.lock().await;
        db.get_device(device_id).await?.ok_or("Device not found")?
    };

    let new_firmware = firmware_manager
        .get_newer_version(&device.device_id, &device.firmware_version)?
        .ok_or("No newer firmware")?;

    let firmware_data = firmware_manager.read_firmware(&new_firmware)?;

    // Retry logic with exponential backoff
    let max_retries = 3;
    let mut retry_count = 0;

    loop {
        println!(
            "OTA attempt {} of {} for {}",
            retry_count + 1,
            max_retries,
            device.device_id
        );

        match OtaClient::connect(
            &device.ip_address,
            device.ota_port.unwrap_or(3232),
            ota_password.clone(),
        ) {
            Ok(mut ota_client) => {
                match ota_client.update(&firmware_data) {
                    Ok(()) => {
                        println!("OTA successful for {}", device.device_id);

                        // Update database
                        let mut db = database.lock().await;
                        db.update_device_firmware_version(&device.device_id, &new_firmware.version)
                            .await?;
                        db.update_device_state(&device.device_id, DeviceState::Idle)
                            .await?;

                        return Ok(());
                    }
                    Err(e) => {
                        eprintln!("OTA update failed: {}", e);

                        retry_count += 1;
                        if retry_count >= max_retries {
                            eprintln!("Max retries reached for {}. Giving up.", device.device_id);

                            // Reset state
                            let mut db = database.lock().await;
                            db.update_device_state(&device.device_id, DeviceState::Idle)
                                .await?;

                            return Err(format!("OTA failed after {} retries", max_retries));
                        }

                        // Exponential backoff: 2^retry * 1 second
                        let backoff_secs = 2u64.pow(retry_count);
                        println!("Retrying in {} seconds...", backoff_secs);
                        tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
                    }
                }
            }
            Err(e) => {
                eprintln!("Failed to connect: {}", e);

                retry_count += 1;
                if retry_count >= max_retries {
                    // Reset state
                    let mut db = database.lock().await;
                    db.update_device_state(&device.device_id, DeviceState::Idle)
                        .await?;

                    return Err(format!("Connection failed after {} retries", max_retries));
                }

                let backoff_secs = 2u64.pow(retry_count);
                tokio::time::sleep(tokio::time::Duration::from_secs(backoff_secs)).await;
            }
        }
    }
}

// ============================================================================
// Example 7: Manual OTA Trigger
// ============================================================================
// Shows how to manually trigger an OTA update for a specific device

async fn example_7_manual_ota_trigger(device_id: &str) -> Result<(), String> {
    let database = Arc::new(Mutex::new(Database::new("ota.db")?));
    let mqtt_client = Arc::new(Mutex::new(MqttClient::new(
        "localhost",
        1883,
        "ota-service",
        None,
        None,
        60,
    )?));
    let firmware_manager = Arc::new(FirmwareManager::new("/var/lib/ota-service/firmware"));

    // Get device
    let device = {
        let db = database.lock().await;
        db.get_device(device_id)
            .await?
            .ok_or(format!("Device {} not found", device_id))?
    };

    println!("Manual OTA trigger for {}", device_id);
    println!("Current firmware: {}", device.firmware_version);

    // Check for newer firmware
    match firmware_manager.get_newer_version(&device.device_id, &device.firmware_version) {
        Ok(Some(new_firmware)) => {
            println!(
                "Newer firmware available: {} -> {}",
                device.firmware_version, new_firmware.version
            );

            // Force update state
            {
                let mut db = database.lock().await;
                db.update_device_state(
                    &device.device_id,
                    DeviceState::NewVersionAvailableTransmitted,
                )
                .await?;
            }

            // Send OTA mode notification
            {
                let mqtt = mqtt_client.lock().await;
                mqtt.publish(&device.ota_mode_topic, "ON", QoS::AtLeastOnce, true)
                    .await?;
            }

            println!("OTA notification sent to {}", device_id);
            println!("Waiting for device to send OTA-READY...");

            Ok(())
        }
        Ok(None) => {
            println!("Device {} is already up to date", device_id);
            Err("No newer firmware available".to_string())
        }
        Err(e) => {
            eprintln!("Failed to check firmware: {}", e);
            Err(e)
        }
    }
}

// ============================================================================
// Main function showing usage
// ============================================================================

#[tokio::main]
async fn main() -> Result<(), String> {
    println!("=== OTA Service Examples ===\n");

    println!("Example 1: Basic Service Setup");
    example_1_basic_service_setup().await?;
    println!();

    println!("Example 2: Device Registration");
    example_2_device_registration().await?;
    println!();

    println!("Example 3: Firmware Version Check");
    example_3_firmware_version_check().await?;
    println!();

    println!("Example 4: OTA Update Execution");
    example_4_ota_update_execution().await?;
    println!();

    println!("Example 5: MQTT Message Handling");
    example_5_mqtt_message_handling().await?;
    println!();

    println!("Example 6: Error Handling and Retry Logic");
    example_6_error_handling_retry().await?;
    println!();

    println!("Example 7: Manual OTA Trigger");
    example_7_manual_ota_trigger("esp32-001").await?;
    println!();

    println!("All examples completed successfully!");

    Ok(())
}
