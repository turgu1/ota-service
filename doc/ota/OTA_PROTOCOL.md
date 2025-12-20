# ESPHome OTA Protocol Implementation

## Overview

This module implements the ESPHome native OTA (Over-The-Air) protocol version 2 for uploading firmware to devices running ESPHome firmware. The protocol typically communicates over TCP port 3232 (configurable per-device).

## Protocol Version

**Current Implementation: ESPHome OTA Protocol v2**

- **Version**: 2
- **Magic Bytes**: `[0x6C, 0x26, 0xF7, 0x5C, 0x45]` (5 bytes)
- **Features**: 
  - Compression support (0x01)
  - SHA256 authentication (0x02)
  - Chunk acknowledgments every 8192 bytes
  - Big-endian firmware size transmission
  - Actual MD5 checksum calculation

## Architecture

### OtaClient Module (`src/ota_client.rs`)

The `OtaClient` struct handles low-level communication with ESPHome devices using the native OTA protocol.

**Key Components:**
- `OtaClient::new(device_ip, device_port)` - Create a new client
- `upload_firmware(firmware_data, auth_password)` - Perform OTA upload
- Protocol message types: HelloRequest/Response, FileRequest/Response, UpdateFinished/Failed, Ping/Pong

### Protocol Flow

```
Client                                       Device (ESPHome)
  |                                                |
  |----------- Magic Bytes (5 bytes) ------------->|
  |<--------- Version Response (2 bytes) ----------|
  |                                                |
  |----------- Features (compression+SHA256) ----->|
  |<--------- Feature Response --------------------|
  |                                                |
  |<--------- Auth Request (if required) ----------|
  |----------- Auth Response (seed+digest) ------->|
  |<--------- Auth OK -----------------------------|
  |                                                |
  |----------- Firmware Size (big-endian) -------->|
  |<--------- Prepare OK --------------------------|
  |                                                |
  |----------- MD5 Checksum (32 hex chars) ------->|
  |<--------- MD5 OK ------------------------------|
  |                                                |
  |----------- Firmware Chunks (1024 bytes) ------>|
  |<--------- Chunk ACK (every 8192 bytes) --------|
  |                                                |
  |<--------- Receive OK --------------------------|
  |<--------- Update End OK -----------------------|
  |                                                |
```

## Usage Examples

### Example 1: Basic Firmware Upload

```rust
use ota_client::OtaClient;

#[tokio::main]
async fn main() {
    let client = OtaClient::new("192.168.1.100".to_string(), 3232);
    
    let firmware_data = std::fs::read("firmware.bin").expect("Failed to read firmware");
    
    match client.upload_firmware(&firmware_data, None).await {
        Ok(()) => println!("Upload successful!"),
        Err(e) => println!("Upload failed: {}", e),
    }
}
```

### Example 2: Upload with Authentication

```rust
let client = OtaClient::new("192.168.1.100".to_string(), 3232)
    .with_timeout(60);

let firmware_data = std::fs::read("firmware.bin")?;

client.upload_firmware(&firmware_data, Some("my_password")).await?;
```

### Example 3: Using OTA Service

The `OtaService` provides a high-level API that integrates with your device database and firmware manager:

```rust
// Upload firmware to a specific device
ota_service
    .upload_firmware_ota(
        "device-001",           // device_id
        "192.168.1.100",       // device_ip
        "1.2.3",               // firmware_version
        None,                  // auth_password
    )
    .await?;
```

## Protocol Details

### Message Structure

All messages follow the ESPHome OTA protocol format:

```
[5 bytes: Message size (little-endian)]
[5 bytes: Magic [0x6C, 0x26, 0xF7, 0x5C, 0x45]]
[1 byte: Protocol version (2)]
[1 byte: Message type]
[remaining bytes: Message payload]
```

### Message Types

| Type | Value | Direction | Purpose |
|------|-------|-----------|---------|
| HelloRequest | 0 | Client→Device | Initiate OTA session |
| HelloResponse | 1 | Device→Client | Device acknowledges, sends capabilities |
| FileRequest | 2 | Client→Device | Request to upload file with size/checksum |
| FileResponse | 3 | Device→Client | Device ready to receive firmware |
| UpdateFinished | 4 | Device→Client | Update completed successfully |
| UpdateFailed | 5 | Device→Client | Update failed |
| Ping | 6 | Either | Keep-alive |
| Pong | 7 | Either | Keep-alive response |

### Protocol Constants

- **Magic bytes**: `[0x6C, 0x26, 0xF7, 0x5C, 0x45]` (5 bytes)
- **Protocol version**: 2
- **Default port**: 3232
- **Chunk size**: 1024 bytes
- **Acknowledgment interval**: 8192 bytes (every 8 chunks)
- **Default timeout**: 30 seconds (configurable)
- **Features byte**: 0x03 (compression + SHA256 auth support)

### Response Codes

**Success Responses (0x40-0x47):**
- `0x40` - HeaderOk: Magic bytes accepted
- `0x41` - AuthOk: Authentication successful
- `0x42` - UpdatePrepareOk: Device ready for firmware
- `0x43` - BinaryMd5Ok: MD5 checksum accepted
- `0x44` - ReceiveOk: All data received
- `0x45` - UpdateEndOk: Update completed
- `0x46` - SupportsCompression: Device supports compression
- `0x47` - ChunkOk: Chunk received successfully

