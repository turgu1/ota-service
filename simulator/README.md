# OTA Service Device Simulator

A comprehensive simulator for testing the OTA service by simulating multiple ESP32 devices with deep sleep behavior and OTA update capabilities.

## Features

- **Multi-Device Simulation**: Simulate up to 100 concurrent ESP32 devices
- **Deep Sleep Behavior**: Devices simulate ESP32 deep sleep with configurable wake/sleep intervals
- **Full OTA Protocol**: Complete ESPHome OTA protocol v2 implementation
- **Automatic Firmware Generation**: Generates fake firmware files at random intervals
- **MQTT Communication**: Full device registration and update notification workflow
- **Configurable Ports**: Each device gets unique OTA port (base_port + device_number)
- **Realistic Timing**: Random sleep/wake cycles and firmware generation intervals

## Building

```bash
cd simulator
cargo build --release
```

## Configuration

Create a `config.yaml` file (see `config.example.yaml`):

```yaml
mqtt:
  host: localhost
  port: 1883
  username: null  # Optional
  password: null  # Optional
  keep_alive: 60

simulator:
  num_devices: 10  # Number of devices (1-100)
  device_id_prefix: "sim-esp32-"  # Device ID prefix
  base_ota_port: 8266  # Base OTA port (each device gets base + number)
  log_level: "info"  # debug, info, warn, error

firmware:
  ota_password: null  # Optional OTA password
  storage_path: "../test-data/firmware"  # Firmware storage path
  generation_interval_min: 300  # Min seconds between firmware generation (5 min)
  generation_interval_max: 600  # Max seconds between firmware generation (10 min)
  initial_version: "1.0.0"  # Starting version for all devices

deep_sleep:
  min_sleep_seconds: 30   # Minimum deep sleep duration
  max_sleep_seconds: 200  # Maximum deep sleep duration
  max_wakeup_seconds: 30  # Maximum wakeup duration

registration:
  topic_prefix: "devices/"  # MQTT topic prefix
```

## Running

```bash
# Using default config.yaml
./target/release/simulator config.yaml

# Or specify custom config
./target/release/simulator /path/to/custom-config.yaml
```

## How It Works

### Device Lifecycle

Each simulated device follows this cycle:

1. **Wake Up**
   - Connects to MQTT broker
   - Registers with OTA service (sends device info via MQTT)
   - Subscribes to OTA mode topic
   - Stays awake for random duration (5 to max_wakeup_seconds)

2. **OTA Update Detection**
   - Listens for "NEW-FIRMWARE-VERSION" message
   - If received, publishes "OTA-READY" signal
   - Starts TCP OTA server on assigned port
   - Receives firmware via ESPHome OTA protocol
   - Simulates reboot and re-registers with new version

3. **Deep Sleep**
   - Disconnects from MQTT
   - Sleeps for random duration (min_sleep to max_sleep seconds)
   - Repeats cycle

### Firmware Generation

A background task generates new firmware files:

- Runs at random intervals (5-10 minutes configurable)
- Selects 1-5 random devices for update
- Creates firmware files in format: `device-id - version.bin`
- Increments version number (semantic versioning)
- Generated files are fake (random bytes, 100-500KB)

### Device Naming

Devices are named: `{prefix}{number:03}`

Examples:
- `sim-esp32-001` (OTA port: 8266)
- `sim-esp32-002` (OTA port: 8267)
- `sim-esp32-050` (OTA port: 8315)

### Topics

Each device uses these MQTT topics:

- Registration: `{prefix}register` (e.g., `devices/register`)
- OTA Mode: `{device_id}ota-mode` (e.g., `sim-esp32-001ota-mode`)
- Readiness: `{device_id}ready` (e.g., `sim-esp32-001ready`)

## Testing Scenarios

### Small Scale Test (5-10 devices)
```yaml
simulator:
  num_devices: 5
deep_sleep:
  min_sleep_seconds: 30
  max_sleep_seconds: 60
  max_wakeup_seconds: 20
```

### Medium Scale Test (25-50 devices)
```yaml
simulator:
  num_devices: 25
deep_sleep:
  min_sleep_seconds: 60
  max_sleep_seconds: 180
  max_wakeup_seconds: 30
```

### Large Scale Test (100 devices)
```yaml
simulator:
  num_devices: 100
deep_sleep:
  min_sleep_seconds: 30
  max_sleep_seconds: 200
  max_wakeup_seconds: 30
```

### Fast Testing (Quick cycles)
```yaml
deep_sleep:
  min_sleep_seconds: 10
  max_sleep_seconds: 30
  max_wakeup_seconds: 15
firmware:
  generation_interval_min: 60  # 1 minute
  generation_interval_max: 120  # 2 minutes
```

## Monitoring

Watch the logs to see:

- Device registrations
- Firmware generation events
- OTA update processes
- Sleep/wake cycles
- Version updates

Example log output:
```
[2025-12-07 10:30:15] [INFO] [device] [sim-esp32-001] Starting device simulation
[2025-12-07 10:30:15] [INFO] [device] [sim-esp32-001] Device waking up
[2025-12-07 10:30:16] [INFO] [device] [sim-esp32-001] Registered with version 1.0.0
[2025-12-07 10:30:16] [INFO] [device] [sim-esp32-001] Will stay awake for 25 seconds
[2025-12-07 10:32:45] [INFO] [device] [sim-esp32-001] Received NEW-FIRMWARE-VERSION
[2025-12-07 10:32:45] [INFO] [device] [sim-esp32-001] Starting OTA server on port 8266
[2025-12-07 10:32:50] [INFO] [device] [sim-esp32-001] Firmware updated to version 1.0.1
```

## Troubleshooting

### Port Already in Use
If devices fail to start OTA servers, check if ports are available:
```bash
# Check if ports are in use
netstat -an | grep 826[0-9]

# Adjust base_ota_port in config
```

### MQTT Connection Issues
- Verify MQTT broker is running
- Check host/port settings
- Verify credentials if authentication is enabled

### No Firmware Updates
- Check firmware storage path exists and is writable
- Verify firmware generation is occurring (check logs)
- Ensure OTA service is running and monitoring the firmware directory

## Architecture

```
simulator/
├── src/
│   ├── main.rs              # Application entry point
│   ├── config.rs            # Configuration management
│   ├── device.rs            # Device simulation logic
│   ├── mqtt_client.rs       # MQTT client wrapper
│   └── firmware_generator.rs # Firmware file generator
├── config.yaml              # Configuration file
└── Cargo.toml              # Dependencies
```

## Protocol Compliance

The simulator implements the complete ESPHome OTA Protocol v2:

1. ✅ Magic byte handshake
2. ✅ Version negotiation
3. ✅ Feature support flags
4. ✅ Optional authentication
5. ✅ Firmware size transmission
6. ✅ MD5 checksum verification
7. ✅ Chunked data transfer (1024 bytes)
8. ✅ Acknowledgments every 8192 bytes
9. ✅ Status responses

## Integration with OTA Service

The simulator works seamlessly with the main OTA service:

1. Devices register via MQTT with all required fields
2. OTA service detects new firmware and sends notifications
3. Devices respond with OTA-READY when available
4. OTA service uploads firmware to device's OTA port
5. Device "reboots" and re-registers with new version
6. Firmware files are managed (optionally deleted if configured)

## Performance Notes

- Each device runs in its own async task
- MQTT connections are independent
- Memory usage scales linearly with device count
- Typical usage: ~10MB base + ~1MB per 10 devices
- Can simulate 100 devices on modest hardware

## License

Same as parent OTA service project
