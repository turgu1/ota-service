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
    pub mac_address: String,
    pub firmware_version: String,
    pub last_updated: String,
    pub ota_readiness_topic: String,
    pub ota_mode_topic: String,
    pub uses_deep_sleep: bool,
    pub ota_port: Option<u16>,
    pub state: DeviceState,
    pub fail_count: i32,
    pub update_count: i32,
    pub rssi: i32,
    pub project_folder: Option<String>,
    pub main_filename: Option<String>,
}

/// Upload history record from the database
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct UploadHistoryEntry {
    pub device_id: String,
    pub version: String,
    pub state: String,
    pub attempted_at: String,
    pub fail_reason: Option<String>,
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

        let devices_query = "
            CREATE TABLE IF NOT EXISTS devices (
                device_id TEXT PRIMARY KEY,
                ip_address TEXT NOT NULL,
                mac_address TEXT NOT NULL,
                firmware_version TEXT NOT NULL,
                last_updated TEXT NOT NULL,
                ota_readiness_topic TEXT NOT NULL,
                ota_mode_topic TEXT NOT NULL,
                uses_deep_sleep INTEGER NOT NULL,
                ota_port INTEGER,
                state TEXT NOT NULL DEFAULT 'idle',
                fail_count INTEGER NOT NULL DEFAULT 0,
                update_count INTEGER NOT NULL DEFAULT 0,
                rssi INTEGER NOT NULL DEFAULT 0,
                project_folder TEXT,
                main_filename TEXT
            )
        ";

        self.connection
            .execute(devices_query)
            .map_err(|e| format!("Failed to create devices table: {}", e))?;

        let upload_history_query = "
            CREATE TABLE IF NOT EXISTS upload_history (
                id INTEGER PRIMARY KEY AUTOINCREMENT,
                device_id TEXT NOT NULL,
                version TEXT NOT NULL,
                state TEXT NOT NULL CHECK(state IN ('SUCCESS', 'FAIL')),
                attempted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP,
                fail_reason TEXT
            )
        ";

        self.connection
            .execute(upload_history_query)
            .map_err(|e| format!("Failed to create upload_history table: {}", e))?;

        // Run migrations for existing databases
        self.run_migrations()?;

        debug!("Database schema initialized");
        Ok(())
    }

    /// Run database migrations to update existing schemas
    fn run_migrations(&self) -> Result<(), String> {
        debug!("Running database migrations");

        // Check if project_folder column exists, if not add it
        let check_column = "SELECT COUNT(*) as count FROM pragma_table_info('devices') WHERE name='project_folder'";

        let mut statement = self
            .connection
            .prepare(check_column)
            .map_err(|e| format!("Failed to check for project_folder column: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute column check: {}", e))?;

        let count: i64 = statement
            .read(0)
            .map_err(|e| format!("Failed to read column count: {}", e))?;

        if count == 0 {
            info!("Adding project_folder column to devices table");
            let add_column = "ALTER TABLE devices ADD COLUMN project_folder TEXT";
            self.connection
                .execute(add_column)
                .map_err(|e| format!("Failed to add project_folder column: {}", e))?;
            info!("project_folder column added successfully");
        }

        // Check if main_filename column exists, if not add it
        let check_main_filename =
            "SELECT COUNT(*) as count FROM pragma_table_info('devices') WHERE name='main_filename'";

        let mut statement = self
            .connection
            .prepare(check_main_filename)
            .map_err(|e| format!("Failed to check for main_filename column: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute column check: {}", e))?;

        let count: i64 = statement
            .read(0)
            .map_err(|e| format!("Failed to read column count: {}", e))?;

        if count == 0 {
            info!("Adding main_filename column to devices table");
            let add_column = "ALTER TABLE devices ADD COLUMN main_filename TEXT";
            self.connection
                .execute(add_column)
                .map_err(|e| format!("Failed to add main_filename column: {}", e))?;
            info!("main_filename column added successfully");
        }

        debug!("Database migrations completed");
        Ok(())
    }

    /// Insert or update a device
    pub fn upsert_device(&mut self, device: &Device) -> Result<(), String> {
        debug!("Upserting device: {}", device.device_id);

        let query = "
            INSERT INTO devices (
                device_id, ip_address, mac_address, firmware_version, last_updated,
                ota_readiness_topic, ota_mode_topic, uses_deep_sleep, ota_port, state,
                fail_count, update_count, rssi, project_folder, main_filename
            ) VALUES (?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?, ?)
            ON CONFLICT(device_id) DO UPDATE SET
                ip_address = excluded.ip_address,
                mac_address = excluded.mac_address,
                firmware_version = excluded.firmware_version,
                ota_readiness_topic = excluded.ota_readiness_topic,
                ota_mode_topic = excluded.ota_mode_topic,
                uses_deep_sleep = excluded.uses_deep_sleep,
                ota_port = excluded.ota_port,
                rssi = excluded.rssi,
                project_folder = excluded.project_folder,
                main_filename = excluded.main_filename
                -- Note: last_updated, state, fail_count, and update_count are preserved on conflict
                -- last_updated is only modified on successful firmware upload via update_device_firmware_version()
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
            .bind((3, device.mac_address.as_str()))
            .map_err(|e| format!("Failed to bind mac_address: {}", e))?;
        statement
            .bind((4, device.firmware_version.as_str()))
            .map_err(|e| format!("Failed to bind firmware_version: {}", e))?;
        statement
            .bind((5, device.last_updated.as_str()))
            .map_err(|e| format!("Failed to bind last_updated: {}", e))?;
        statement
            .bind((6, device.ota_readiness_topic.as_str()))
            .map_err(|e| format!("Failed to bind ota_readiness_topic: {}", e))?;
        statement
            .bind((7, device.ota_mode_topic.as_str()))
            .map_err(|e| format!("Failed to bind ota_mode_topic: {}", e))?;
        statement
            .bind((8, if device.uses_deep_sleep { 1i64 } else { 0i64 }))
            .map_err(|e| format!("Failed to bind uses_deep_sleep: {}", e))?;
        statement
            .bind((9, device.ota_port.map(|p| p as i64).unwrap_or(0i64).max(0)))
            .map_err(|e| format!("Failed to bind ota_port: {}", e))?;
        statement
            .bind((10, device.state.to_string()))
            .map_err(|e| format!("Failed to bind state: {}", e))?;
        statement
            .bind((11, device.fail_count as i64))
            .map_err(|e| format!("Failed to bind fail_count: {}", e))?;
        statement
            .bind((12, device.update_count as i64))
            .map_err(|e| format!("Failed to bind update_count: {}", e))?;
        statement
            .bind((13, device.rssi as i64))
            .map_err(|e| format!("Failed to bind rssi: {}", e))?;
        statement
            .bind((14, device.project_folder.as_ref().map(|s| s.as_str())))
            .map_err(|e| format!("Failed to bind project_folder: {}", e))?;
        statement
            .bind((15, device.main_filename.as_ref().map(|s| s.as_str())))
            .map_err(|e| format!("Failed to bind main_filename: {}", e))?;

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
                mac_address: statement
                    .read::<String, _>("mac_address")
                    .map_err(|e| format!("Failed to read mac_address: {}", e))?,
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
                fail_count: statement.read::<i64, _>("fail_count").unwrap_or(0) as i32,
                update_count: statement.read::<i64, _>("update_count").unwrap_or(0) as i32,
                rssi: statement.read::<i64, _>("rssi").unwrap_or(0) as i32,
                project_folder: statement.read::<String, _>("project_folder").ok(),
                main_filename: statement.read::<String, _>("main_filename").ok(),
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
                mac_address: statement
                    .read::<String, _>("mac_address")
                    .map_err(|e| format!("Failed to read mac_address: {}", e))?,
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
                fail_count: statement.read::<i64, _>("fail_count").unwrap_or(0) as i32,
                update_count: statement.read::<i64, _>("update_count").unwrap_or(0) as i32,
                rssi: statement.read::<i64, _>("rssi").unwrap_or(0) as i32,
                project_folder: statement.read::<String, _>("project_folder").ok(),
                main_filename: statement.read::<String, _>("main_filename").ok(),
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

        let query = "UPDATE devices SET state = ? WHERE device_id = ?";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare update statement: {}", e))?;

        statement
            .bind((1, new_state.to_string()))
            .map_err(|e| format!("Failed to bind state: {}", e))?;
        statement
            .bind((2, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute update: {}", e))?;

        info!("Updated state for device {}: {}", device_id, new_state);
        Ok(())
    }
    /// Increment fail count for a device
    pub fn increment_fail_count(&mut self, device_id: &str) -> Result<(), String> {
        debug!("Incrementing fail count for device {}", device_id);

        let query = "UPDATE devices SET fail_count = fail_count + 1 WHERE device_id = ?";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare update statement: {}", e))?;

        statement
            .bind((1, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute update: {}", e))?;

        info!("Fail count incremented for device {}", device_id);
        Ok(())
    }

    /// Increment update count for a device
    pub fn increment_update_count(&mut self, device_id: &str) -> Result<(), String> {
        debug!("Incrementing update count for device {}", device_id);

        let query = "UPDATE devices SET update_count = update_count + 1, last_updated = ? WHERE device_id = ?";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare update statement: {}", e))?;

        let now = chrono::Local::now().to_rfc3339();

        statement
            .bind((1, now.as_str()))
            .map_err(|e| format!("Failed to bind last_updated: {}", e))?;
        statement
            .bind((2, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to execute update: {}", e))?;

        info!("Update count incremented for device {}", device_id);
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

    /// Add an upload history record
    ///
    /// # Arguments
    /// * `device_id` - Device identifier
    /// * `version` - Firmware version that was uploaded
    /// * `success` - Whether the upload succeeded (true) or failed (false)
    /// * `fail_reason` - Optional failure reason (only relevant when success is false)
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn add_upload_history(
        &mut self,
        device_id: &str,
        version: &str,
        success: bool,
        fail_reason: Option<&str>,
    ) -> Result<(), String> {
        let state = if success { "SUCCESS" } else { "FAIL" };
        debug!(
            "Adding upload history for device {} with version {} - state: {}",
            device_id, version, state
        );

        let now = chrono::Local::now().to_rfc3339();

        let query = "
            INSERT INTO upload_history (device_id, version, state, attempted_at, fail_reason)
            VALUES (?, ?, ?, ?, ?)
        ";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare upload history insert: {}", e))?;

        statement
            .bind((1, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;
        statement
            .bind((2, version))
            .map_err(|e| format!("Failed to bind version: {}", e))?;
        statement
            .bind((3, state))
            .map_err(|e| format!("Failed to bind state: {}", e))?;
        statement
            .bind((4, now.as_str()))
            .map_err(|e| format!("Failed to bind attempted_at: {}", e))?;
        statement
            .bind((5, fail_reason.unwrap_or("")))
            .map_err(|e| format!("Failed to bind fail_reason: {}", e))?;

        statement
            .next()
            .map_err(|e| format!("Failed to insert upload history: {}", e))?;

        info!(
            "Upload history recorded for device {} version {} - {}",
            device_id, version, state
        );
        Ok(())
    }

    /// Get upload history for a specific device
    ///
    /// # Arguments
    /// * `device_id` - Device identifier
    ///
    /// # Returns
    /// Result containing a vector of (version, state, attempted_at) tuples
    pub fn get_upload_history(
        &self,
        device_id: &str,
    ) -> Result<Vec<(String, String, String)>, String> {
        debug!("Getting upload history for device {}", device_id);

        let query = "
            SELECT version, state, attempted_at
            FROM upload_history
            WHERE device_id = ?
            ORDER BY attempted_at DESC
        ";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare upload history query: {}", e))?;

        statement
            .bind((1, device_id))
            .map_err(|e| format!("Failed to bind device_id: {}", e))?;

        let mut history = Vec::new();

        while let Ok(State::Row) = statement.next() {
            let version = statement
                .read::<String, _>("version")
                .map_err(|e| format!("Failed to read version: {}", e))?;
            let state = statement
                .read::<String, _>("state")
                .map_err(|e| format!("Failed to read state: {}", e))?;
            let attempted_at = statement
                .read::<String, _>("attempted_at")
                .map_err(|e| format!("Failed to read attempted_at: {}", e))?;

            history.push((version, state, attempted_at));
        }

        debug!("Found {} upload history records", history.len());
        Ok(history)
    }

    /// Get all upload history records
    ///
    /// # Returns
    /// Result containing a vector of (device_id, version, state, attempted_at) tuples
    pub fn get_all_upload_history(&self) -> Result<Vec<(String, String, String, String)>, String> {
        debug!("Getting all upload history");

        let query = "
            SELECT device_id, version, state, attempted_at
            FROM upload_history
            ORDER BY attempted_at DESC
        ";

        let mut statement = self
            .connection
            .prepare(query)
            .map_err(|e| format!("Failed to prepare upload history query: {}", e))?;

        let mut history = Vec::new();

        while let Ok(State::Row) = statement.next() {
            let device_id = statement
                .read::<String, _>("device_id")
                .map_err(|e| format!("Failed to read device_id: {}", e))?;
            let version = statement
                .read::<String, _>("version")
                .map_err(|e| format!("Failed to read version: {}", e))?;
            let state = statement
                .read::<String, _>("state")
                .map_err(|e| format!("Failed to read state: {}", e))?;
            let attempted_at = statement
                .read::<String, _>("attempted_at")
                .map_err(|e| format!("Failed to read attempted_at: {}", e))?;

            history.push((device_id, version, state, attempted_at));
        }

        debug!("Found {} upload history records", history.len());
        Ok(history)
    }

    /// Get last N upload history records with all fields including fail_reason
    ///
    /// # Arguments
    /// * `limit` - Maximum number of records to retrieve
    ///
    /// # Returns
    /// Result containing a vector of UploadHistoryEntry
    pub fn get_recent_upload_history(
        &self,
        limit: usize,
    ) -> Result<Vec<UploadHistoryEntry>, String> {
        debug!("Getting last {} upload history records", limit);

        let query = format!(
            "SELECT device_id, version, state, attempted_at, fail_reason
             FROM upload_history
             ORDER BY attempted_at DESC
             LIMIT {}",
            limit
        );

        let mut statement = self
            .connection
            .prepare(&query)
            .map_err(|e| format!("Failed to prepare upload history query: {}", e))?;

        let mut history = Vec::new();

        while let Ok(State::Row) = statement.next() {
            let device_id = statement
                .read::<String, _>("device_id")
                .map_err(|e| format!("Failed to read device_id: {}", e))?;
            let version = statement
                .read::<String, _>("version")
                .map_err(|e| format!("Failed to read version: {}", e))?;
            let state = statement
                .read::<String, _>("state")
                .map_err(|e| format!("Failed to read state: {}", e))?;
            let attempted_at = statement
                .read::<String, _>("attempted_at")
                .map_err(|e| format!("Failed to read attempted_at: {}", e))?;
            let fail_reason = statement
                .read::<Option<String>, _>("fail_reason")
                .ok()
                .flatten();

            history.push(UploadHistoryEntry {
                device_id,
                version,
                state,
                attempted_at,
                fail_reason,
            });
        }

        debug!("Found {} upload history records", history.len());
        Ok(history)
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
