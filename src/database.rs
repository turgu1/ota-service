use log::{debug, error, info};
use serde::{Deserialize, Serialize};
use sqlite::{Connection, State};
use std::fmt;
use std::path::Path;

/// Device state enumeration
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DeviceState {
    /// Device is idle, not currently updating
    Idle,
    /// New firmware version available and notification sent
    NewVersionAvailableTransmitted,
    /// OTA update in progress
    OtaTransmit,
}

impl DeviceState {
    /// Convert DeviceState to string representation for database storage
    pub fn to_string(&self) -> &'static str {
        match self {
            DeviceState::Idle => "idle",
            DeviceState::NewVersionAvailableTransmitted => "new_version_available_transmitted",
            DeviceState::OtaTransmit => "ota_transmit",
        }
    }

    /// Convert string from database to DeviceState
    pub fn from_string(s: &str) -> Option<Self> {
        match s {
            "idle" => Some(DeviceState::Idle),
            "new_version_available_transmitted" => {
                Some(DeviceState::NewVersionAvailableTransmitted)
            }
            "ota_transmit" => Some(DeviceState::OtaTransmit),
            _ => None,
        }
    }
}

impl fmt::Display for DeviceState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}", self.to_string())
    }
}

/// Device record in the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Device {
    pub device_id: String,
    pub ip_address: String,
    pub firmware_version: String,
    pub last_updated: String,
    pub ota_readiness_topic: String,
    pub ota_mode_topic: String,
    pub uses_deep_sleep: bool,
    pub ota_port: Option<u16>,
    pub state: DeviceState,
}

/// SQLite database for device management
pub struct Database {
    connection: Connection,
}

impl Database {
    /// Create a new database instance
    ///
    /// # Arguments
    /// * `db_path` - Path to the SQLite database file
    ///
    /// # Returns
    /// Result containing the Database instance or error message
    pub fn new<P: AsRef<Path>>(db_path: P) -> Result<Self, String> {
        let path = db_path.as_ref();
        info!("Opening database at: {:?}", path);

        let connection =
            Connection::open(path).map_err(|e| format!("Failed to open database: {}", e))?;

        let db = Database { connection };

        // Initialize database schema
        db.initialize_schema()?;

        Ok(db)
    }

    /// Initialize database schema (create tables if they don't exist)
    fn initialize_schema(&self) -> Result<(), String> {
        debug!("Initializing database schema");

        let query = "
            CREATE TABLE IF NOT EXISTS devices (
                device_id TEXT PRIMARY KEY,
                ip_address TEXT NOT NULL,
                firmware_version TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                ota_readiness_topic TEXT NOT NULL,
                ota_mode_topic TEXT NOT NULL,
                uses_deep_sleep INTEGER NOT NULL,
                ota_port INTEGER,
                state TEXT NOT NULL DEFAULT 'idle'
            )
        ";

        self.connection
            .execute(query)
            .map_err(|e| format!("Failed to create devices table: {}", e))?;

        debug!("Database schema initialized");
        Ok(())
    }

