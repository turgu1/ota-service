# OTA Service

**Over-The-Air Firmware Update Service for ESPHome Devices**

*Last updated: December 6, 2025*

A robust Rust-based service that automatically manages firmware updates for ESP32 devices running ESPHome firmware. The service monitors for new firmware versions, coordinates updates via MQTT, and uploads firmware using the ESPHome native OTA protocol v2.

> **Platform Note:** This application is built and tested for Linux platforms. It may also work on other operating systems, but Linux is the primary target environment.

## Features

✅ **ESPHome OTA Protocol v2** - Native protocol with comprehensive error handling  
✅ **MQTT Coordination** - Device registration and update notifications  
✅ **Automatic Version Management** - Detects and deploys latest firmware versions  
✅ **Device State Tracking** - SQLite database tracks device status and update progress  
✅ **Authentication Support** - MD5 and SHA256 password authentication  
✅ **Pushover Notifications** - Real-time alerts for updates, failures, and new devices  
✅ **Timeout Protection** - All network operations have configurable timeouts  
✅ **Comprehensive Error Handling** - 13 distinct error codes with descriptive messages  
✅ **Concurrent Updates** - Configurable parallel firmware uploads  
✅ **Deep Sleep Support** - Compatible with battery-powered devices  

## Quick Start

### Prerequisites

- Rust 1.70+ (2024 edition)
- MQTT broker (e.g., Mosquitto, Home Assistant)
- ESP32 devices running ESPHome firmware with OTA enabled
- Optional: Pushover account for push notifications

### Installation

```bash
# Clone the repository
git clone <repository-url>
cd ota-service

# Build the service
cargo build --release

# Copy and configure
cp config.example.yaml /etc/ota-service/config.yaml
# Edit /etc/ota-service/config.yaml with your settings
```

### Configuration

Create a `config.yaml` file:

```yaml
mqtt:
  host: "mqtt.local"
  port: 1883
  client_id: "ota-service"
  username: "ota-user"
  password: "ota-password"
  keep_alive: 60
  ota_service_topic_prefix: "ota-service/"

database:
  path: "/var/lib/ota-service/devices.db"
  pool_size: 5

service:
  name: "ota-service"
  port: 8080
  log_level: "info"
  log_file_path: "/var/log/ota-service/ota-service.log"

firmware:
  storage_path: "/var/lib/ota-service/firmware"
  max_concurrent_updates: 10
  update_timeout: 3600
  check_interval: 60
  ota_password: "your_hex_password"  # Optional OTA authentication
  default_ota_port: 3232  # Default OTA port (devices can override)
  erase_firmware_after_upload: false  # Delete firmware after successful upload

pushover:  # Optional push notifications
  enabled: true
  api_token: "your_pushover_api_token"
  user_key: "your_pushover_user_key"
  priority: 0  # -2 to 2
```

**Configuration Notes:**
- `erase_firmware_after_upload`: When set to `true`, firmware files are automatically deleted after successful upload to a device. This helps manage disk space when deploying firmware to multiple devices. Set to `false` (default) to keep firmware files for reuse or rollback purposes.

### Running the Service

```bash
# Run directly
./target/release/ota-service /etc/ota-service/config.yaml

# Or install as systemd service
sudo cp ota-service.service /etc/systemd/system/
sudo systemctl enable ota-service
sudo systemctl start ota-service
```

## How It Works

### Device Registration

Devices register themselves via MQTT by publishing to the registration topic:

```json
{
  "device_id": "esp32-001",
  "ip_address": "192.168.1.100",
  "firmware_version": "1.0.0",
  "ota_readiness_topic": "devices/esp32-001/ready",
  "ota_mode_topic": "devices/esp32-001/ota-mode",
  "uses_deep_sleep": false,
  "ota_port": 3232  // Optional: custom OTA port (uses default if omitted)
}
```

**Note on OTA Ports:**
- The `ota_port` field is optional in device registration
- If omitted, devices will use the `default_ota_port` from configuration (typically 3232 for ESPHome)
- Custom ports are useful for devices with non-standard OTA configurations or multiple devices behind NAT
- The service automatically uses the device-specific port when uploading firmware

### Firmware Update Workflow

