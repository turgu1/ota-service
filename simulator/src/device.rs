use crate::config::MqttConfig;
use crate::mqtt_client::DeviceMqttClient;
use log::{debug, error, info};
use rand::Rng;
use rumqttc::{Event, Packet, QoS};
use serde::{Deserialize, Serialize};
use std::net::TcpListener;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;
use tokio::time::{sleep, timeout, Duration};

/// Simulated ESP32 device
pub struct SimulatedDevice {
    device_id: String,
    mac_address: String,
    ota_port: u16,
    mqtt_config: MqttConfig,
    ota_password: Option<String>,
    firmware_version: String,
    min_sleep: u64,
    max_sleep: u64,
    max_wakeup: u64,
    is_first_boot: bool, // Track if this is a boot (true) or wakeup from sleep (false)
}

/// Device registration message
#[derive(Debug, Serialize, Deserialize)]
struct DeviceRegistration {
    device_id: String,
    mac_address: String,
    ip_address: String,
    firmware_version: String,
    ota_readiness_topic: String,
    ota_mode_topic: String,
    uses_deep_sleep: bool,
    ota_port: u16,
    rssi: i32,
}

impl SimulatedDevice {
    /// Create a new simulated device
    pub fn new(
        device_id: String,
        ota_port: u16,
        mqtt_config: MqttConfig,
        ota_password: Option<String>,
        initial_version: String,
        min_sleep: u64,
        max_sleep: u64,
        max_wakeup: u64,
    ) -> Self {
        // Generate a unique MAC address based on the device_id
        let mac_address = Self::generate_mac_address(&device_id);

        SimulatedDevice {
            device_id,
            mac_address,
            ota_port,
            mqtt_config,
            ota_password,
            firmware_version: initial_version,
            min_sleep,
            max_sleep,
            max_wakeup,
            is_first_boot: true, // First run is always a boot
        }
    }

    /// Generate a fake MAC address in format XX:XX:XX:XX:XX:XX
    /// Uses device_id to ensure uniqueness and reproducibility
    fn generate_mac_address(device_id: &str) -> String {
        use std::collections::hash_map::DefaultHasher;
        use std::hash::{Hash, Hasher};

        let mut hasher = DefaultHasher::new();
        device_id.hash(&mut hasher);
        let hash = hasher.finish();

        // Generate 6 bytes for MAC address using the hash
        let bytes = [
            0xAA, // First byte with locally administered bit set
            ((hash >> 40) & 0xFF) as u8,
            ((hash >> 32) & 0xFF) as u8,
            ((hash >> 24) & 0xFF) as u8,
            ((hash >> 16) & 0xFF) as u8,
            ((hash >> 8) & 0xFF) as u8,
        ];

        format!(
            "{:02X}:{:02X}:{:02X}:{:02X}:{:02X}:{:02X}",
            bytes[0], bytes[1], bytes[2], bytes[3], bytes[4], bytes[5]
        )
    }

    /// Run the device simulation
    pub async fn run(mut self) -> Result<(), String> {
        info!("[{}] Starting device simulation", self.device_id);

        loop {
            // Wake up phase
            if let Err(e) = self.wakeup_cycle().await {
                error!("[{}] Wakeup cycle error: {}", self.device_id, e);
            }

            // Deep sleep phase
            let sleep_duration = self.random_sleep_duration();
            info!(
                "[{}] Entering deep sleep for {} seconds",
                self.device_id,
                sleep_duration.as_secs()
            );
            sleep(sleep_duration).await;
        }
    }