    /// Insert or update a device
    pub fn upsert_device(&mut self, device: &Device) -> Result<(), String> {
        debug!("Upserting device: {}", device.device_id);

        let query = "
            INSERT OR REPLACE INTO devices (
                device_id, ip_address, firmware_version, last_updated,
                ota_readiness_topic, ota_mode_topic, uses_deep_sleep, ota_port, state
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?)
        ";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare upsert statement: {}", e))?;

        statement
            .bind((1, device.device_id.as_str()))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;
        statement
            .bind((2, device.ip_address.as_str()))
            .map_err(|e| format!("Failed to bind ip_address: {}", e))?;
        statement
            .bind((3, device.firmware_version.as_str()))
            .map_err(|e| format!("Failed to bind firmware_version: {}", e))?;
        statement
            .bind((4, device.last_updated.as_str()))
            .map_err(|e| format!("Failed to bind last_updated: {}", e))?;
        statement
            .bind((5, device.ota_readiness_topic.as_str()))
            .map_err(|e| format!("Failed to bind ota_readiness_topic: {}", e))?;
        statement
            .bind((6, device.ota_mode_topic.as_str()))
            .map_err(|e| format!("Failed to bind ota_mode_topic: {}", e))?;
        statement
            .bind((7, if device.uses_deep_sleep { 1i64 } else { 0i64 }))
            .map_err(|e| format!("Failed to bind uses_deep_sleep: {}", e))?;
        statement
            .bind((8, device.ota_port.map(|p| p as i64).unwrap_or(0i64).max(0)))
            .map_err(|e| format!("Failed to bind ota_port: {}", e))?;
        statement
            .bind((9, device.state.to_string()))
            .map_err(|e| format!("Failed to bind state: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute upsert: {}", e))?;

        info!("Device upserted: {}", device.device_id);
        Ok(())
    }

    /// Get a device by ID
    pub fn get_device(&self, device_id: &str) -> Result<Option<Device>, String> {
        debug!("Getting device: {}", device_id);

        let query = "SELECT * FROM devices WHERE device_id = ?";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare select statement: {}", e))?;

        statement
            .bind((1, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        if let Ok(State::Row) = statement.next() {
            let device = Device {
                device_id: statement
                    .read::<String, _>("device_id")
                    .map_err(|e| format!("Failed to read device_id: {}", e))?,
                ip_address: statement
                    .read::<String, _>("ip_address")
                    .map_err(|e| format!("Failed to read ip_address: {}", e))?,
                firmware_version: statement
                    .read::<String, _>("firmware_version")
                    .map_err(|e| format!("Failed to read firmware_version: {}", e))?,
                last_updated: statement
                    .read::<String, _>("last_updated")
                    .map_err(|e| format!("Failed to read last_updated: {}", e))?,
                ota_readiness_topic: statement
                    .read::<String, _>("ota_readiness_topic")
                    .map_err(|e| format!("Failed to read ota_readiness_topic: {}", e))?,
                ota_mode_topic: statement
                    .read::<String, _>("ota_mode_topic")
                    .map_err(|e| format!("Failed to read ota_mode_topic: {}", e))?,
                uses_deep_sleep: statement
                    .read::<i64, _>("uses_deep_sleep")
                    .map_err(|e| format!("Failed to read uses_deep_sleep: {}", e))?
                    != 0,
                ota_port: {
                    let port = statement
                        .read::<i64, _>("ota_port")
                        .map_err(|e| format!("Failed to read ota_port: {}", e))?;
                    if port > 0 {
                        Some(port as u16)
                    } else {
                        None
                    }
                },
                state: {
                    let state_str = statement
                        .read::<String, _>("state")
                        .map_err(|e| format!("Failed to read state: {}", e))?;
                    DeviceState::from_string(&state_str).unwrap_or(DeviceState::Idle)
                },
            };

            Ok(Some(device))
        } else {
            Ok(None)
        }
    }

    /// Get all devices
    pub fn get_all_devices(&self) -> Result<Vec<Device>, String> {
        debug!("Getting all devices");

        let query = "SELECT * FROM devices";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare select statement: {}", e))?;

        let mut devices = Vec::new();

        while let Ok(State::Row) = statement.next() {
            let device = Device {
                device_id: statement
                    .read::<String, _>("device_id")
                    .map_err(|e| format!("Failed to read device_id: {}", e))?,
                ip_address: statement
                    .read::<String, _>("ip_address")
                    .map_err(|e| format!("Failed to read ip_address: {}", e))?,
                firmware_version: statement
                    .read::<String, _>("firmware_version")
                    .map_err(|e| format!("Failed to read firmware_version: {}", e))?,
                last_updated: statement
                    .read::<String, _>("last_updated")
                    .map_err(|e| format!("Failed to read last_updated: {}", e))?,
                ota_readiness_topic: statement
                    .read::<String, _>("ota_readiness_topic")
                    .map_err(|e| format!("Failed to read ota_readiness_topic: {}", e))?,
                ota_mode_topic: statement
                    .read::<String, _>("ota_mode_topic")
                    .map_err(|e| format!("Failed to read ota_mode_topic: {}", e))?,
                uses_deep_sleep: statement
                    .read::<i64, _>("uses_deep_sleep")
                    .map_err(|e| format!("Failed to read uses_deep_sleep: {}", e))?
                    != 0,
                ota_port: {
                    let port = statement
                        .read::<i64, _>("ota_port")
                        .map_err(|e| format!("Failed to read ota_port: {}", e))?;
                    if port > 0 {
                        Some(port as u16)
                    } else {
                        None
                    }
                },
                state: {
                    let state_str = statement
                        .read::<String, _>("state")
                        .map_err(|e| format!("Failed to read state: {}", e))?;
                    DeviceState::from_string(&state_str).unwrap_or(DeviceState::Idle)
                },
            };

            devices.push(device);
        }

        debug!("Retrieved {} devices", devices.len());
        Ok(devices)
    }

    /// Update device firmware version
    pub fn update_device_firmware_version(
        &mut self,
        device_id: &str,
        new_version: &str,
    ) -> Result<(), String> {
        debug!(
            "Updating firmware version for device {}: {}",
            device_id, new_version
        );

        let query = "UPDATE devices SET firmware_version = ?, last_updated = ? WHERE device_id = ?";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare update statement: {}", e))?;

        let now = chrono::Local::now().to_rfc3339();

        statement
            .bind((1, new_version))
            .map_err(|e| format!("Failed to bind firmware_version: {}", e))?;
        statement
            .bind((2, now.as_str()))
            .map_err(|e| format!("Failed to bind last_updated: {}", e))?;
        statement
            .bind((3, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute update: {}", e))?;

        info!(
            "Updated firmware version for device {}: {}",
            device_id, new_version
        );
        Ok(())
    }

    /// Update device state
    pub fn update_device_state(
        &mut self,
        device_id: &str,
        new_state: DeviceState,
    ) -> Result<(), String> {
        debug!("Updating state for device {}: {}", device_id, new_state);

        let query = "UPDATE devices SET state = ?, last_updated = ? WHERE device_id = ?";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare update statement: {}", e))?;

        let now = chrono::Local::now().to_rfc3339();

        statement
            .bind((1, new_state.to_string()))
            .map_err(|e| format!("Failed to bind state: {}", e))?;
        statement
            .bind((2, now.as_str()))
            .map_err(|e| format!("Failed to bind last_updated: {}", e))?;
        statement
            .bind((3, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute update: {}", e))?;

        info!("Updated state for device {}: {}", device_id, new_state);
        Ok(())
    }

    /// Delete a device
    pub fn delete_device(&mut self, device_id: &str) -> Result<(), String> {
        debug!("Deleting device: {}", device_id);

        let query = "DELETE FROM devices WHERE device_id = ?";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare delete statement: {}", e))?;

        statement
            .bind((1, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute delete: {}", e))?;

        info!("Device deleted: {}", device_id);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_device_state_conversion() {
        assert_eq!(DeviceState::Idle.to_string(), "idle");
        assert_eq!(
            DeviceState::NewVersionAvailableTransmitted.to_string(),
            "new_version_available_transmitted"
        );
        assert_eq!(DeviceState::OtaTransmit.to_string(), "ota_transmit");

        assert_eq!(DeviceState::from_string("idle"), Some(DeviceState::Idle));
        assert_eq!(
            DeviceState::from_string("new_version_available_transmitted"),
            Some(DeviceState::NewVersionAvailableTransmitted)
        );
        assert_eq!(
            DeviceState::from_string("ota_transmit"),
            Some(DeviceState::OtaTransmit)
        );
        assert_eq!(DeviceState::from_string("invalid"), None);
    }

    #[test]
    fn test_database_creation() {
        let db_path = "/tmp/test_ota_db.db";
        let _ = std::fs::remove_file(db_path);

        let db = Database::new(db_path);
        assert!(db.is_ok());

        let _ = std::fs::remove_file(db_path);
    }
}
