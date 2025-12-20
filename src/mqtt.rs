use crate::database::{Device, DeviceState};
use log::{debug, error, info};
use serde::{Deserialize, Serialize};

/// Device registration message received from MQTT
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceRegistration {
    pub device_id: String,
    pub ip_address: String,
    pub mac_address: String,
    pub firmware_version: String,
    pub ota_readiness_topic: String,
    pub ota_mode_topic: String,
    #[serde(rename = "uses_deep_sleep")]
    pub uses_deep_sleep: bool,
    /// Optional device-specific OTA port
    #[serde(default)]
    pub ota_port: Option<u16>,
    /// WiFi signal strength (RSSI)
    #[serde(default)]
    pub rssi: i32,
}

impl DeviceRegistration {
    /// Parse a device registration from JSON
    pub fn from_json(json_str: &str) -> Result<Self, String> {
        serde_json::from_str(json_str).map_err(|e| {
            let err_msg = format!("Failed to parse device registration JSON: {}", e);
            error!("{}", err_msg);
            err_msg
        })
    }

    /// Convert to a Device record (state is initialized to Idle)
    pub fn to_device(&self) -> Device {
        Device {
            device_id: self.device_id.clone(),
            ip_address: self.ip_address.clone(),
            mac_address: self.mac_address.clone(),
            firmware_version: self.firmware_version.clone(),
            last_updated: chrono::Local::now().to_rfc3339(),
            ota_readiness_topic: self.ota_readiness_topic.clone(),
            ota_mode_topic: self.ota_mode_topic.clone(),
            uses_deep_sleep: self.uses_deep_sleep,
            ota_port: self.ota_port,
            state: DeviceState::Idle,
            fail_count: 0,
            update_count: 0,
            rssi: self.rssi,
            project_folder: None,
            main_filename: None,
        }
    }
}

/// MQTT coordinator for OTA service
pub struct MqttCoordinator {
    /// Registration topic where devices publish their info
    pub registration_topic: String,
    /// Prefix for device readiness topics
    pub readiness_topic_prefix: String,
}

impl MqttCoordinator {
    /// Create a new MQTT coordinator
    pub fn new(registration_topic: String, readiness_topic_prefix: String) -> Self {
        info!("Initializing MQTT coordinator");
        info!("Registration topic: {}", registration_topic);
        info!("Readiness topic prefix: {}", readiness_topic_prefix);

        MqttCoordinator {
            registration_topic,
            readiness_topic_prefix,
        }
    }

    /// Get the registration topic
    pub fn _get_registration_topic(&self) -> &str {
        &self.registration_topic
    }

    /// Get the readiness topic prefix
    pub fn _get_readiness_topic_prefix(&self) -> &str {
        &self.readiness_topic_prefix
    }

    /// Check if a topic is a registration topic
    #[cfg(test)]
    pub fn is_registration_topic(&self, topic: &str) -> bool {
        topic == self.registration_topic
    }

    /// Check if a topic is a readiness topic
    #[cfg(test)]
    pub fn is_readiness_topic(&self, topic: &str) -> bool {
        topic.starts_with(&self.readiness_topic_prefix)
    }

    /// Extract device ID from a readiness topic
    #[cfg(test)]
    pub fn extract_device_id_from_readiness_topic(&self, topic: &str) -> Option<String> {
        if let Some(device_part) = topic.strip_prefix(&self.readiness_topic_prefix) {
            // Readiness topic format might be: "devices/{device_id}/ready"
            // Extract device_id
            let parts: Vec<&str> = device_part.split('/').collect();
            if !parts.is_empty() && !parts[0].is_empty() {
                return Some(parts[0].to_string());
            }
        }
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_registration_from_json() {
        let json = r#"{
            "device_id": "esp32-001",
            "ip_address": "192.168.1.100",
            "mac_address": "AA:BB:CC:DD:EE:FF",
            "firmware_version": "1.0.0",
            "ota_readiness_topic": "devices/esp32-001/ready",
            "ota_mode_topic": "devices/esp32-001/ota-mode",
            "uses_deep_sleep": true,
            "ota_port": 3232,
            "rssi": -65
        }"#;

        let reg = DeviceRegistration::from_json(json);
        assert!(reg.is_ok());

        let reg = reg.unwrap();
        assert_eq!(reg.device_id, "esp32-001");
        assert_eq!(reg.ip_address, "192.168.1.100");
        assert_eq!(reg.mac_address, "AA:BB:CC:DD:EE:FF");
        assert_eq!(reg.firmware_version, "1.0.0");
        assert_eq!(reg.ota_port, Some(3232));
        assert_eq!(reg.rssi, -65);
    }

    #[test]
    fn test_mqtt_coordinator() {
        let coordinator =
            MqttCoordinator::new("devices/register".to_string(), "devices/".to_string());

        assert!(coordinator.is_registration_topic("devices/register"));
        assert!(!coordinator.is_registration_topic("devices/other"));

        assert!(coordinator.is_readiness_topic("devices/esp32-001/ready"));
        assert!(!coordinator.is_readiness_topic("other/topic"));

        let device_id =
            coordinator.extract_device_id_from_readiness_topic("devices/esp32-001/ready");
        assert_eq!(device_id, Some("esp32-001".to_string()));
    }
}
