use config::{Config, ConfigError, File};
use log::info;
use serde::{Deserialize, Serialize};
use std::path::Path;

/// Main configuration struct for the OTA service
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Configuration {
    /// MQTT broker configuration
    pub mqtt: MqttConfig,
    /// Database configuration
    pub database: DatabaseConfig,
    /// Service configuration
    pub service: ServiceConfig,
    /// Firmware update configuration
    pub firmware: FirmwareConfig,
    /// Pushover notification configuration (optional)
    #[serde(default)]
    pub pushover: Option<PushoverConfig>,
    /// Web interface configuration
    pub web: WebConfig,
}

/// MQTT broker configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct MqttConfig {
    /// MQTT broker host
    pub host: String,
    /// MQTT broker port
    pub port: u16,
    /// Client ID for MQTT connection
    pub client_id: String,
    /// Username for MQTT authentication (optional)
    pub username: Option<String>,
    /// Password for MQTT authentication (optional)
    pub password: Option<String>,
    /// Keep-alive interval in seconds
    pub keep_alive: u64,
    /// Topic for device registration messages
    pub registration_topic: String,
}

/// Database configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DatabaseConfig {
    /// Path to SQLite database file
    pub path: String,
    /// Connection pool size
    pub pool_size: u32,
}

/// Service configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ServiceConfig {
    /// Service name
    pub name: String,
    /// Log level
    pub log_level: String,
    /// Path to log file (optional)
    #[serde(default)]
    pub log_file_path: Option<String>,
}

/// Pushover notification configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct PushoverConfig {
    /// Pushover API token/key
    pub api_token: String,
    /// Pushover user key
    pub user_key: String,
    /// Device name to send to (optional)
    pub device: Option<String>,
    /// Notification priority (-2 to 2)
    #[serde(default = "default_priority")]
    pub priority: i8,
    /// Enable notifications
    #[serde(default = "default_enabled")]
    pub enabled: bool,
}

fn default_priority() -> i8 {
    0
}

fn default_enabled() -> bool {
    true
}

/// Web interface configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WebConfig {
    /// Web server port
    pub port: u16,
    /// Username for web interface authentication
    pub username: String,
    /// Password for web interface authentication
    pub password: String,
    /// Refresh period in seconds for updating device table
    pub refresh_period: u64,
}

/// Firmware update configuration
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareConfig {
    /// Directory where firmware files are stored
    pub storage_path: String,
    /// Maximum concurrent OTA updates
    pub max_concurrent_updates: u32,
    /// Update timeout in seconds
    pub update_timeout: u64,
    /// Interval in seconds between firmware availability checks
    pub check_interval: u64,
    /// OTA authentication password (same for all devices)
    pub ota_password: Option<String>,
    /// Default OTA port number (can be overridden per device)
    #[serde(default = "default_ota_port")]
    pub default_ota_port: u16,
    /// Erase firmware file after successful upload
    #[serde(default = "default_erase_firmware")]
    pub erase_firmware_after_upload: bool,
}

fn default_ota_port() -> u16 {
    3232
}

fn default_erase_firmware() -> bool {
    false
}

impl Configuration {
    /// Load configuration from a YAML file
    ///
    /// # Arguments
    /// * `config_path` - Path to the YAML configuration file
    ///
    /// # Returns
    /// Result containing the loaded Configuration or a ConfigError
    pub fn from_file<P: AsRef<Path>>(config_path: P) -> Result<Self, ConfigError> {
        let path = config_path.as_ref();

        info!("Loading configuration from: {:?}", path);

        let settings = Config::builder().add_source(File::from(path)).build()?;

        settings.try_deserialize()
    }

