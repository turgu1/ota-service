use log::{debug, error, info, warn};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::{Path, PathBuf};

/// Firmware metadata
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FirmwareInfo {
    /// Device ID this firmware is for
    pub device_id: String,
    /// Firmware version
    pub version: String,
    /// File path to the firmware binary
    pub file_path: PathBuf,
    /// File size in bytes
    pub size: u64,
}

/// Firmware manager for handling firmware files
pub struct FirmwareManager {
    /// Directory where firmware files are stored
    storage_path: PathBuf,
}

impl FirmwareManager {
    /// Create a new firmware manager
    ///
    /// # Arguments
    /// * `storage_path` - Directory where firmware files are stored
    ///
    /// # Returns
    /// Result containing the FirmwareManager or error message
    pub fn new<P: AsRef<Path>>(storage_path: P) -> Result<Self, String> {
        let path = storage_path.as_ref().to_path_buf();

        info!(
            "Initializing firmware manager with storage path: {:?}",
            path
        );

        // Create storage directory if it doesn't exist
        if !path.exists() {
            info!("Creating firmware storage directory: {:?}", path);
            fs::create_dir_all(&path)
                .map_err(|e| format!("Failed to create storage directory: {}", e))?;
        }

        Ok(FirmwareManager { storage_path: path })
    }

    /// Get firmware info for a specific device
    ///
    /// Scans the storage directory for firmware files matching the pattern:
    /// {device_id}-{version}.bin
    ///
    /// # Arguments
    /// * `device_id` - Device ID to search for
    ///
    /// # Returns
    /// Vector of FirmwareInfo for the device, sorted by version (newest first)
    pub fn get_firmware_for_device(&self, device_id: &str) -> Result<Vec<FirmwareInfo>, String> {
        debug!("Scanning for firmware files for device: {}", device_id);

        let mut firmware_list = Vec::new();

        let entries = fs::read_dir(&self.storage_path)
            .map_err(|e| format!("Failed to read storage directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    // Expected format: {device_id}-{version}.bin
                    if filename.ends_with(".bin") && filename.starts_with(device_id) {
                        // Extract version from filename
                        let name_without_ext = filename.trim_end_matches(".bin");
                        if let Some(version_part) = name_without_ext.strip_prefix(device_id) {
                            let version = version_part.trim_start_matches('-');
                            if !version.is_empty() {
                                let metadata = fs::metadata(&path)
                                    .map_err(|e| format!("Failed to read file metadata: {}", e))?;

                                firmware_list.push(FirmwareInfo {
                                    device_id: device_id.to_string(),
                                    version: version.to_string(),
                                    file_path: path.clone(),
                                    size: metadata.len(),
                                });

                                debug!(
                                    "Found firmware: {} version {} ({} bytes)",
                                    device_id,
                                    version,
                                    metadata.len()
                                );
                            }
                        }
                    }
                }
            }
        }

        // Sort by version (newest first)
        firmware_list.sort_by(|a, b| {
            use version_compare::Cmp;
            version_compare::compare(&b.version, &a.version)
                .unwrap_or(Cmp::Eq)
                .ord()
                .unwrap_or(std::cmp::Ordering::Equal)
        });

        debug!(
            "Found {} firmware file(s) for device {}",
            firmware_list.len(),
            device_id
        );

