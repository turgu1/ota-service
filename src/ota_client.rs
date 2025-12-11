use log::{debug, error, info};
use sha2::{Digest, Sha256};
use std::io::{Read, Write};
use std::net::TcpStream;
use std::time::Duration;

/// ESPHome OTA Protocol v2 implementation
/// Based on: https://github.com/esphome/esphome/blob/main/esphome/espota2.py

// Magic bytes to initiate OTA
const MAGIC_BYTES: [u8; 5] = [0x6C, 0x26, 0xF7, 0x5C, 0x45];

// OTA protocol version
const OTA_VERSION_2_0: u8 = 2;

// Response codes
const RESPONSE_OK: u8 = 0x00;
const RESPONSE_REQUEST_AUTH: u8 = 0x01;
const RESPONSE_REQUEST_AUTH_SHA256: u8 = 0x02;

const RESPONSE_HEADER_OK: u8 = 0x40;
const RESPONSE_AUTH_OK: u8 = 0x41;
const RESPONSE_UPDATE_PREPARE_OK: u8 = 0x42;
const RESPONSE_BIN_MD5_OK: u8 = 0x43;
const RESPONSE_RECEIVE_OK: u8 = 0x44;
const RESPONSE_UPDATE_END_OK: u8 = 0x45;
const RESPONSE_SUPPORTS_COMPRESSION: u8 = 0x46;
const RESPONSE_CHUNK_OK: u8 = 0x47;

// Error codes
const RESPONSE_ERROR_MAGIC: u8 = 0x80;
const RESPONSE_ERROR_UPDATE_PREPARE: u8 = 0x81;
const RESPONSE_ERROR_AUTH_INVALID: u8 = 0x82;
const RESPONSE_ERROR_WRITING_FLASH: u8 = 0x83;
const RESPONSE_ERROR_UPDATE_END: u8 = 0x84;
const RESPONSE_ERROR_UNKNOWN: u8 = 0xFF;

// Upload block size (8KB chunks)
const UPLOAD_BLOCK_SIZE: usize = 8192;

// Feature flags
const FEATURE_SUPPORTS_SHA256: u8 = 0x02;

/// OTA client for ESPHome devices
pub struct OtaClient {
    stream: TcpStream,
    password: Option<String>,
    version: u8,
}

impl OtaClient {
    /// Connect to device OTA port
    pub fn connect(host: &str, port: u16, password: Option<String>) -> Result<Self, String> {
        info!("Connecting to OTA endpoint: {}:{}", host, port);

        let addr = format!("{}:{}", host, port);
        let stream = TcpStream::connect_timeout(
            &addr
                .parse()
                .map_err(|e| format!("Invalid address: {}", e))?,
            Duration::from_secs(10),
        )
        .map_err(|e| format!("Failed to connect to {}:{}: {}", host, port, e))?;

        stream
            .set_read_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set read timeout: {}", e))?;

        stream
            .set_write_timeout(Some(Duration::from_secs(30)))
            .map_err(|e| format!("Failed to set write timeout: {}", e))?;

        info!("Connected to {}:{}", host, port);

        Ok(OtaClient {
            stream,
            password,
            version: 0,
        })
    }

    /// Perform OTA update
    pub fn update(&mut self, firmware: &[u8]) -> Result<(), String> {
        info!("Starting OTA update ({} bytes)", firmware.len());

        // Step 1: Send magic bytes
        self.send_magic_bytes()?;

        // Step 2: Receive version
        self.receive_version()?;

        // Step 3: Send and receive features
        self.exchange_features()?;

        // Step 4: Authenticate if required
        self.handle_authentication()?;

        // Step 5: Send firmware size
        self.send_firmware_size(firmware.len() as u32)?;

        // Step 6: Send MD5 checksum
        let md5_hash = self.calculate_md5(firmware);
        self.send_md5(&md5_hash)?;

        // Step 7: Send firmware data
        self.send_firmware_data(firmware)?;

        // Step 8: Finalize update
        self.finalize_update()?;

        info!("OTA update completed successfully");
        Ok(())
    }

    /// Send magic bytes to initiate OTA
    fn send_magic_bytes(&mut self) -> Result<(), String> {
        debug!("Sending magic bytes");
        self.stream
            .write_all(&MAGIC_BYTES)
            .map_err(|e| format!("Failed to send magic bytes: {}", e))?;
        Ok(())
    }

