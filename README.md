# OTA Service for ESPHome devices

**Over-The-Air Firmware Update Service for ESPHome Devices**

*Last updated: December 15, 2025*

A robust Rust-based service that automatically manages firmware updates for devices running ESPHome firmware. The service monitors for new firmware versions, coordinates updates via MQTT, and uploads firmware using the ESPHome native OTA protocol v2.

> **Platform Note:** This application is built and tested for Linux platforms. It may also work on other operating systems, but Linux is the primary target environment.

> **Device Note:** All testing has been done with ESP32 devices. Please advise if the other kind of devices supported by ESPHome OTA capability are not working properly.

## Features

✅ **Web-Based Monitoring** - Real-time device dashboard with sortable tables and configuration management  
✅ **ESPHome OTA Protocol v2** - Native protocol with comprehensive error handling  
✅ **MQTT Coordination** - Device registration and update notifications  
✅ **Automatic Version Management** - Detects and deploys latest firmware versions  
✅ **Device State Tracking** - SQLite database tracks device status and update progress  
✅ **Authentication Support** - MD5 and SHA256 password authentication  
✅ **Pushover Notifications** - Real-time alerts for updates, failures, and new devices  
✅ **Home Assistant Integration** - Uses the MQTT automatic discovery feature to have it's state on Home Assistant  
✅ **Timeout Protection** - All network operations have configurable timeouts  
✅ **Comprehensive Error Handling** - 13 distinct error codes with descriptive messages  
✅ **Concurrent Updates** - Configurable parallel firmware uploads  
✅ **Deep Sleep Support** - Compatible with battery-powered devices  

## Quick Start

### Prerequisites

- Rust 1.70+ (2024 edition)
- MQTT broker (e.g., Mosquitto, Home Assistant)
- Devices running ESPHome firmware with OTA enabled
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

database:
  path: "/var/lib/ota-service/devices.db"
  pool_size: 5

service:
  name: "ota-service"
  log_level: "info"
  log_file_path: "/var/log/ota-service/ota-service.log"

firmware:
  storage_path: "/var/lib/ota-service/firmware"
  max_concurrent_updates: 10
  update_timeout: 3600
  check_interval: 300  # 5 minutes
  ota_password: "your_hex_password"  # Optional OTA authentication
  default_ota_port: 3232  # Default OTA port (devices can override)
  erase_firmware_after_upload: false  # Delete firmware after successful upload

pushover:  # Optional push notifications
  enabled: false
  api_token: "your_pushover_api_token"
  user_key: "your_pushover_user_key"
  device: "iphone"  # Optional: specific device name
  priority: 0  # -2 to 2

home_assistant:  # Optional Home Assistant MQTT discovery
  enabled: false
  discovery_prefix: "homeassistant"
  node_id: "ota_service"
  device_name: "OTA Service"
  manufacturer: "ESPHome OTA Service"
  model: "Firmware Update Manager"
  update_interval: 60  # seconds

web:
  port: 8080
  username: "admin"
  password: "admin"
  refresh_period: 5
  edit_session_timeout: 15
```

**Configuration Notes:**
- `erase_firmware_after_upload`: When set to `true`, the uploaded firmware file and all older versions for the same device are automatically deleted after successful upload. This helps manage disk space when deploying firmware to multiple devices. Set to `false` (default) to keep firmware files for reuse or rollback purposes.

### Running the Service

```bash
# Run directly
./target/release/ota-service /etc/ota-service/config.yaml

# Or install as systemd service
sudo cp ota-service.service /etc/systemd/system/
sudo systemctl enable ota-service
sudo systemctl start ota-service
```

## Web Interface

The OTA Service includes a comprehensive web-based monitoring and management interface for:

- **Real-time device monitoring** - WebSocket-based live updates of all registered devices
- **Firmware update history** - Track all upload attempts with success/failure status
- **Configuration management** - View and edit service configuration through the web UI
- **Service restart** - Restart the service directly from the interface

### Access the Web Interface

1. Configure the web server in `config.yaml`:
   ```yaml
   web:
     port: 8080
     username: "admin"
     password: "change_this_password"  # Change in production!
     refresh_period: 5
     edit_session_timeout: 15
   ```

2. Open your browser and navigate to: `http://localhost:8080`

3. Log in with your configured credentials

### Key Features

**Devices Tab:**
- Live device table with auto-refresh
- Sortable and resizable columns
- Color-coded status indicators (Idle, OTA in Progress, Update Available)
- WiFi signal strength (RSSI) with visual indicators
- Firmware version with semantic sorting