    /// Simulate a wakeup cycle
    async fn wakeup_cycle(&mut self) -> Result<(), String> {
        loop {
            info!("[{}] Device waking up", self.device_id);

            // Connect to MQTT
            let mut mqtt_client = DeviceMqttClient::new(&self.device_id, &self.mqtt_config)?;

            // Wait a bit for connection to establish
            sleep(Duration::from_millis(500)).await;

            // Register device only on boot (initial start or after firmware update)
            if self.is_first_boot {
                // Generate a fake RSSI value (signal strength in dB, typically -30 to -90)
                let rssi = {
                    let mut rng = rand::rng();
                    rng.random_range(-90..=-30)
                };

                let registration = DeviceRegistration {
                    device_id: self.device_id.clone(),
                    mac_address: self.mac_address.clone(),
                    ip_address: "127.0.0.1".to_string(),
                    firmware_version: self.firmware_version.clone(),
                    ota_readiness_topic: format!(
                        "{}{}/ota-ready",
                        self.mqtt_config.topic_prefix, self.device_id
                    ),
                    ota_mode_topic: format!(
                        "{}{}/ota-mode",
                        self.mqtt_config.topic_prefix, self.device_id
                    ),
                    uses_deep_sleep: true,
                    ota_port: self.ota_port,
                    rssi,
                };

                let payload = serde_json::to_string(&registration)
                    .map_err(|e| format!("Failed to serialize registration: {}", e))?;

                let topic = self.mqtt_config.registration_topic.clone();

                mqtt_client
                    .publish(&topic, &payload, QoS::AtLeastOnce, false)
                    .await?;

                info!(
                    "[{}] Registered with version {} (boot)",
                    self.device_id, self.firmware_version
                );

                // Clear boot flag - subsequent wakeups are from deep sleep
                self.is_first_boot = false;
            } else {
                info!(
                    "[{}] Waking from deep sleep (no registration)",
                    self.device_id
                );
            }

            // Subscribe to OTA mode topic
            let ota_mode_topic = format!(
                "{}{}/ota-mode",
                self.mqtt_config.topic_prefix, self.device_id
            );
            mqtt_client
                .subscribe(&ota_mode_topic, QoS::AtLeastOnce)
                .await?;

            info!(
                "[{}] Subscribed to OTA mode topic: {}",
                self.device_id, ota_mode_topic
            );

            // Calculate wakeup timeout
            let wakeup_timeout = self.random_wakeup_duration();
            let wakeup_start = tokio::time::Instant::now();

            info!(
                "[{}] Will stay awake for {} seconds",
                self.device_id,
                wakeup_timeout.as_secs()
            );

            let mut should_reboot = false;

            // Poll for OTA update notification during wakeup time
            while wakeup_start.elapsed() < wakeup_timeout {
                if let Some(event) = mqtt_client.poll().await {
                    if let Event::Incoming(Packet::Publish(publish)) = event {
                        if publish.topic == ota_mode_topic {
                            if let Ok(payload) = String::from_utf8(publish.payload.to_vec()) {
                                if payload.trim() == "NEW-FIRMWARE-VERSION" {
                                    info!(
                                        "[{}] Received NEW-FIRMWARE-VERSION notification",
                                        self.device_id
                                    );

                                    // Clear the retained message
                                    mqtt_client.clear_retained(&ota_mode_topic).await?;

                                    // Enter OTA mode
                                    match self.handle_ota_update(&mut mqtt_client).await {
                                        Ok(new_version) => {
                                            // OTA successful, simulate reboot
                                            info!(
                                                "[{}] OTA update successful, rebooting with new firmware version {}",
                                                self.device_id, new_version
                                            );

                                            // Disconnect from MQTT (simulating device power cycle)
                                            drop(mqtt_client);

                                            // Update firmware version
                                            self.firmware_version = new_version;
                                            self.is_first_boot = true;

                                            // Simulate boot delay
                                            sleep(Duration::from_secs(2)).await;

                                            // Signal that we need to reboot (restart wakeup cycle)
                                            should_reboot = true;
                                            break;
                                        }
                                        Err(e) => {
                                            error!("[{}] OTA update failed: {}", self.device_id, e);
                                        }
                                    }
                                }
                            }
                        }
                    }
                }
                sleep(Duration::from_millis(100)).await;
            }

            // If rebooting, continue loop to start fresh wakeup cycle
            if should_reboot {
                continue;
            }

            // Normal end of wakeup cycle - return to allow deep sleep
            info!("[{}] Wakeup period ended, going to sleep", self.device_id);
            return Ok(());
        }
    }

    /// Handle OTA update process
    async fn handle_ota_update(
        &mut self,
        mqtt_client: &mut DeviceMqttClient,
    ) -> Result<String, String> {
        info!("[{}] Preparing for OTA update", self.device_id);

        // Start OTA server first
        info!(
            "[{}] Starting OTA server on port {}",
            self.device_id, self.ota_port
        );

        let listener = TcpListener::bind(format!("127.0.0.1:{}", self.ota_port))
            .map_err(|e| format!("Failed to bind OTA port: {}", e))?;

        listener
            .set_nonblocking(true)
            .map_err(|e| format!("Failed to set nonblocking: {}", e))?;

        let listener = tokio::net::TcpListener::from_std(listener)
            .map_err(|e| format!("Failed to convert listener: {}", e))?;

        info!("[{}] OTA server listening", self.device_id);

        // Now publish OTA-READY signal (retained so the OTA service receives it even if subscribed slightly late)
        let readiness_topic = format!(
            "{}{}/ota-ready",
            self.mqtt_config.topic_prefix, self.device_id
        );
        mqtt_client
            .publish(&readiness_topic, "OTA-READY", QoS::ExactlyOnce, true)
            .await?;

        info!(
            "[{}] Published OTA-READY to {}",
            self.device_id, readiness_topic
        );

        // Wait for OTA upload
        let new_version = self.wait_for_ota_upload(listener).await?;

        // Clear OTA-READY message
        mqtt_client.clear_retained(&readiness_topic).await?;

        info!(
            "[{}] Firmware received, version {}",
            self.device_id, new_version
        );

        Ok(new_version)
    }