    /// Validate the configuration
    ///
    /// # Returns
    /// Result indicating if configuration is valid
    pub fn validate(&self) -> Result<(), String> {
        // Validate MQTT configuration
        if self.mqtt.host.is_empty() {
            let err = "MQTT host cannot be empty".to_string();
            log::error!("{}", err);
            return Err(err);
        }
        if self.mqtt.port == 0 {
            let err = "MQTT port must be greater than 0".to_string();
            log::error!("{}", err);
            return Err(err);
        }
        if self.mqtt.client_id.is_empty() {
            let err = "MQTT client ID cannot be empty".to_string();
            log::error!("{}", err);
            return Err(err);
        }

        // Validate database configuration
        if self.database.path.is_empty() {
            let err = "Database path cannot be empty".to_string();
            log::error!("{}", err);
            return Err(err);
        }
        if self.database.pool_size == 0 {
            let err = "Database pool size must be greater than 0".to_string();
            log::error!("{}", err);
            return Err(err);
        }

        // Validate firmware configuration
        if self.firmware.storage_path.is_empty() {
            let err = "Firmware storage path cannot be empty".to_string();
            log::error!("{}", err);
            return Err(err);
        }
        if self.firmware.max_concurrent_updates == 0 {
            let err = "Max concurrent updates must be greater than 0".to_string();
            log::error!("{}", err);
            return Err(err);
        }
        if self.firmware.update_timeout == 0 {
            let err = "Update timeout must be greater than 0".to_string();
            log::error!("{}", err);
            return Err(err);
        }
        if self.firmware.check_interval == 0 {
            let err = "Check interval must be greater than 0".to_string();
            log::error!("{}", err);
            return Err(err);
        }

        // Validate service configuration
        if self.service.name.is_empty() {
            let err = "Service name cannot be empty".to_string();
            log::error!("{}", err);
            return Err(err);
        }

        // Validate log level
        let valid_levels = ["trace", "debug", "info", "warn", "error"];
        if !valid_levels.contains(&self.service.log_level.to_lowercase().as_str()) {
            let err = format!(
                "Invalid log level '{}'. Must be one of: trace, debug, info, warn, error",
                self.service.log_level
            );
            log::error!("{}", err);
            return Err(err);
        }

        // Validate Pushover configuration if present
        if let Some(ref pushover) = self.pushover {
            if pushover.enabled {
                if pushover.api_token.is_empty() {
                    let err = "Pushover API token cannot be empty when enabled".to_string();
                    log::error!("{}", err);
                    return Err(err);
                }
                if pushover.user_key.is_empty() {
                    let err = "Pushover user key cannot be empty when enabled".to_string();
                    log::error!("{}", err);
                    return Err(err);
                }
                if !(-2..=2).contains(&pushover.priority) {
                    let err = format!(
                        "Pushover priority must be between -2 and 2, got {}",
                        pushover.priority
                    );
                    log::error!("{}", err);
                    return Err(err);
                }
            }
        }

        Ok(())
    }

    /// Get MQTT broker connection string
    pub fn mqtt_connection_string(&self) -> String {
        format!("{}:{}", self.mqtt.host, self.mqtt.port)
    }

    /// Check if Pushover notifications are enabled
    pub fn pushover_enabled(&self) -> bool {
        self.pushover.as_ref().map_or(false, |p| p.enabled)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_default_values() {
        assert_eq!(default_priority(), 0);
        assert_eq!(default_enabled(), true);
        assert_eq!(default_ota_port(), 3232);
        assert_eq!(default_erase_firmware(), false);
    }

    #[test]
    fn test_mqtt_connection_string() {
        let config = Configuration {
            mqtt: MqttConfig {
                host: "localhost".to_string(),
                port: 1883,
                client_id: "test".to_string(),
                username: None,
                password: None,
                keep_alive: 60,
                registration_topic: "ota/registration".to_string(),
            },
            database: DatabaseConfig {
                path: "test.db".to_string(),
                pool_size: 5,
            },
            service: ServiceConfig {
                name: "test".to_string(),
                log_level: "info".to_string(),
                log_file_path: None,
            },
            firmware: FirmwareConfig {
                storage_path: "/tmp".to_string(),
                max_concurrent_updates: 5,
                update_timeout: 300,
                check_interval: 60,
                ota_password: None,
                default_ota_port: 3232,
                erase_firmware_after_upload: false,
            },
            pushover: None,
            web: WebConfig {
                port: 8080,
                username: "admin".to_string(),
                password: "admin".to_string(),
                refresh_period: 5,
            },
        };

        assert_eq!(config.mqtt_connection_string(), "localhost:1883");
        assert_eq!(config.pushover_enabled(), false);
    }
}