**Update Log Tab:**
- Complete history of firmware uploads
- Success/failure status with detailed error messages
- Sortable by device, version, status, or timestamp

**Config Tab:**
- View all configuration settings
- Edit configuration values with password protection
- Sensitive data automatically masked (passwords, API tokens, IP addresses)
- Changes saved to config file and require restart

**See [doc/WEB_INTERFACE.md](doc/WEB_INTERFACE.md) for complete documentation.**

## How It Works

### Device Registration

Devices register themselves via MQTT by publishing to the registration topic:

```json
{
  "device_id": "device-001",
  "ip_address": "192.168.1.100",
  "firmware_version": "1.0.0",
  "ota_readiness_topic": "devices/device-001/ready",
  "ota_mode_topic": "devices/device-001/ota-mode",
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
device-kitchen - 1.2.3.bin
device-bedroom - 2.0.1.bin
device-garage - 1.5.0.bin
```

The service automatically selects the **highest version number** for each device.

## Deploying Firmware with deploy-device-firmware.sh

The `deploy-device-firmware.sh` script simplifies firmware deployment by automating the process of compiling ESPHome firmware and copying it to the OTA service firmware directory.

### Prerequisites

- ESPHome CLI installed (`pip install esphome`)
- ESPHome device configuration YAML files
- OTA service configured and running
- **User must be member of `ota-service` group** (for daemon installations)

### User Group Membership

If the OTA service is installed as a daemon, the firmware folder has restricted permissions (770). Users deploying firmware must be added to the `ota-service` group:

```bash
# Add your user to the ota-service group
sudo usermod -a -G ota-service $USER

# Log out and back in for group change to take effect

# Verify group membership
groups
```

### Usage

```bash
# Navigate to your ESPHome device configuration folder
cd /path/to/esphome-configs

# Run the deployment script with the device YAML file
/path/to/ota-service/deploy-firmware.sh device-kitchen.yaml
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
INFO: Device name: device-kitchen
INFO: Firmware version: 1.2.3
INFO: Starting ESPHome firmware compilation...
SUCCESS: Firmware compiled successfully
INFO: Source file: /path/to/.esphome/build/device-kitchen/.pioenvs/device-kitchen/firmware.bin
INFO: Destination: /var/lib/ota-service/firmware/device-kitchen - 1.2.3.bin
SUCCESS: Firmware deployed successfully!
```

### Notes

- The script must be run from the directory containing your ESPHome YAML file
- Device name is extracted from the `name:` substitution field in the YAML
- Device id is extracted from the 'device_id:' substitution field in the YAML
- Version is extracted from the `firmware_version:` substitution field in the YAML
- The script will not overwrite existing firmware files with the same version

## ESPHome Device Configuration

Your devices must be running ESPHome with OTA enabled and proper MQTT integration. The MQTT configuration is **required** for the OTA service to communicate with devices.

### Required MQTT Topics

The OTA service uses MQTT for coordination with devices:

1. **Registration Topic** (`ota-service/registration`): Devices publish to this topic to register with the service
2. **OTA Mode Topic** (`devices/<device_id>/ota-mode`): Service publishes `NEW-FIRMWARE-VERSION` to notify device
3. **OTA Readiness Topic** (`devices/<device_id>/ready`): Device publishes `OTA-READY` when ready for firmware upload

### Complete ESPHome Configuration Example