    /// Wait for OTA upload on the listening socket
    async fn wait_for_ota_upload(
        &self,
        listener: tokio::net::TcpListener,
    ) -> Result<String, String> {
        // Wait for connection with timeout (30 seconds)
        let accept_result = timeout(Duration::from_secs(30), listener.accept()).await;

        match accept_result {
            Ok(Ok((stream, addr))) => {
                info!("[{}] OTA connection from {}", self.device_id, addr);
                self.handle_ota_connection(stream).await
            }
            Ok(Err(e)) => Err(format!("Failed to accept connection: {}", e)),
            Err(_) => Err("OTA connection timeout".to_string()),
        }
    }

    /// Handle OTA connection and protocol
    async fn handle_ota_connection(&self, mut stream: TcpStream) -> Result<String, String> {
        debug!("[{}] Handling OTA connection", self.device_id);

        // Store firmware data to extract version
        let mut firmware_data: Vec<u8> = Vec::new();

        // ESPHome OTA Protocol v2
        // 1. Receive magic bytes
        let mut magic = vec![0u8; 5];
        stream
            .read_exact(&mut magic)
            .await
            .map_err(|e| format!("Failed to read magic: {}", e))?;

        debug!("[{}] Received magic bytes", self.device_id);

        // 2. Send version (2 bytes)
        stream
            .write_all(&[0x00, 0x02])
            .await
            .map_err(|e| format!("Failed to send version: {}", e))?;

        // 3. Receive features (1 byte)
        let mut features = vec![0u8; 1];
        stream
            .read_exact(&mut features)
            .await
            .map_err(|e| format!("Failed to read features: {}", e))?;

        let client_features = features[0];
        let supports_compression = (client_features & 0x01) != 0;
        let supports_sha256_auth = (client_features & 0x02) != 0;

        debug!(
            "[{}] Received features: 0x{:02X} (compression: {}, sha256: {})",
            self.device_id, client_features, supports_compression, supports_sha256_auth
        );

        // 4. Send feature response (1 byte) - RESPONSE_HEADER_OK
        stream
            .write_all(&[0x40])
            .await
            .map_err(|e| format!("Failed to send feature response: {}", e))?;

        // 5. Handle authentication if password provided
        if let Some(password) = &self.ota_password {
            // Choose authentication method based on client support
            let use_sha256 = supports_sha256_auth;

            if use_sha256 {
                // SHA256 authentication
                stream
                    .write_all(&[0x02]) // RESPONSE_REQUEST_SHA256_AUTH
                    .await
                    .map_err(|e| format!("Failed to send SHA256 auth request: {}", e))?;

                debug!("[{}] Sent auth request (SHA256)", self.device_id);

                // Generate and send device seed (64 hex characters = 32 bytes in hex format)
                use sha2::{Digest, Sha256};
                let device_seed: [u8; 32] = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let nanos = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos();
                    let mut hasher = Sha256::new();
                    hasher.update(&nanos.to_le_bytes());
                    hasher.update(self.device_id.as_bytes());
                    hasher.finalize().into()
                };

                let device_seed_hex = device_seed
                    .iter()
                    .map(|b| format!("{:02x}", b))
                    .collect::<String>();

                stream
                    .write_all(device_seed_hex.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to send device seed: {}", e))?;

                debug!(
                    "[{}] Sent device seed (SHA256): {}",
                    self.device_id, device_seed_hex
                );

                // Receive auth response (128 bytes: 64 hex app_seed + 64 hex digest)
                let mut auth_resp = vec![0u8; 128];
                stream
                    .read_exact(&mut auth_resp)
                    .await
                    .map_err(|e| format!("Failed to read auth response: {}", e))?;

                let auth_str = String::from_utf8(auth_resp)
                    .map_err(|e| format!("Invalid auth response UTF-8: {}", e))?;

                if auth_str.len() != 128 {
                    return Err(format!(
                        "Invalid SHA256 auth response length: {}",
                        auth_str.len()
                    ));
                }

                let app_seed_hex = &auth_str[0..64];
                let received_digest_hex = &auth_str[64..128];

                debug!("[{}] Received app_seed: {}", self.device_id, app_seed_hex);
                debug!(
                    "[{}] Received digest: {}",
                    self.device_id, received_digest_hex
                );

                // Calculate expected digest: SHA256(password + device_seed_hex + app_seed_hex)
                let expected_digest = {
                    let combined = format!("{}{}{}", password, device_seed_hex, app_seed_hex);
                    let mut hasher = Sha256::new();
                    hasher.update(combined.as_bytes());
                    hasher.finalize()
                };
                let expected_digest_hex = format!("{:x}", expected_digest);

                debug!(
                    "[{}] Expected digest: {}",
                    self.device_id, expected_digest_hex
                );

                // Verify digest
                if received_digest_hex != expected_digest_hex {
                    stream
                        .write_all(&[0x82]) // RESPONSE_ERROR_AUTH_INVALID
                        .await
                        .map_err(|e| format!("Failed to send auth failed: {}", e))?;

                    return Err("SHA256 authentication failed: digest mismatch".to_string());
                }

                // Send auth OK
                stream
                    .write_all(&[0x41])
                    .await
                    .map_err(|e| format!("Failed to send auth OK: {}", e))?;

                debug!("[{}] SHA256 authentication successful", self.device_id);
            } else {
                // MD5 authentication (legacy)
                stream
                    .write_all(&[0x01]) // RESPONSE_REQUEST_AUTH
                    .await
                    .map_err(|e| format!("Failed to send MD5 auth request: {}", e))?;

                debug!("[{}] Sent auth request (MD5)", self.device_id);

                // Generate and send device seed (32 hex characters = 16 bytes in hex format)
                let device_seed: [u8; 16] = {
                    use std::time::{SystemTime, UNIX_EPOCH};
                    let nanos = SystemTime::now()
                        .duration_since(UNIX_EPOCH)
                        .unwrap()
                        .as_nanos();
                    let data = format!("{}{}", nanos, self.device_id);
                    let digest = md5::compute(data.as_bytes());
                    digest.0
                };

                let device_seed_hex = format!("{:032x}", md5::Digest(device_seed));
                stream
                    .write_all(device_seed_hex.as_bytes())
                    .await
                    .map_err(|e| format!("Failed to send device seed: {}", e))?;

                debug!(
                    "[{}] Sent device seed (MD5): {}",
                    self.device_id, device_seed_hex
                );

                // Receive auth response (64 bytes: 32 hex app_seed + 32 hex digest)
                let mut auth_resp = vec![0u8; 64];
                stream
                    .read_exact(&mut auth_resp)
                    .await
                    .map_err(|e| format!("Failed to read auth response: {}", e))?;

                let auth_str = String::from_utf8(auth_resp)
                    .map_err(|e| format!("Invalid auth response UTF-8: {}", e))?;

                if auth_str.len() != 64 {
                    return Err(format!(
                        "Invalid MD5 auth response length: {}",
                        auth_str.len()
                    ));
                }

                let app_seed_hex = &auth_str[0..32];
                let received_digest_hex = &auth_str[32..64];

                debug!("[{}] Received app_seed: {}", self.device_id, app_seed_hex);
                debug!(
                    "[{}] Received digest: {}",
                    self.device_id, received_digest_hex
                );

                // Calculate expected digest: MD5(password + device_seed_hex + app_seed_hex)
                let expected_digest = {
                    let combined = format!("{}{}{}", password, device_seed_hex, app_seed_hex);
                    md5::compute(combined.as_bytes())
                };
                let expected_digest_hex = format!("{:x}", expected_digest);

                debug!(
                    "[{}] Expected digest: {}",
                    self.device_id, expected_digest_hex
                );

                // Verify digest
                if received_digest_hex != expected_digest_hex {
                    stream
                        .write_all(&[0x82]) // RESPONSE_ERROR_AUTH_INVALID
                        .await
                        .map_err(|e| format!("Failed to send auth failed: {}", e))?;

                    return Err("MD5 authentication failed: digest mismatch".to_string());
                }

                // Send auth OK
                stream
                    .write_all(&[0x41])
                    .await
                    .map_err(|e| format!("Failed to send auth OK: {}", e))?;

                debug!("[{}] MD5 authentication successful", self.device_id);
            }
        } else {
            // No authentication required - send RESPONSE_AUTH_OK
            stream
                .write_all(&[0x41])
                .await
                .map_err(|e| format!("Failed to send no-auth response: {}", e))?;

            debug!("[{}] No authentication required", self.device_id);
        }

        // 6. Receive firmware size (4 bytes, big-endian)
        let mut size_bytes = vec![0u8; 4];
        stream
            .read_exact(&mut size_bytes)
            .await
            .map_err(|e| format!("Failed to read firmware size: {}", e))?;

        let firmware_size =
            u32::from_be_bytes([size_bytes[0], size_bytes[1], size_bytes[2], size_bytes[3]]);

        info!(
            "[{}] Receiving firmware: {} bytes",
            self.device_id, firmware_size
        );

        // Send prepare OK
        stream
            .write_all(&[0x42])
            .await
            .map_err(|e| format!("Failed to send prepare OK: {}", e))?;

        // 7. Receive MD5 checksum (32 bytes)
        let mut md5 = vec![0u8; 32];
        stream
            .read_exact(&mut md5)
            .await
            .map_err(|e| format!("Failed to read MD5: {}", e))?;

        // Send MD5 OK
        stream
            .write_all(&[0x43])
            .await
            .map_err(|e| format!("Failed to send MD5 OK: {}", e))?;

        // 8. Receive firmware chunks
        let mut received = 0u32;
        let mut chunk_buffer = vec![0u8; 1024];
        let mut ack_counter = 0;

        while received < firmware_size {
            let to_read = std::cmp::min(1024, (firmware_size - received) as usize);
            let n = stream
                .read(&mut chunk_buffer[..to_read])
                .await
                .map_err(|e| format!("Failed to read chunk: {}", e))?;

            if n == 0 {
                return Err("Connection closed prematurely".to_string());
            }

            // Store firmware data for version extraction
            firmware_data.extend_from_slice(&chunk_buffer[..n]);

            received += n as u32;
            ack_counter += n;

            // Send ACK every 8192 bytes
            if (ack_counter >= 8192) || (received == firmware_size) {
                stream
                    .write_all(&[0x47])
                    .await
                    .map_err(|e| format!("Failed to send ACK: {}", e))?;
                ack_counter = 0;
            }
        }

        info!(
            "[{}] Received complete firmware ({} bytes)",
            self.device_id, received
        );

        // Send receive OK
        stream
            .write_all(&[0x44])
            .await
            .map_err(|e| format!("Failed to send receive OK: {}", e))?;

        // Send update end OK
        stream
            .write_all(&[0x45])
            .await
            .map_err(|e| format!("Failed to send update end OK: {}", e))?;

        // Extract version from firmware data
        let new_version = self.extract_version_from_firmware(&firmware_data)?;

        info!(
            "[{}] Extracted version from firmware: {}",
            self.device_id, new_version
        );

        Ok(new_version)
    }

