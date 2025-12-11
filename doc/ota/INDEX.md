# OTA Service Master Index

Complete index of all modules, features, APIs, and documentation.

## Table of Contents

- [Modules](#modules)
- [Core APIs](#core-apis)
- [Configuration](#configuration)
- [Database Schema](#database-schema)
- [MQTT Topics](#mqtt-topics)
- [OTA Protocol](#ota-protocol)
- [Error Codes](#error-codes)
- [File Locations](#file-locations)
- [CLI Commands](#cli-commands)

## Modules

### Source Files

| Module | File | Description | Lines |
|--------|------|-------------|-------|
| **Main** | `src/main.rs` | Application entry point, logging setup | 102 |
| **Service** | `src/service.rs` | High-level OTA service orchestration | 578 |
| **OTA Client** | `src/ota_client.rs` | ESPHome OTA protocol implementation | 341 |
| **MQTT Client** | `src/mqtt_client.rs` | MQTT wrapper with async operations | 169 |
| **MQTT Coordinator** | `src/mqtt.rs` | Device registration parsing | 149 |
| **Database** | `src/database.rs` | SQLite device management with upload history | 584 |
| **Firmware Manager** | `src/firmware.rs` | Firmware file discovery and versioning | 415 |
| **Configuration** | `src/config.rs` | Configuration loading and validation | 295 |
| **Version** | `src/version.rs` | Semantic version parsing and comparison | 145 |
| **Pushover** | `src/pushover.rs` | Pushover notification client | 155 |

**Total**: 10 modules, 2,933 lines of code

### Module Dependencies

```
main.rs
  └─> service.rs
       ├─> ota_client.rs
       ├─> version.rs
       ├─> mqtt_client.rs
       ├─> mqtt.rs
       ├─> database.rs
       ├─> firmware.rs
       ├─> config.rs
       └─> pushover.rs (optional)
```

## Core APIs

### OtaService

**Location**: `src/service.rs`

#### Public Methods

```rust
pub struct OtaService {
    database: Arc<Mutex<Database>>,
    mqtt_client: Arc<Mutex<MqttClient>>,
    firmware_manager: Arc<FirmwareManager>,
    registration_topic: String,
    ota_password: String,
    default_ota_port: u16,
    erase_firmware_after_upload: bool,
    pushover_client: Option<Arc<PushoverClient>>,
}

impl OtaService {
    /// Create new OTA service instance
    pub async fn new(config: Configuration) -> Result<Arc<Self>, String>
    
    /// Start the service (firmware checks + MQTT listener)
    pub async fn run(self: Arc<Self>) -> Result<(), String>
    
    /// Start firmware check loop
    pub async fn start_firmware_check_loop(&self, interval_secs: u64)
    
    /// Start unified MQTT message listener
    pub async fn start_mqtt_listener(&self)
    
    /// Check all devices for newer firmware
    async fn check_all_devices_for_updates(&self) -> Result<(), String>
    
    /// Notify device about available update
    async fn notify_device_update_available(
        &self,
        device_id: &str,
        current_version: &str,
        new_version: &str,
        ota_mode_topic: &str,
    ) -> Result<(), String>
}
```

### OtaClient

**Location**: `src/ota_client.rs`

#### Public Methods

```rust
pub struct OtaClient {
    stream: TcpStream,
    supports_compression: bool,
}

impl OtaClient {
    /// Connect to device and perform handshake
    pub fn connect(ip: &str, port: u16, password: String) -> Result<Self, String>
    
    /// Perform complete OTA update
    pub fn update(&mut self, firmware: &[u8]) -> Result<(), String>
    
    // Private protocol methods
    fn send_hello(&mut self) -> Result<(), String>
    fn receive_hello(&mut self) -> Result<u8, String>
    fn authenticate(&mut self, password: &str, nonce: &[u8]) -> Result<(), String>
    fn send_update_start(&mut self, firmware_size: u32) -> Result<(), String>
    fn send_firmware_data(&mut self, firmware: &[u8]) -> Result<(), String>
    fn send_update_end(&mut self, firmware: &[u8]) -> Result<(), String>
    fn wait_ack(&mut self) -> Result<(), String>
}
```

### Database

**Location**: `src/database.rs`

#### Public Methods

```rust
pub struct Database {
    conn: Connection,
}

impl Database {
    /// Create/open database
    pub fn new(db_path: &str) -> Result<Self, String>
    
    /// Insert or update device
    pub async fn upsert_device(&mut self, device: &Device) -> Result<(), String>
    
    /// Get single device
    pub async fn get_device(&self, device_id: &str) -> Result<Option<Device>, String>
    
    /// Get all devices
    pub async fn get_all_devices(&self) -> Result<Vec<Device>, String>
    
    /// Update device firmware version
    pub async fn update_device_firmware_version(
        &mut self,
        device_id: &str,
        firmware_version: &str,
    ) -> Result<(), String>
    
    /// Update device state
    pub async fn update_device_state(
        &mut self,
        device_id: &str,
        device_state: DeviceState,
    ) -> Result<(), String>
    
    /// Delete device
    pub async fn delete_device(&mut self, device_id: &str) -> Result<(), String>
    
    /// Add upload history record
    pub fn add_upload_history(
        &mut self,
        device_id: &str,
        version: &str,
        success: bool,
    ) -> Result<(), String>
    
    /// Get upload history for a device
    pub fn get_upload_history(
        &self,
        device_id: &str,
    ) -> Result<Vec<(String, String, String)>, String>  // (version, state, attempted_at)
    
    /// Get all upload history
    pub fn get_all_upload_history(&self) -> Result<Vec<(String, String, String, String)>, String>  // (device_id, version, state, attempted_at)
}
```

### FirmwareManager

**Location**: `src/firmware.rs`

#### Public Methods

```rust
pub struct FirmwareManager {
    firmware_dir: String,
}

impl FirmwareManager {
    /// Create firmware manager
    pub fn new(firmware_dir: &str) -> Self
    
    /// Get all firmware files for device
    pub fn get_firmware_for_device(&self, device_id: &str) -> Result<Vec<FirmwareInfo>, String>
    
    /// Find newer firmware version
    pub fn get_newer_version(
        &self,
        device_id: &str,
        current_version: &str,
    ) -> Result<Option<FirmwareInfo>, String>
    
    /// Read firmware binary
    pub fn read_firmware(&self, firmware_info: &FirmwareInfo) -> Result<Vec<u8>, String>
    
    /// Delete firmware file
    pub fn delete_firmware(&self, firmware_info: &FirmwareInfo) -> Result<(), String>
    
    /// List all firmware files
    pub fn list_all_firmware(&self) -> Result<Vec<FirmwareInfo>, String>
    
    /// Delete firmware and all older versions for a device
    pub fn delete_firmware_and_older_versions(
        &self,
        device_id: &str,
        current_version: &str,
    ) -> Result<usize, String>
}
```

### Version

**Location**: `src/version.rs`

#### Public Methods

```rust
pub struct Version {
    pub major: u32,
    pub minor: u32,
    pub patch: u32,
}

impl Version {
    /// Parse version string "major.minor.patch"
    pub fn parse(version_str: &str) -> Result<Self, String>
    
    /// Compare this version with another (returns Ordering)
    pub fn compare(&self, other: &Version) -> Ordering
    
    /// Check if this version is greater than another
    pub fn is_greater_than(&self, other: &Version) -> bool
    
    /// Check if this version is less than another
    pub fn is_less_than(&self, other: &Version) -> bool
    
    /// Check if this version is less than or equal to another
    pub fn is_less_or_equal(&self, other: &Version) -> bool
}
```

**Note**: Version comparison is numeric, not string-based. For example, "1.10.0" > "1.9.0" (correct numeric comparison), whereas string comparison would incorrectly treat "1.10.0" < "1.9.0".

### MqttClient

**Location**: `src/mqtt_client.rs`

#### Public Methods

```rust
pub struct MqttClient {
    client: AsyncClient,
    eventloop: Arc<Mutex<Option<EventLoop>>>,
}

impl MqttClient {
    /// Create new MQTT client
    pub fn new(
        host: &str,
        port: u16,
        client_id: &str,
        username: Option<&str>,
        password: Option<&str>,
        keep_alive: u64,
    ) -> Result<Self, String>
    
    /// Wait for connection
    pub async fn wait_connected(&self, max_attempts: u32) -> Result<(), String>
    
    /// Subscribe to topic
    pub async fn subscribe(&self, topic: &str, qos: QoS) -> Result<(), String>
    
    /// Publish message
    pub async fn publish(
        &self,
        topic: &str,
        payload: &str,
        qos: QoS,
        retain: bool,
    ) -> Result<(), String>
    
    /// Clear retained message
    pub async fn clear_retained(&self, topic: &str) -> Result<(), String>
    
    /// Get next message
    pub async fn next_message(&mut self) -> Option<MqttMessage>
}
```

### Configuration

**Location**: `src/config.rs`

#### Public Methods

```rust
pub struct Configuration {
    pub mqtt: MqttConfig,
    pub database: DatabaseConfig,
    pub service: ServiceConfig,
    pub firmware: FirmwareConfig,
    pub pushover: Option<PushoverConfig>,
}

impl Configuration {
    /// Load configuration from file
    pub fn from_file(path: &str) -> Result<Self, String>
    
    /// Validate configuration
    pub fn validate(&self) -> Result<(), String>
}
```

## Configuration

### Configuration File Format

**File**: `ota_config.yml`

```yaml
mqtt:
  host: "localhost"
  port: 1883
  client_id: "ota-service"
  username: "mqtt_user"
  password: "mqtt_pass"
  keep_alive: 60
  registration_topic: "home/ota/registration"

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
  check_interval: 300  # seconds
  ota_password: null  # or "secure123"
  default_ota_port: 3232
  erase_firmware_after_upload: false

pushover:
  enabled: true
  api_token: "your-pushover-app-token"
  user_key: "your-pushover-user-key"
  priority: 0
```

### Environment Variables

| Variable | Description | Default |
|----------|-------------|---------|
| `OTA_CONFIG_PATH` | Path to configuration file | `./ota_config.yml` |
| `RUST_LOG` | Logging level | `info` |

## Database Schema

### Tables

#### devices

```sql
CREATE TABLE devices (
    device_id TEXT PRIMARY KEY,
    ip_address TEXT NOT NULL,
    firmware_version TEXT NOT NULL,
    last_updated TEXT NOT NULL,
    ota_readiness_topic TEXT NOT NULL,
    ota_mode_topic TEXT NOT NULL,
    uses_deep_sleep INTEGER NOT NULL,
    ota_port INTEGER,
    state TEXT NOT NULL DEFAULT 'idle'
);
```

#### upload_history

```sql
CREATE TABLE upload_history (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    device_id TEXT NOT NULL,
    version TEXT NOT NULL,
    state TEXT NOT NULL CHECK(state IN ('SUCCESS', 'FAIL')),
    attempted_at TEXT NOT NULL DEFAULT CURRENT_TIMESTAMP
);
```

### Enums

#### DeviceState

```rust
pub enum DeviceState {
    Idle,                               // No update in progress
    NewVersionAvailableTransmitted,     // Device notified of update
    OtaTransmit,                        // Update in progress
}
```

**Database Values**: `"idle"`, `"new_version_available_transmitted"`, `"ota_transmit"`

## MQTT Topics

### Registration Topic

**Topic**: `home/ota/registration` (configurable)

**Payload** (JSON):
```json
{
  "device_id": "esp32-001",
  "ip_address": "192.168.1.100",
  "mac_address": "AA:BB:CC:DD:EE:FF",
  "firmware_version": "1.0.0",
  "ota_port": 3232,
  "ota_readiness_topic": "home/esp32-001/ota/ready",
  "ota_mode_topic": "home/esp32-001/ota/mode"
}
```

### Per-Device Topics

#### OTA Mode Topic

**Topic**: `home/{device_id}/ota/mode`

**Service → Device**:
- `"ON"` - Update available, enter OTA mode
- `""` (empty, retained) - Clear notification

#### OTA Readiness Topic

**Topic**: `home/{device_id}/ota/ready`

**Device → Service**:
- `"OTA-READY"` - Ready to receive update

**Service → Device**:
- `""` (empty, retained) - Clear after update

## OTA Protocol

### Protocol Version

**Version**: 2 (ESPHome OTA Protocol v2)

### Connection

**Transport**: TCP  
**Port**: 3232 (default, configurable per device)

### Message Format

All multi-byte integers are **little-endian**.

### Commands

| Command | Code | Direction | Description |
|---------|------|-----------|-------------|
| `OK` | `0x00` | ← Device | Success acknowledgment |
| `AUTH` | `0x01` | → Device | Authentication hash |
| `UPDATE_START` | `0x64` | → Device | Begin update (firmware size) |
| `UPDATE_DATA` | `0x65` | → Device | Firmware chunk |
| `UPDATE_END` | `0x66` | → Device | End update (MD5 hash) |

### Hello Handshake

**Service → Device**:
```
Offset  Size  Field           Value
0       5     magic           [0x6C, 0x26, 0xF7, 0x5C, 0x45]
5       1     version         0x02
6       1     reserved        0x00
```

**Device → Service**:
```
Offset  Size  Field           Value
0       5     magic           [0x6C, 0x26, 0xF7, 0x5C, 0x45]
5       1     version         0x02
6       1     features        Bit 0: Compression support
7       32    nonce           Random bytes (for auth)
```

### Authentication

**If password configured**:

```rust
let auth = SHA256(password + nonce)
```

Send: `[0x01, auth[0..32]]`  
Receive: `[0x00]` (OK)

### Update Sequence

1. **UPDATE_START**: `[0x64, size[0..4]]`
   - Response: `[0x00]`

2. **UPDATE_DATA** (repeated): `[0x65, chunk[0..1024]]`
   - Response: `[0x00]` after each chunk

3. **UPDATE_END**: `[0x66, md5[0..16]]`
   - Response: `[0x00]` if MD5 valid

## Error Codes

### OTA Protocol Errors

| Code | Name | Description |
|------|------|-------------|
| `0x00` | `OK` | Success |
| `0x80` | `ERROR_MAGIC` | Invalid magic bytes |
| `0x81` | `ERROR_INVALID_HASH` | MD5/SHA256 validation failed |
| `0x82` | `ERROR_UPDATE_PREPARE` | Cannot prepare for update |
| `0x83` | `ERROR_AUTH_INVALID` | Authentication failed |
| `0x84` | `ERROR_WRITING_FLASH` | Flash write error |
| `0x85` | `ERROR_UPDATE_END` | Cannot finalize update |
| `0x86` | `ERROR_UNKNOWN` | Unknown error |

**See**: [OTA_PROTOCOL.md](OTA_PROTOCOL.md) for complete list

## File Locations

### Runtime Directories

| Path | Purpose |
|------|---------|
| `/var/lib/ota-service/` | Service data directory |
| `/var/lib/ota-service/firmware/` | Firmware binary storage |
| `/var/lib/ota-service/ota.db` | SQLite database |
| `/var/log/ota-service.log` | Application log file |

### Firmware File Naming

**Format**: `{device_id} - {version}.bin`

**Examples**:
- `esp32-001 - 1.0.0.bin`
- `esp32-prod - 2.1.3.bin`
- `sensor-kitchen - 1.2.0.bin`

**Version Format**: Semantic versioning (MAJOR.MINOR.PATCH)

## CLI Commands

### Service Management

```bash
# Start service
sudo systemctl start ota-service

# Stop service
sudo systemctl stop ota-service

# Restart service
sudo systemctl restart ota-service

# View status
sudo systemctl status ota-service

# Enable auto-start
sudo systemctl enable ota-service

# View logs
journalctl -u ota-service -f
```

### Database Queries

```bash
# Connect to database
sqlite3 /var/lib/ota-service/ota.db

# List all devices
SELECT device_id, firmware_version, device_state FROM devices;

# Check device state
SELECT * FROM devices WHERE device_id = 'esp32-001';

# Update device state (manual)
UPDATE devices SET device_state = 'Idle' WHERE device_id = 'esp32-001';

# Delete device
DELETE FROM devices WHERE device_id = 'esp32-001';
```

### Firmware Management

```bash
# List firmware files
ls -lh /var/lib/ota-service/firmware/

# Deploy new firmware
cp new-firmware.bin /var/lib/ota-service/firmware/esp32-001-2.0.0.bin

# Check firmware permissions
ls -la /var/lib/ota-service/firmware/

# Verify firmware size
du -h /var/lib/ota-service/firmware/*.bin

# Remove old firmware
rm /var/lib/ota-service/firmware/esp32-001-1.0.0.bin
```

### MQTT Testing

```bash
# Subscribe to all OTA topics
mosquitto_sub -h localhost -t "home/+/ota/#" -v

# Subscribe to registration
mosquitto_sub -h localhost -t "home/ota/registration" -v

# Manual registration trigger (testing)
mosquitto_pub -h localhost -t "home/ota/registration" -m '{
  "device_id": "esp32-test",
  "ip_address": "192.168.1.99",
  "mac_address": "AA:BB:CC:DD:EE:FF",
  "firmware_version": "1.0.0",
  "ota_port": 3232,
  "ota_readiness_topic": "home/esp32-test/ota/ready",
  "ota_mode_topic": "home/esp32-test/ota/mode"
}'

# Clear retained messages
mosquitto_pub -h localhost -t "home/esp32-001/ota/mode" -r -n
mosquitto_pub -h localhost -t "home/esp32-001/ota/ready" -r -n
```

### Debugging

```bash
# Check configuration
cat /etc/ota-service/ota_config.yml

# Validate configuration (requires custom tool)
ota-service --validate-config

# Test device connectivity
ping 192.168.1.100
nc -zv 192.168.1.100 3232

# Monitor network traffic
sudo tcpdump -i any port 3232 -A

# Check file descriptors
lsof -p $(pgrep ota-service)

# Memory usage
ps aux | grep ota-service
```

## Quick Reference Links

### Documentation

- [README.md](README.md) - Documentation overview
- [OTA_QUICK_REFERENCE.md](OTA_QUICK_REFERENCE.md) - Quick facts and troubleshooting
- [README_OTA.md](README_OTA.md) - Complete architecture guide
- [OTA_PROTOCOL.md](OTA_PROTOCOL.md) - Protocol specification
- [OTA_IMPLEMENTATION.md](OTA_IMPLEMENTATION.md) - Implementation guide
- [OTA_COMPLETE_WORKFLOW.md](OTA_COMPLETE_WORKFLOW.md) - End-to-end workflow
- [EXAMPLES.rs](EXAMPLES.rs) - Code examples

### External Resources

- [ESPHome OTA Documentation](https://esphome.io/components/ota.html)
- [MQTT Protocol](https://mqtt.org/)
- [Tokio Async Runtime](https://tokio.rs/)
- [rumqttc MQTT Client](https://github.com/bytebeamio/rumqtt)

## Version History

### Current Version

**Version**: 1.0.0  
**Last Updated**: December 2024  
**Status**: Production Ready

### Module Versions

| Module | Version | Status |
|--------|---------|--------|
| OTA Protocol | v2 | Stable |
| MQTT | 3.1.1/5.0 | Stable |
| Database Schema | 1.0 | Stable |
| Configuration | 1.0 | Stable |

## Support

For issues, questions, or contributions:

1. Check [OTA_QUICK_REFERENCE.md](OTA_QUICK_REFERENCE.md) troubleshooting section
2. Review logs: `journalctl -u ota-service -f`
3. Verify configuration: `/etc/ota-service/ota_config.yml`
4. Check database state: `sqlite3 /var/lib/ota-service/ota.db`

---

**Index Version**: 1.0.0  
**Generated**: December 2024  
**Maintainer**: OTA Service Team
