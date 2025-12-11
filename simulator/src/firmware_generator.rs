use log::{error, info};
use rand::Rng;
use std::fs;
use std::path::{Path, PathBuf};
use tokio::time::{sleep, Duration};

/// Generates fake firmware files for simulated devices
pub struct FirmwareGenerator {
    storage_path: PathBuf,
    num_devices: u32,
    device_id_prefix: String,
    current_version: String,
    min_interval: u64,
    max_interval: u64,
}

impl FirmwareGenerator {
    /// Create a new firmware generator
    pub fn new(
        storage_path: String,
        num_devices: u32,
        device_id_prefix: String,
        initial_version: String,
        min_interval: u64,
        max_interval: u64,
    ) -> Result<Self, String> {
        let path = Path::new(&storage_path);

        // Create storage directory if it doesn't exist
        if !path.exists() {
            fs::create_dir_all(path)
                .map_err(|e| format!("Failed to create firmware directory: {}", e))?;
            info!("Created firmware directory: {:?}", path);
        }

        Ok(FirmwareGenerator {
            storage_path: path.to_path_buf(),
            num_devices,
            device_id_prefix,
            current_version: initial_version,
            min_interval,
            max_interval,
        })
    }

    /// Start the firmware generation task
    pub async fn start_generation_task(&mut self) {
        info!("Starting firmware generation task");
        info!(
            "Generation interval: {} to {} seconds",
            self.min_interval, self.max_interval
        );

        loop {
            // Wait random interval
            let interval = self.random_interval();
            info!("Next firmware generation in {} seconds", interval.as_secs());
            sleep(interval).await;

            // Generate new version
            let new_version = self.increment_version();
            info!("Generating firmware version: {}", new_version);

            // Randomly select some devices to get new firmware (not all)
            let num_updates = rand::rng().random_range(1..=self.num_devices.min(5));
            let mut updated_devices = vec![];

            for _ in 0..num_updates {
                let device_num = rand::rng().random_range(1..=self.num_devices);
                let device_id = format!("{}{:03}", self.device_id_prefix, device_num);

                if !updated_devices.contains(&device_id) {
                    if let Err(e) = self.generate_firmware_file(&device_id, &new_version) {
                        error!("Failed to generate firmware for {}: {}", device_id, e);
                    } else {
                        updated_devices.push(device_id.clone());
                        info!("Generated firmware: {} - {}", device_id, new_version);
                    }
                }
            }

            info!(
                "Firmware generation complete. Updated {} devices",
                updated_devices.len()
            );
        }
    }

    /// Generate a firmware file for a specific device
    fn generate_firmware_file(&self, device_id: &str, version: &str) -> Result<(), String> {
        let filename = format!("{} - {}.bin", device_id, version);
        let filepath = self.storage_path.join(&filename);

        // Generate fake firmware content (random bytes)
        let size = rand::rng().random_range(100_000..500_000); // 100KB to 500KB
        let mut content: Vec<u8> = Vec::new();

        // Prepend version string to firmware in square brackets
        let version_header = format!("[VERSION-{}]", version);
        content.extend_from_slice(version_header.as_bytes());

        // Add random firmware data
        let random_data: Vec<u8> = (0..size).map(|_| rand::random::<u8>()).collect();
        content.extend_from_slice(&random_data);

        fs::write(&filepath, content)
            .map_err(|e| format!("Failed to write firmware file: {}", e))?;

        Ok(())
    }

    /// Get random interval between min and max
    fn random_interval(&self) -> Duration {
        let seconds = rand::rng().random_range(self.min_interval..=self.max_interval);
        Duration::from_secs(seconds)
    }

    /// Increment version (simple semantic versioning)
    fn increment_version(&mut self) -> String {
        let parts: Vec<&str> = self.current_version.split('.').collect();
        if parts.len() == 3 {
            if let (Ok(major), Ok(minor), Ok(patch)) = (
                parts[0].parse::<u32>(),
                parts[1].parse::<u32>(),
                parts[2].parse::<u32>(),
            ) {
                // Increment patch version
                let new_patch = patch + 1;
                self.current_version = format!("{}.{}.{}", major, minor, new_patch);
            }
        }
        self.current_version.clone()
    }
}