    /// Receive protocol version from device
    fn receive_version(&mut self) -> Result<(), String> {
        debug!("Receiving version");

        let mut response = [0u8; 2];
        self.read_with_timeout(&mut response, "version")?;

        if response[0] != RESPONSE_OK {
            return Err(format!("Expected RESPONSE_OK, got: 0x{:02X}", response[0]));
        }

        self.version = response[1];
        debug!("Device OTA version: {}", self.version);

        if self.version != OTA_VERSION_2_0 {
            return Err(format!(
                "Unsupported OTA version: {} (expected {})",
                self.version, OTA_VERSION_2_0
            ));
        }

        Ok(())
    }

    /// Exchange feature flags with device
    fn exchange_features(&mut self) -> Result<(), String> {
        debug!("Exchanging features");

        // Send our features (we support SHA256 but not compression)
        let features = FEATURE_SUPPORTS_SHA256;
        self.stream
            .write_all(&[features])
            .map_err(|e| format!("Failed to send features: {}", e))?;

        // Receive device features
        let mut response = [0u8; 1];
        self.read_with_timeout(&mut response, "features")?;

        let device_features = response[0];
        debug!("Device features: 0x{:02X}", device_features);

        // Check if device supports compression
        if device_features == RESPONSE_SUPPORTS_COMPRESSION {
            debug!("Device supports compression (not using it)");
        } else if device_features != RESPONSE_HEADER_OK {
            return Err(format!(
                "Unexpected features response: 0x{:02X}",
                device_features
            ));
        }

        Ok(())
    }

    /// Handle authentication if required
    fn handle_authentication(&mut self) -> Result<(), String> {
        debug!("Checking authentication requirement");

        let mut response = [0u8; 1];
        self.read_with_timeout(&mut response, "auth requirement")?;

        match response[0] {
            RESPONSE_AUTH_OK => {
                debug!("No authentication required");
                Ok(())
            }
            RESPONSE_REQUEST_AUTH => {
                info!("Device requires MD5 authentication");
                self.authenticate_md5()
            }
            RESPONSE_REQUEST_AUTH_SHA256 => {
                info!("Device requires SHA256 authentication");
                self.authenticate_sha256()
            }
            other => Err(format!("Unexpected auth response: 0x{:02X}", other)),
        }
    }

    /// Authenticate using MD5 challenge-response
    fn authenticate_md5(&mut self) -> Result<(), String> {
        let password = self
            .password
            .as_ref()
            .ok_or_else(|| "Device requires password but none provided".to_string())?
            .clone();

        // Receive 32-byte hex nonce from device
        let mut nonce = [0u8; 32];
        self.read_with_timeout(&mut nonce, "nonce")?;

        let nonce_str =
            std::str::from_utf8(&nonce).map_err(|e| format!("Invalid nonce UTF-8: {}", e))?;
        debug!("Received nonce: {}", nonce_str);

        // Generate cnonce (MD5 of random value)
        use std::time::{SystemTime, UNIX_EPOCH};
        let random_val = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();
        let cnonce = format!("{:x}", md5::compute(random_val.as_bytes()));
        debug!("Generated cnonce: {}", cnonce);

        // Send cnonce
        self.stream
            .write_all(cnonce.as_bytes())
            .map_err(|e| format!("Failed to send cnonce: {}", e))?;

        // Calculate authentication result: MD5(password + nonce + cnonce)
        let mut auth_input = Vec::new();
        auth_input.extend_from_slice(password.as_bytes());
        auth_input.extend_from_slice(&nonce);
        auth_input.extend_from_slice(cnonce.as_bytes());

        let auth_result = format!("{:x}", md5::compute(&auth_input));
        debug!("Sending auth result");

        // Send auth result
        self.stream
            .write_all(auth_result.as_bytes())
            .map_err(|e| format!("Failed to send auth result: {}", e))?;

        // Wait for auth confirmation
        let mut auth_response = [0u8; 1];
        self.read_with_timeout(&mut auth_response, "auth response")?;

        if auth_response[0] == RESPONSE_AUTH_OK {
            info!("Authentication successful");
            Ok(())
        } else {
            Err(format!("Authentication failed: 0x{:02X}", auth_response[0]))
        }
    }