        Ok(firmware_list)
    }

    /// Get newer firmware version for a device
    ///
    /// # Arguments
    /// * `device_id` - Device ID
    /// * `current_version` - Current firmware version on the device
    ///
    /// # Returns
    /// Option containing FirmwareInfo for newer version, or None if no newer version available
    pub fn get_newer_version(
        &self,
        device_id: &str,
        current_version: &str,
    ) -> Result<Option<FirmwareInfo>, String> {
        debug!(
            "Checking for newer firmware for device {} (current: {})",
            device_id, current_version
        );

        let firmware_list = self.get_firmware_for_device(device_id)?;

        // Find the first (newest) version that is greater than current_version
        for firmware in firmware_list {
            match version_compare::compare(&firmware.version, current_version) {
                Ok(version_compare::Cmp::Gt) | Ok(version_compare::Cmp::Ge) => {
                    info!(
                        "Found newer firmware for device {}: {} -> {}",
                        device_id, current_version, firmware.version
                    );
                    return Ok(Some(firmware));
                }
                Ok(version_compare::Cmp::Eq) => {
                    debug!(
                        "Firmware version {} matches current version",
                        firmware.version
                    );
                }
                Ok(version_compare::Cmp::Lt)
                | Ok(version_compare::Cmp::Le)
                | Ok(version_compare::Cmp::Ne) => {
                    debug!(
                        "Firmware version {} is not newer than current version",
                        firmware.version
                    );
                }
                Err(_) => {
                    warn!("Failed to compare versions - invalid version format");
                }
            }
        }

        debug!(
            "No newer firmware found for device {} (current: {})",
            device_id, current_version
        );
        Ok(None)
    }

    /// Read firmware binary data
    ///
    /// # Arguments
    /// * `firmware` - FirmwareInfo containing path to firmware file
    ///
    /// # Returns
    /// Vec<u8> containing the firmware binary data
    pub fn read_firmware(&self, firmware: &FirmwareInfo) -> Result<Vec<u8>, String> {
        debug!("Reading firmware file: {:?}", firmware.file_path);

        let data = fs::read(&firmware.file_path)
            .map_err(|e| format!("Failed to read firmware file: {}", e))?;

        info!(
            "Read firmware file: {:?} ({} bytes)",
            firmware.file_path,
            data.len()
        );

        Ok(data)
    }

    /// Delete firmware file
    ///
    /// # Arguments
    /// * `firmware` - FirmwareInfo containing path to firmware file to delete
    ///
    /// # Returns
    /// Result indicating success or error
    pub fn delete_firmware(&self, firmware: &FirmwareInfo) -> Result<(), String> {
        info!("Deleting firmware file: {:?}", firmware.file_path);

        fs::remove_file(&firmware.file_path)
            .map_err(|e| format!("Failed to delete firmware file: {}", e))?;

        info!("Firmware file deleted: {:?}", firmware.file_path);
        Ok(())
    }

    /// List all firmware files in storage
    pub fn list_all_firmware(&self) -> Result<Vec<FirmwareInfo>, String> {
        debug!("Listing all firmware files");

        let mut firmware_list = Vec::new();

        let entries = fs::read_dir(&self.storage_path)
            .map_err(|e| format!("Failed to read storage directory: {}", e))?;

        for entry in entries {
            let entry = entry.map_err(|e| format!("Failed to read directory entry: {}", e))?;
            let path = entry.path();

            if path.is_file() {
                if let Some(filename) = path.file_name().and_then(|n| n.to_str()) {
                    if filename.ends_with(".bin") {
                        // Parse filename: {device_id}-{version}.bin
                        let name_without_ext = filename.trim_end_matches(".bin");
                        if let Some(dash_pos) = name_without_ext.rfind('-') {
                            let device_id = &name_without_ext[..dash_pos];
                            let version = &name_without_ext[dash_pos + 1..];

                            if !device_id.is_empty() && !version.is_empty() {
                                let metadata = fs::metadata(&path)
                                    .map_err(|e| format!("Failed to read file metadata: {}", e))?;

                                firmware_list.push(FirmwareInfo {
                                    device_id: device_id.to_string(),
                                    version: version.to_string(),
                                    file_path: path.clone(),
                                    size: metadata.len(),
                                });
                            }
                        }
                    }
                }
            }
        }

        debug!("Found {} total firmware file(s)", firmware_list.len());
        Ok(firmware_list)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_firmware_manager_creation() {
        let temp_dir = "/tmp/test_firmware_storage";
        let _ = fs::remove_dir_all(temp_dir);

        let manager = FirmwareManager::new(temp_dir);
        assert!(manager.is_ok());

        assert!(Path::new(temp_dir).exists());

        let _ = fs::remove_dir_all(temp_dir);
    }

    #[test]
    fn test_firmware_info() {
        let info = FirmwareInfo {
            device_id: "esp32-001".to_string(),
            version: "1.2.3".to_string(),
            file_path: PathBuf::from("/tmp/test.bin"),
            size: 12345,
        };

        assert_eq!(info.device_id, "esp32-001");
        assert_eq!(info.version, "1.2.3");
        assert_eq!(info.size, 12345);
    }
}