**Error Responses (0x80-0x8C, 0xFF):**
- `0x80` - ErrorMagic: Invalid magic bytes
- `0x81` - ErrorUpdatePrepare: Update preparation failed
- `0x82` - ErrorAuthInvalid: Authentication failed
- `0x83` - ErrorWritingFlash: Flash write error
- `0x84` - ErrorUpdateEnd: Update end failed
- `0x85` - ErrorInvalidBootstrapping
- `0x86` - ErrorWrongCurrentFlashConfig
- `0x87` - ErrorWrongNewFlashConfig
- `0x88` - ErrorEsp8266NotEnoughSpace
- `0x89` - ErrorEsp32NotEnoughSpace
- `0x8A` - ErrorNoUpdatePartition
- `0x8B` - ErrorMd5Mismatch: MD5 checksum mismatch
- `0x8C` - ErrorRp2040NotEnoughSpace
- `0xFF` - ErrorUnknown: Unknown error

## Error Handling

The implementation handles various error scenarios:

1. **Connection errors**: Device unreachable, port closed
2. **Timeout errors**: Communication takes too long
3. **Protocol errors**: Invalid magic bytes, unexpected message types
4. **File errors**: Firmware file not found or unreadable

## Integration with OtaService

The `upload_firmware_ota()` method on `OtaService`:

1. Retrieves firmware from the firmware manager
2. Creates an OTA client connection to the device
3. Executes the upload protocol with authentication
4. Sends Pushover notification on success/failure (if configured)
5. Updates device state to `OtaTransmit` on success
6. Returns appropriate errors on failure

**Device State Transitions:**
```
Idle → (new firmware available) → NewVersionAvailableTransmitted
     → (device initiates update) → OtaTransmit
     → (update complete) → Idle
```

### Pushover Notifications

Optional push notifications via Pushover service:

- **Success**: Normal priority notification with device ID, IP, and firmware version
- **Failure**: High priority notification with error details
- **New Device**: Low priority info notification when new device registers

## Configuration

### YAML Configuration Example

```yaml
service:
  max_concurrent_updates: 10
  check_interval: 300         # seconds between availability checks (5 minutes)
  ota_password: "your_hex_password"  # Optional OTA authentication
  default_ota_port: 3232      # Default OTA port (devices can override)

firmware:
  storage_path: "/var/lib/ota-service/firmware"
  erase_firmware_after_upload: false  # Delete firmware file after successful upload

pushover:                      # Optional push notifications
  enabled: true
  api_token: "your_api_token"
  user_key: "your_user_key"
  device: "optional_device_name"
  priority: 0                  # -2 to 2
```

### Client Configuration

```rust
let client = OtaClient::new(device_ip, 3232)
    .with_timeout(60);  // 60-second timeout
```

## Performance Considerations

1. **Chunk size**: 1024 bytes with acknowledgments every 8192 bytes (8 chunks)
2. **Timeouts**: 30-second default on all network operations, configurable via `with_timeout()`
3. **Concurrency**: Limited by service config `max_concurrent_updates`
4. **Firmware size**: Typical firmware 500KB-2MB
5. **Network**: Big-endian size transmission, timeout protection on all reads

## Security Notes

1. **Authentication**: MD5 and SHA256 password support with hex string concatenation
2. **Checksum**: Actual MD5 validation of firmware data
3. **Network**: Assumes trusted network (no encryption at protocol level)
4. **MQTT Bridge**: Additional security layer when used with MQTT coordination
5. **Password**: Should be provided as hexadecimal string in configuration

## Debugging

Enable debug logging to see protocol details:

```rust
use log::LevelFilter;
use simple_logger::SimpleLogger;

SimpleLogger::new()
    .with_level(LevelFilter::Debug)
    .init()?;
```

Debug output will show:
- Message type transitions
- Connection attempts and timeouts
- Firmware chunk transmission progress
- State changes and errors

## Testing

Unit tests are included for:
- OTA client creation
- Message type conversion
- Timeout configuration
- Connection establishment

Run tests with:
```bash
cargo test --lib ota_client
```

## Troubleshooting

### Connection Refused
- Verify device IP address and port 3232 is open
- Check device is running ESPHome firmware
- Verify device OTA is enabled in ESPHome configuration

### Protocol Errors
- Check magic bytes validation
- Verify device ESPHome version compatibility
- Enable debug logging for detailed error messages

### Timeout Errors
- Increase timeout value for slow networks
- Check network connectivity
- Verify device is responsive

### Firmware Upload Stalls
- Check network bandwidth usage
- Verify firmware file integrity
- Consider reducing chunk size for unreliable networks

## Future Enhancements

1. **MD5 Checksum**: Implement firmware integrity verification
2. **Resume Support**: Resume interrupted uploads
3. **Progress Callbacks**: Report upload progress to application
4. **Compression**: Support firmware compression for faster uploads
5. **Multiple Concurrent Uploads**: Parallelize uploads to multiple devices
6. **HTTP Server**: Host firmware files via HTTP for device downloads