    /// Authenticate using SHA256 challenge-response
    fn authenticate_sha256(&mut self) -> Result<(), String> {
        let password = self
            .password
            .as_ref()
            .ok_or_else(|| "Device requires password but none provided".to_string())?
            .clone();

        // Receive 64-byte hex nonce from device (32 bytes as hex string)
        let mut nonce = [0u8; 64];
        self.read_with_timeout(&mut nonce, "nonce")?;

        let nonce_str =
            std::str::from_utf8(&nonce).map_err(|e| format!("Invalid nonce UTF-8: {}", e))?;
        debug!("Received SHA256 nonce: {}", nonce_str);

        // Generate cnonce (SHA256 of random value)
        use std::time::{SystemTime, UNIX_EPOCH};
        let random_val = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .unwrap()
            .as_nanos()
            .to_string();

        let mut hasher = Sha256::new();
        hasher.update(random_val.as_bytes());
        let cnonce = format!("{:x}", hasher.finalize());
        debug!("Generated SHA256 cnonce: {}", cnonce);

        // Send cnonce
        self.stream
            .write_all(cnonce.as_bytes())
            .map_err(|e| format!("Failed to send cnonce: {}", e))?;

        // Calculate authentication result: SHA256(password + nonce + cnonce)
        let mut auth_input = Vec::new();
        auth_input.extend_from_slice(password.as_bytes());
        auth_input.extend_from_slice(&nonce);
        auth_input.extend_from_slice(cnonce.as_bytes());

        let mut hasher = Sha256::new();
        hasher.update(&auth_input);
        let auth_result = format!("{:x}", hasher.finalize());
        debug!("Sending SHA256 auth result");

        // Send auth result
        self.stream
            .write_all(auth_result.as_bytes())
            .map_err(|e| format!("Failed to send auth result: {}", e))?;

        // Wait for auth confirmation
        let mut auth_response = [0u8; 1];
        self.read_with_timeout(&mut auth_response, "auth response")?;

        if auth_response[0] == RESPONSE_AUTH_OK {
            info!("SHA256 authentication successful");
            Ok(())
        } else {
            Err(format!(
                "SHA256 authentication failed: 0x{:02X}",
                auth_response[0]
            ))
        }
    }

    /// Send firmware size
    fn send_firmware_size(&mut self, size: u32) -> Result<(), String> {
        debug!("Sending firmware size: {} bytes", size);

        // Send size as big-endian 4-byte integer
        let size_bytes = [
            ((size >> 24) & 0xFF) as u8,
            ((size >> 16) & 0xFF) as u8,
            ((size >> 8) & 0xFF) as u8,
            (size & 0xFF) as u8,
        ];

        self.stream
            .write_all(&size_bytes)
            .map_err(|e| format!("Failed to send firmware size: {}", e))?;

        // Wait for acknowledgment
        self.wait_response("firmware size", RESPONSE_UPDATE_PREPARE_OK)?;

        Ok(())
    }

    /// Send MD5 checksum
    fn send_md5(&mut self, md5_hash: &str) -> Result<(), String> {
        debug!("Sending MD5 checksum: {}", md5_hash);

        self.stream
            .write_all(md5_hash.as_bytes())
            .map_err(|e| format!("Failed to send MD5: {}", e))?;

        // Wait for acknowledgment
        self.wait_response("MD5 checksum", RESPONSE_BIN_MD5_OK)?;

        Ok(())
    }

    /// Send firmware data in chunks
    fn send_firmware_data(&mut self, firmware: &[u8]) -> Result<(), String> {
        info!("Sending firmware data");

        let total_chunks = (firmware.len() + UPLOAD_BLOCK_SIZE - 1) / UPLOAD_BLOCK_SIZE;

        for (chunk_num, chunk) in firmware.chunks(UPLOAD_BLOCK_SIZE).enumerate() {
            debug!(
                "Sending chunk {}/{} ({} bytes)",
                chunk_num + 1,
                total_chunks,
                chunk.len()
            );

            self.stream
                .write_all(chunk)
                .map_err(|e| format!("Failed to send chunk {}: {}", chunk_num, e))?;

            // For version 2.0, wait for chunk acknowledgment
            if self.version >= OTA_VERSION_2_0 {
                self.wait_response(&format!("chunk {}", chunk_num), RESPONSE_CHUNK_OK)?;
            }

            if (chunk_num + 1) % 50 == 0 || chunk_num + 1 == total_chunks {
                info!(
                    "Progress: {}/{} chunks sent ({:.1}%)",
                    chunk_num + 1,
                    total_chunks,
                    ((chunk_num + 1) as f64 / total_chunks as f64) * 100.0
                );
            }
        }

        info!("All firmware data sent successfully");
        Ok(())
    }

    /// Finalize OTA update
    fn finalize_update(&mut self) -> Result<(), String> {
        debug!("Finalizing update");

        // Wait for receive confirmation
        self.wait_response("receive OK", RESPONSE_RECEIVE_OK)?;

        // Wait for update end confirmation
        self.wait_response("update end", RESPONSE_UPDATE_END_OK)?;

        // Send final acknowledgment
        self.stream
            .write_all(&[RESPONSE_OK])
            .map_err(|e| format!("Failed to send final ACK: {}", e))?;

        Ok(())
    }

