use config::{Config, ConfigError, File};
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main configuration struct for the simulator
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    pub mqtt: MqttConfig,
    pub simulator: SimulatorConfig,
    pub firmware: FirmwareConfig,
    pub deep_sleep: DeepSleepConfig,
}

/// MQTT broker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    pub host: String,
    pub port: u16,
    pub username: Option<String>,
    pub password: Option<String>,
    pub keep_alive: u64,
    pub topic_prefix: String,
    pub registration_topic: String,
}

/// Simulator configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SimulatorConfig {
    pub num_devices: u32,
    pub device_id_prefix: String,
    pub base_ota_port: u16,
    pub log_level: String,
}

/// Firmware configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareConfig {
    pub ota_password: Option<String>,
    pub storage_path: String,
    pub generation_interval_min: u64,
    pub generation_interval_max: u64,
    pub initial_version: String,
}

/// Deep sleep configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeepSleepConfig {
    pub min_sleep_seconds: u64,
    pub max_sleep_seconds: u64,
    pub max_wakeup_seconds: u64,
}

impl Configuration {
    /// Load configuration from a YAML file
    pub fn from_file<P: AsRef<Path>>(path: P) -> Result<Self, ConfigError> {
        let config = Config::builder()
            .add_source(File::from(path.as_ref()))
            .build()?;

        config.try_deserialize()
    }

    /// Validate configuration
    pub fn validate(&self) -> Result<(), String> {
        // Validate number of devices
        if self.simulator.num_devices == 0 {
            return Err("num_devices must be greater than 0".to_string());
        }
        if self.simulator.num_devices > 100 {
            return Err("num_devices must not exceed 100".to_string());
        }

        // Validate sleep parameters
        if self.deep_sleep.min_sleep_seconds >= self.deep_sleep.max_sleep_seconds {
            return Err("min_sleep_seconds must be less than max_sleep_seconds".to_string());
        }

        if self.deep_sleep.max_wakeup_seconds == 0 {
            return Err("max_wakeup_seconds must be greater than 0".to_string());
        }

        // Validate firmware generation intervals
        if self.firmware.generation_interval_min >= self.firmware.generation_interval_max {
            return Err(
                "generation_interval_min must be less than generation_interval_max".to_string(),
            );
        }

        // Validate base OTA port
        if self.simulator.base_ota_port == 0 {
            return Err("base_ota_port must be greater than 0".to_string());
        }

        // Check if base_ota_port + num_devices exceeds u16 max
        if (self.simulator.base_ota_port as u32 + self.simulator.num_devices) > 65535 {
            return Err("base_ota_port + num_devices exceeds maximum port number".to_string());
        }

        Ok(())
    }
}