```
1. Service periodically scans firmware directory
   └─> Finds firmware files matching pattern: "device_id - version.bin"

2. Version comparison
   └─> Compares available firmware with device's current version
   └─> Selects highest version number

3. Availability notification
   └─> Service publishes "NEW-FIRMWARE-VERSION" to device's ota_mode_topic

4. Device readiness
   └─> Device responds "OTA-READY" to ota_readiness_topic

5. OTA upload (Port 3232)
   └─> Service connects to device via ESPHome OTA protocol v2
   └─> Authenticates (if password configured)
   └─> Uploads firmware in 1024-byte chunks
   └─> Receives acknowledgment every 8192 bytes
   └─> Device validates MD5 checksum

6. Device restart
   └─> Device installs firmware and restarts
   └─> Re-registers with new version

7. Notification (if Pushover enabled)
   └─> Success or failure notification sent
```

## Firmware File Naming

Place firmware files in the configured storage directory using this naming convention:

```
<device_id> - <version>.bin
```

Examples:
```
esp32-kitchen - 1.2.3.bin
esp32-bedroom - 2.0.1.bin
esp32-garage - 1.5.0.bin
```

The service automatically selects the **highest version number** for each device.

## Deploying Firmware with deploy-firmware.sh

The `deploy-firmware.sh` script simplifies firmware deployment by automating the process of compiling ESPHome firmware and copying it to the OTA service firmware directory.

### Prerequisites

- ESPHome CLI installed (`pip install esphome`)
- ESPHome device configuration YAML files
- OTA service configured and running

### Usage

```bash
# Navigate to your ESPHome device configuration folder
cd /path/to/esphome-configs

# Run the deployment script with the device YAML file
/path/to/ota-service/deploy-firmware.sh esp32-kitchen.yaml
```

### What the Script Does

1. **Validates** the ESPHome YAML file exists in current directory
2. **Extracts** device name and version from the YAML configuration
3. **Compiles** the firmware using ESPHome CLI
4. **Reads** OTA service configuration to find firmware storage directory
5. **Copies** the compiled firmware with proper naming: `device_id - version.bin`
6. **Verifies** the deployment was successful

### Configuration

Before first use, edit the `OTA_CONFIG_FILE` path in the script:

```bash
# TODO: Adjust this path to match your OTA service configuration file location
OTA_CONFIG_FILE="/etc/ota-service/config.yaml"
```

### Example Output

```
INFO: Using OTA service config: /etc/ota-service/config.yaml
INFO: Firmware storage path: /var/lib/ota-service/firmware
INFO: Device name: esp32-kitchen
INFO: Firmware version: 1.2.3
INFO: Starting ESPHome firmware compilation...
SUCCESS: Firmware compiled successfully
INFO: Source file: /path/to/.esphome/build/esp32-kitchen/.pioenvs/esp32-kitchen/firmware.bin
INFO: Destination: /var/lib/ota-service/firmware/esp32-kitchen - 1.2.3.bin
SUCCESS: Firmware deployed successfully!
```

### Notes

- The script must be run from the directory containing your ESPHome YAML file
- Device name is extracted from the `name:` field in the YAML
- Version is extracted from the `project.version:` field in the YAML
- The script will not overwrite existing firmware files with the same version

## ESPHome Device Configuration

Your ESP32 devices must be running ESPHome with OTA enabled:

```yaml
# ESPHome configuration
wifi:
  ssid: "your-ssid"
  password: "your-password"

ota:
  password: "d96112143a8c04d8b2945b226a9b95e7"  # Must match config.yaml

mqtt:
  broker: mqtt.local
  # Device registration and readiness topics
```

## Protocol Details

### ESPHome OTA Protocol v2

- **Port**: 3232 (TCP)
- **Magic Bytes**: `[0x6C, 0x26, 0xF7, 0x5C, 0x45]`
- **Chunk Size**: 1024 bytes
- **Acknowledgments**: Every 8192 bytes (8 chunks)
- **Authentication**: MD5 or SHA256 with hex string concatenation
- **Checksum**: Actual MD5 verification of firmware data
- **Timeout**: 30 seconds (configurable) on all network operations

### Response Codes

**Success (0x40-0x47):**
- `0x40` HeaderOk, `0x41` AuthOk, `0x42` UpdatePrepareOk, `0x43` BinaryMd5Ok
- `0x44` ReceiveOk, `0x45` UpdateEndOk, `0x46` SupportsCompression, `0x47` ChunkOk

**Errors (0x80-0x8C, 0xFF):**
- `0x82` ErrorAuthInvalid (password mismatch)
- `0x8B` ErrorMd5Mismatch (checksum verification failed)
- `0x89` ErrorEsp32NotEnoughSpace (firmware too large)
- See [doc/ota/OTA_PROTOCOL.md](doc/ota/OTA_PROTOCOL.md) for complete list