```yaml
# ESPHome configuration with OTA service integration
substitutions:
  device_id: device-kitchen
  firmware_version: "1.0.0"  # Update this when deploying new firmware
  ota_password: "d96112143a8c04d8b2945b226a9b95e7"  # Must match OTA service config

esphome:
  name: ${device_id}
  platform: ESP32
  board: esp32dev
  
  # Register device on boot
  on_boot:
    priority: -100  # Run after WiFi and MQTT are connected
    then:
      - delay: 5s  # Wait for MQTT connection to stabilize
      - script.execute: register_device

wifi:
  ssid: "your-ssid"
  password: "your-password"

# Enable OTA updates via ESPHome protocol v2
# This allows the OTA service to upload firmware over port 3232
ota:
  password: ${ota_password}  # Must match ota_password in OTA service config.yaml
  port: 3232                  # Default ESPHome OTA port

# MQTT configuration - REQUIRED for OTA service integration
mqtt:
  broker: mqtt.local
  port: 1883
  username: "mqtt_user"     # Optional
  password: "mqtt_password" # Optional
  client_id: ${device_id}
  
  # Subscribe to OTA mode topic
  # Service publishes NEW-FIRMWARE-VERSION here to notify device
  on_message:
    - topic: devices/${device_id}/ota-mode
      payload: 'NEW-FIRMWARE-VERSION'
      then:
        - logger.log: "New firmware available, preparing for OTA update"
        # Respond immediately that device is ready
        - mqtt.publish:
            topic: devices/${device_id}/ready
            payload: "OTA-READY"
            retain: false
            qos: 2

# Text sensor for IP address (needed for registration)
text_sensor:
  - platform: wifi_info
    ip_address:
      id: device_ip

# Script to register device with OTA service
script:
  - id: register_device
    then:
      - wait_until:
          mqtt.connected:
      - delay: 2s
      - lambda: |-
          // Build registration JSON with all required fields
          std::string ip = id(device_ip).state.c_str();
          std::string mac = wifi::global_wifi_component->get_mac_address_pretty();
          int rssi = wifi::global_wifi_component->wifi_rssi();
          
          std::string registration = "{";
          registration += "\"device_id\":\"${device_id}\",";
          registration += "\"ip_address\":\"" + ip + "\",";
          registration += "\"mac_address\":\"" + mac + "\",";
          registration += "\"firmware_version\":\"${firmware_version}\",";
          registration += "\"ota_readiness_topic\":\"devices/${device_id}/ready\",";
          registration += "\"ota_mode_topic\":\"devices/${device_id}/ota-mode\",";
          registration += "\"uses_deep_sleep\":false,";
          registration += "\"ota_port\":3232,";  // Optional: omit to use default from config
          registration += "\"rssi\":" + std::to_string(rssi);
          registration += "}";
          
          // Publish registration to OTA service
          // Topic must match registration_topic in OTA service config.yaml
          id(mqtt_client).publish("ota-service/registration", registration);
          
          ESP_LOGI("ota", "Device registered with OTA service");
```

### MQTT Message Flow

**1. Device Registration (Device → Service)**
```json
Topic: ota-service/registration
Payload: {
  "device_id": "device-kitchen",
  "ip_address": "192.168.1.100",
  "mac_address": "AA:BB:CC:DD:EE:FF",
  "firmware_version": "1.0.0",
  "ota_readiness_topic": "devices/device-kitchen/ready",
  "ota_mode_topic": "devices/device-kitchen/ota-mode",
  "uses_deep_sleep": false,
  "ota_port": 3232,
  "rssi": -45
}
```

**2. Update Notification (Service → Device)**
```
Topic: devices/device-kitchen/ota-mode
Payload: NEW-FIRMWARE-VERSION
```

**3. Readiness Confirmation (Device → Service)**
```
Topic: devices/device-kitchen/ready
Payload: OTA-READY
```

**4. Firmware Upload (Service → Device)**
- Service connects directly to device IP address on port 3232
- Uses ESPHome OTA Protocol v2 over TCP (not MQTT)
- See [doc/ota/OTA_PROTOCOL.md](doc/ota/OTA_PROTOCOL.md) for protocol details

### Configuration Notes

- **Registration topic** must match `mqtt.registration_topic` in OTA service config.yaml (default: `ota-service/registration`)
- **Device topics** use pattern `devices/<device_id>/ota-mode` and `devices/<device_id>/ready`
- **OTA password** must be identical in both ESPHome and OTA service configurations
- **OTA port** defaults to 3232; can be customized per-device if needed
- Devices should re-register after reboot to update IP address and firmware version

For deep-sleep devices, see [esphome-examples/esp32-deep-sleep.yaml](esphome-examples/esp32-deep-sleep.yaml) for configuration that handles intermittent connectivity.

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
- `0x89` ErrorNotEnoughSpace (firmware too large)
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

See [PUSHOVER.md](PUSHOVER.md) for complete notification documentation.

## Home Assistant Integration

The OTA Service can automatically register itself in Home Assistant using MQTT discovery:

### Features
- **Automatic Device Creation** - No manual configuration needed
- **Real-time Monitoring** - Device count, updates available, active updates
- **Service Status** - Binary sensor shows if service is online
- **Dashboard Ready** - Create cards to monitor firmware deployment

### Quick Setup

Add to your `config.yaml`:

```yaml
home_assistant:
  enabled: true
  discovery_prefix: "homeassistant"
  node_id: "ota_service"
  device_name: "OTA Service"
  update_interval: 60
```

### Available Sensors

Once enabled, the OTA Service appears as a device with:
- Device Count sensor
- Updates Available sensor
- Devices Updating sensor
- Last Check timestamp
- Successful Updates counter (since restart)
- Failed Updates counter (since restart)
- Service Status binary sensor