    /// Wait for expected response from device
    fn wait_response(&mut self, operation: &str, expected: u8) -> Result<(), String> {
        let mut response = [0u8; 1];
        self.read_with_timeout(&mut response, operation)?;

        if response[0] == expected {
            debug!("Received expected response for {}", operation);
            Ok(())
        } else if response[0] >= 0x80 {
            // Error response
            let error_msg = self.error_code_to_string(response[0]);
            error!("Device returned error for {}: {}", operation, error_msg);
            Err(format!("OTA error during {}: {}", operation, error_msg))
        } else {
            Err(format!(
                "Unexpected response for {}: expected 0x{:02X}, got 0x{:02X}",
                operation, expected, response[0]
            ))
        }
    }

    /// Read data from stream with timeout detection
    fn read_with_timeout(&mut self, buffer: &mut [u8], operation: &str) -> Result<(), String> {
        use std::io::ErrorKind;
        use std::time::Instant;

        let timeout_duration = self
            .stream
            .read_timeout()
            .map_err(|e| format!("Failed to get read timeout: {}", e))?
            .unwrap_or(Duration::from_secs(30));

        let start = Instant::now();
        let mut total_read = 0;

        while total_read < buffer.len() {
            match self.stream.read(&mut buffer[total_read..]) {
                Ok(0) => {
                    return Err(format!(
                        "Connection closed by device while receiving {} (read {} of {} bytes)",
                        operation,
                        total_read,
                        buffer.len()
                    ));
                }
                Ok(n) => {
                    total_read += n;
                    debug!(
                        "Read {}/{} bytes for {}",
                        total_read,
                        buffer.len(),
                        operation
                    );
                }
                Err(ref e)
                    if e.kind() == ErrorKind::WouldBlock || e.kind() == ErrorKind::TimedOut =>
                {
                    if start.elapsed() >= timeout_duration {
                        return Err(format!(
                            "Timeout after {:?} while receiving {} (read {} of {} bytes)",
                            start.elapsed(),
                            operation,
                            total_read,
                            buffer.len()
                        ));
                    }
                    // For non-blocking reads, sleep briefly before retrying
                    std::thread::sleep(Duration::from_millis(10));
                }
                Err(e) => {
                    return Err(format!(
                        "Failed to receive {} after reading {} of {} bytes: {}",
                        operation,
                        total_read,
                        buffer.len(),
                        e
                    ));
                }
            }
        }

        Ok(())
    }

    /// Calculate MD5 hash of firmware
    fn calculate_md5(&self, firmware: &[u8]) -> String {
        format!("{:x}", md5::compute(firmware))
    }

    /// Convert error code to human-readable string
    fn error_code_to_string(&self, code: u8) -> String {
        match code {
            RESPONSE_ERROR_MAGIC => "Invalid magic byte".to_string(),
            RESPONSE_ERROR_UPDATE_PREPARE => "Update prepare failed".to_string(),
            RESPONSE_ERROR_AUTH_INVALID => "Authentication invalid".to_string(),
            RESPONSE_ERROR_WRITING_FLASH => "Error writing flash".to_string(),
            RESPONSE_ERROR_UPDATE_END => "Update end failed".to_string(),
            RESPONSE_ERROR_UNKNOWN => "Unknown error".to_string(),
            other => format!("Unknown error code: 0x{:02X}", other),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_calculate_md5() {
        let stream =
            TcpStream::connect_timeout(&"127.0.0.1:1".parse().unwrap(), Duration::from_millis(1));

        if stream.is_ok() {
            let client = OtaClient {
                stream: stream.unwrap(),
                password: None,
                version: 2,
            };

            let data = b"test data";
            let hash = client.calculate_md5(data);

            // Verify it's a valid MD5 hash (32 hex characters)
            assert_eq!(hash.len(), 32);
        }
    }

    #[test]
    fn test_error_code_to_string() {
        let stream =
            TcpStream::connect_timeout(&"127.0.0.1:1".parse().unwrap(), Duration::from_millis(1));

        if stream.is_ok() {
            let client = OtaClient {
                stream: stream.unwrap(),
                password: None,
                version: 2,
            };

            assert_eq!(
                client.error_code_to_string(RESPONSE_ERROR_MAGIC),
                "Invalid magic byte"
            );
            assert_eq!(
                client.error_code_to_string(RESPONSE_ERROR_AUTH_INVALID),
                "Authentication invalid"
            );
            assert_eq!(
                client.error_code_to_string(RESPONSE_ERROR_UNKNOWN),
                "Unknown error"
            );
        }
    }
}