## Pushover Notifications

The service sends push notifications via Pushover for:

### OTA Updates
- **Success** (Normal priority): Firmware successfully uploaded with device details
- **Failure** (High priority): Upload failed with error details

### Device Events
- **New Device** (Low priority): New device registered with IP and firmware version
- **Startup** (Low priority): Service started successfully
- **Startup Error** (High priority): Service failed to start with error details

## Database Schema

The service maintains a SQLite database tracking:
- Device ID, IP address, firmware version
- Last update timestamp
- MQTT topics (readiness, OTA mode)
- Deep sleep mode configuration
- Update state (Idle, OtaTransmit, NewVersionAvailableTransmitted)

## Project Structure

```
ota-service/
├── src/
│   ├── main.rs                  # Application entry point
│   ├── config.rs                # Configuration management
│   ├── database.rs              # SQLite device tracking
│   ├── firmware.rs              # Firmware file management
│   ├── mqtt.rs                  # MQTT message handling
│   ├── mqtt_client.rs           # MQTT client wrapper
│   ├── ota_client.rs            # ESPHome OTA protocol v2 implementation
│   ├── pushover.rs              # Pushover notification client
│   └── service.rs               # OTA service coordinator
├── doc/ota/                     # Comprehensive documentation
│   ├── README.md                # Documentation index
│   ├── OTA_PROTOCOL.md          # Protocol specification
│   ├── OTA_IMPLEMENTATION.md    # Implementation guide
│   ├── OTA_QUICK_REFERENCE.md   # Quick reference
│   └── ...
├── Cargo.toml                   # Rust dependencies
└── README.md                    # This file
```

## Documentation

Comprehensive documentation is available in the `doc/ota/` directory:

- **[doc/ota/README.md](doc/ota/README.md)** - Documentation overview and index
- **[doc/ota/OTA_QUICK_REFERENCE.md](doc/ota/OTA_QUICK_REFERENCE.md)** - Quick facts and troubleshooting
- **[doc/ota/OTA_PROTOCOL.md](doc/ota/OTA_PROTOCOL.md)** - ESPHome OTA protocol v2 details
- **[doc/ota/OTA_IMPLEMENTATION.md](doc/ota/OTA_IMPLEMENTATION.md)** - Implementation guide
- **[doc/ota/OTA_COMPLETE_WORKFLOW.md](doc/ota/OTA_COMPLETE_WORKFLOW.md)** - End-to-end workflow
- **[doc/ota/EXAMPLES.rs](doc/ota/EXAMPLES.rs)** - Code examples

## Troubleshooting

### Device not updating

```bash
# Check device is reachable
ping 192.168.1.100

# Test OTA port
nc -zv 192.168.1.100 3232

# Check service logs
journalctl -u ota-service -f

# Verify firmware file exists and naming is correct
ls -la /var/lib/ota-service/firmware/
```

### Authentication failures (0x82)

- Verify `ota_password` in config.yaml matches device OTA password
- Password must be hexadecimal string
- Check device ESPHome configuration

### MD5 checksum mismatch (0x8B)

- Firmware file may be corrupted
- Re-download or rebuild firmware
- Verify file integrity

### Connection timeouts

- Increase timeout: Default is 30 seconds
- Check network connectivity
- Device may be in deep sleep mode

See [doc/ota/OTA_QUICK_REFERENCE.md](doc/ota/OTA_QUICK_REFERENCE.md) for more troubleshooting tips.

## Development

### Building

```bash
# Debug build
cargo build

# Release build
cargo build --release

# Run tests
cargo test

# Check code
cargo clippy
```

### Dependencies

- **tokio**: Async runtime
- **rumqttc**: MQTT client
- **sqlite**: Database
- **reqwest**: HTTP client (Pushover)
- **md5**, **sha2**: Cryptographic hashing
- **serde**, **serde_json**, **serde_yaml**: Serialization
- **log**, **fern**: Logging
- **config**: Configuration management

## Contributing

Contributions are welcome! Please:

1. Fork the repository
2. Create a feature branch
3. Make your changes with tests
4. Submit a pull request

## License

[Add your license here]

## Acknowledgments

- ESPHome project for the OTA protocol specification
- Pushover for notification service

## Support

For issues and questions:
- Review documentation in `doc/ota/`
- Check logs: `journalctl -u ota-service -f`
- Enable debug logging: Set `log_level: "debug"` in config.yaml

---

**Made with ❤️ for the ESPHome community**