See [doc/HOME_ASSISTANT.md](doc/HOME_ASSISTANT.md) for complete integration guide, dashboard examples, and troubleshooting.

## Database Schema

The service maintains a SQLite database with two tables:

### devices table
- Device ID (primary key), IP address, MAC address, firmware version
- Last update timestamp
- MQTT topics (readiness, OTA mode)
- Deep sleep mode configuration
- OTA port (optional, uses default if not specified)
- Update state (idle, new_version_available_transmitted, ota_transmit)
- Statistics: fail_count (failed OTA attempts), update_count (successful OTA updates)
- WiFi signal strength (rssi) in dBm

### upload_history table
- Upload attempt records with SUCCESS/FAIL state
- Device ID, version, attempt timestamp
- Used for tracking firmware deployment history

## Project Structure

```
ota-service/
├── src/
│   ├── main.rs                  # Application entry point
│   ├── config.rs                # Configuration management
│   ├── database.rs              # SQLite device tracking
│   ├── firmware.rs              # Firmware file management
│   ├── home_assistant.rs        # Home Assistant MQTT discovery
│   ├── mqtt.rs                  # MQTT message handling
│   ├── mqtt_client.rs           # MQTT client wrapper
│   ├── ota_client.rs            # ESPHome OTA protocol v2 implementation
│   ├── pushover.rs              # Pushover notification client
│   ├── service.rs               # OTA service coordinator
│   └── web.rs                   # Web server and API
├── static/
│   └── index.html               # Web interface
├── doc/
│   ├── HOME_ASSISTANT.md        # Home Assistant integration guide
│   ├── WEB_INTERFACE.md         # Web interface documentation
│   ├── SERVICE_INSTALL.md       # Installation guide
│   └── ota/                     # OTA protocol documentation
│       ├── README.md            # Documentation index
│       ├── OTA_PROTOCOL.md      # Protocol specification
│       ├── OTA_IMPLEMENTATION.md # Implementation guide
│       └── ...
├── Cargo.toml                   # Rust dependencies
└── README.md                    # This file
```

## Documentation

Comprehensive documentation is available in the `doc/` directory:

**Installation & Configuration:**
- **[doc/SERVICE_INSTALL.md](doc/SERVICE_INSTALL.md)** - Complete installation guide for Linux systemd

**Web Interface:**
- **[doc/WEB_INTERFACE.md](doc/WEB_INTERFACE.md)** - Web interface features and usage guide

**Integrations:**
- **[doc/HOME_ASSISTANT.md](doc/HOME_ASSISTANT.md)** - Home Assistant MQTT discovery integration
- **[PUSHOVER.md](PUSHOVER.md)** - Push notification configuration

**OTA Protocol:**
- **[doc/ota/README.md](doc/ota/README.md)** - Documentation overview and index
- **[doc/ota/OTA_QUICK_REFERENCE.md](doc/ota/OTA_QUICK_REFERENCE.md)** - Quick facts and troubleshooting
- **[doc/ota/OTA_PROTOCOL.md](doc/ota/OTA_PROTOCOL.md)** - ESPHome OTA protocol v2 details
- **[doc/ota/OTA_IMPLEMENTATION.md](doc/ota/OTA_IMPLEMENTATION.md)** - Implementation guide
- **[doc/ota/OTA_COMPLETE_WORKFLOW.md](doc/ota/OTA_COMPLETE_WORKFLOW.md)** - End-to-end workflow
- **[doc/ota/EXAMPLES.rs](doc/ota/EXAMPLES.rs)** - Code examples

**Notifications:**
- **[doc/PUSHOVER.md](doc/PUSHOVER.md)** - Pushover notification setup

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

This is free software released under a dual license. You may use either the MIT license or the GNU General Public License v3 (GPLv3) at your convenience. See the LICENSE.md files in the main folder.

## Acknowledgments

- ESPHome project for the OTA protocol specification
- Pushover for notification service
- Claude Sonnet V4.5 in support of this development

## Support

For issues and questions:
- Review documentation in `doc/ota/`
- Check logs: `journalctl -u ota-service -f`
- Enable debug logging: Set `log_level: "debug"` in config.yaml

---

**Made with ❤️ for the ESPHome community**

If you find this project helpful, consider buying me a coffee! ☕

[![Buy Me a Coffee](https://img.shields.io/badge/Buy%20Me%20a%20Coffee-FFDD00?style=for-the-badge&logo=buy-me-a-coffee&logoColor=black)](https://paypal.me/turgu1)