    /// Extract version from firmware data
    /// Firmware format: "[VERSION-X.Y.Z]" followed by firmware data
    fn extract_version_from_firmware(&self, firmware_data: &[u8]) -> Result<String, String> {
        // Look for "[VERSION-" prefix
        let version_prefix = b"[VERSION-";

        if firmware_data.len() < version_prefix.len() + 6 {
            return Err("Firmware data too small to contain version".to_string());
        }

        // Check if firmware starts with [VERSION-
        if !firmware_data.starts_with(version_prefix) {
            return Err("Firmware does not contain [VERSION- header".to_string());
        }

        // Find the closing bracket ]
        let version_start = version_prefix.len();
        let mut version_end = None;

        for i in version_start..firmware_data.len() {
            let ch = firmware_data[i] as char;
            if ch == ']' {
                version_end = Some(i);
                break;
            }
        }

        let version_end =
            version_end.ok_or("No closing bracket found for version header".to_string())?;

        if version_end == version_start {
            return Err("No version number found after [VERSION- prefix".to_string());
        }

        // Extract and validate version string
        let version_bytes = &firmware_data[version_start..version_end];
        let version_str = String::from_utf8(version_bytes.to_vec())
            .map_err(|e| format!("Invalid UTF-8 in version string: {}", e))?;

        // Validate version format (X.Y.Z)
        let parts: Vec<&str> = version_str.split('.').collect();
        if parts.len() != 3 {
            return Err(format!("Invalid version format: {}", version_str));
        }

        for part in &parts {
            if part.parse::<u32>().is_err() {
                return Err(format!("Invalid version component: {}", part));
            }
        }

        Ok(version_str)
    }

    /// Get random sleep duration
    fn random_sleep_duration(&self) -> Duration {
        let seconds = rand::rng().random_range(self.min_sleep..=self.max_sleep);
        Duration::from_secs(seconds)
    }

    /// Get random wakeup duration
    fn random_wakeup_duration(&self) -> Duration {
        let seconds = rand::rng().random_range(5..=self.max_wakeup);
        Duration::from_secs(seconds)
    }
}
