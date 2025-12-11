# ESPHome Configuration Examples for OTA Service

This directory contains example ESPHome configurations that demonstrate how to configure ESP32 devices to work with the OTA service.

## Files

### esp32-standard.yaml
Complete configuration for a **standard (non-sleeping) device** such as:
- Wall-powered sensors
- Smart switches
- Always-on monitoring devices
- Devices that need to respond immediately to updates

**Key features:**
- Automatic device registration on boot
- MQTT subscription to OTA mode topic
- Immediate response to firmware update notifications
- Continuous WiFi connection
- No deep sleep mode

### esp32-deep-sleep.yaml
Complete configuration for a **battery-powered device with deep sleep** such as:
- Outdoor temperature sensors
- Battery-powered door/window sensors
- Soil moisture sensors
- Remote environmental monitors

**Key features:**
- Periodic wake from deep sleep
- Device registration with `uses_deep_sleep: true` flag
- OTA update check on each wake cycle
- Prevents deep sleep when OTA update is available
- Power-optimized settings (fast connect, reduced logging)

## Prerequisites

Before using these examples, ensure you have:

1. **ESPHome installed**
   ```bash
   pip install esphome
   ```

2. **secrets.yaml file** with your credentials:
   ```yaml
   # secrets.yaml
   wifi_ssid: "YourWiFiSSID"
   wifi_password: "YourWiFiPassword"
   ap_password: "FallbackPassword"
   
   mqtt_broker: "homeassistant.local"
   mqtt_username: "mqtt-user"
   mqtt_password: "mqtt-pass"
   
   api_encryption_key: "your-32-char-api-key-here=="
   ```

3. **OTA Service running** with proper configuration
   - MQTT broker accessible
   - Topic prefix matching device configuration
   - OTA password matching device configuration

## Configuration Requirements

### Critical Settings to Match

Both device and OTA service must agree on:

1. **OTA Password**
   ```yaml
   # Device (ESPHome)
   ota:
     password: "d96112143a8c04d8b2945b226a9b95e7"
   
   # Service (config.yaml)
   firmware:
     ota_password: "d96112143a8c04d8b2945b226a9b95e7"
   ```

2. **Topic Structure**
   ```yaml
   # Device registration topic (where device publishes)
   ota-service/register
   
   # Device-specific topics
   devices/{device_id}/ota-mode      # Service publishes here
   devices/{device_id}/ready         # Device publishes here
   ```

3. **Registration JSON Format**
   ```json
   {
     "device_id": "esp32-kitchen",
     "ip_address": "192.168.1.100",
     "mac_address": "AA:BB:CC:DD:EE:FF",
     "firmware_version": "2024.11.0",
     "ota_readiness_topic": "devices/esp32-kitchen/ready",
     "ota_mode_topic": "devices/esp32-kitchen/ota-mode",
     "uses_deep_sleep": false,
     "rssi": -65
   }
   ```

## Usage

### Step 1: Customize the Configuration

1. Copy one of the example files
2. Update substitutions:
   ```yaml
   substitutions:
     device_name: esp32-YOUR-DEVICE
     device_id: esp32-YOUR-DEVICE
     friendly_name: "Your Device Name"
     ota_password: "your-hex-password"
   ```

3. Add your actual sensors and components

### Step 2: Compile and Upload Initial Firmware

```bash
# Compile the firmware
esphome compile esp32-standard.yaml

# Upload via USB (first time)
esphome upload esp32-standard.yaml

# Or generate binary for manual upload
esphome compile esp32-standard.yaml
# Binary will be in .esphome/build/{device_name}/.pioenvs/{device_name}/firmware.bin
```

### Step 3: Place Firmware in OTA Service Storage

After building new firmware versions:

```bash
# Copy firmware to OTA service storage with correct naming
cp .esphome/build/esp32-kitchen/.pioenvs/esp32-kitchen/firmware.bin \
   /var/lib/ota-service/firmware/"esp32-kitchen - 1.0.0.bin"
```

**Important:** Use the naming convention: `{device_id} - {version}.bin`

### Step 4: Verify Device Registration

Watch the OTA service logs to see device registration:

```bash
journalctl -u ota-service -f
```

You should see:
```
[INFO] Device registered: esp32-kitchen
[DEBUG] Device IP: 192.168.1.100, Firmware: 2024.11.0
```

### Step 5: Test Firmware Update

1. Build new firmware version
2. Copy to OTA service storage with higher version number
3. Wait for service to detect new firmware (check_interval)
4. Service will publish "NEW-FIRMWARE-VERSION"
5. Device will respond "OTA-READY"
6. OTA upload will begin automatically

## Deep Sleep Device Workflow

For battery-powered devices using `esp32-deep-sleep.yaml`:

```
1. Device wakes from deep sleep
   └─> Connects to WiFi
   └─> Connects to MQTT

2. Device registers with OTA service
   └─> Includes "uses_deep_sleep": true

3. Device waits 5 seconds for OTA notification
   └─> If "NEW-FIRMWARE-VERSION" received:
       ├─> Prevent deep sleep
       ├─> Respond "OTA-READY"
       └─> Wait for OTA upload
   └─> If no notification:
       ├─> Read sensors
       ├─> Publish data
       └─> Enter deep sleep

4. After OTA upload (if triggered)
   └─> Device restarts with new firmware
   └─> Registers with new version
```

## Troubleshooting

### Device not registering

Check:
- MQTT broker is accessible
- MQTT credentials are correct
- Topic prefix matches service configuration
- Device has WiFi connection

View device logs:
```bash
esphome logs esp32-standard.yaml
```

### OTA update not starting

Check:
- Firmware file naming is correct
- Version number is higher than current
- OTA password matches on both sides
- Port 3232 is accessible from service to device
- Device IP is correct in database

Test OTA port:
```bash
nc -zv 192.168.1.100 3232
```

### Deep sleep device not updating

Check:
- `uses_deep_sleep: true` in registration
- Device stays awake long enough (run_duration: 30s)
- OTA service detects device is awake
- Device prevents deep sleep when OTA is available

## Important Notes

### For All Devices

1. **First Upload**: Must be done via USB/serial (ESPHome cannot OTA to a blank device)

2. **OTA Password**: Must be consistent across:
   - Device ESPHome configuration
   - OTA service config.yaml
   - All firmware versions

3. **Network**: Device and OTA service must be on same network or have routing between them

### For Deep Sleep Devices

1. **Wake Duration**: Must be long enough for:
   - WiFi connection
   - MQTT connection
   - Registration message
   - Wait for OTA notification (5+ seconds)
   - Recommend: run_duration: 30s minimum

2. **OTA Timing**: Service must be configured to send notification quickly when deep sleep device registers

3. **Battery Impact**: OTA updates consume significant power, plan accordingly

## Advanced Customization

### Custom MQTT Topics

If you need different topic structure:

```yaml
# Update registration script
registration += "\"ota_readiness_topic\":\"custom/path/" + device_id + "/ready\",";
registration += "\"ota_mode_topic\":\"custom/path/" + device_id + "/mode\",";

# Update MQTT subscription
on_message:
  - topic: custom/path/${device_id}/mode
```

### Multiple Networks

For devices on different VLANs/subnets:
- Ensure routing between OTA service and device networks
- Firewall must allow TCP port 3232
- Consider separate MQTT topics per network

### Secure OTA

For production deployments:
- Use strong OTA passwords (32+ character hex strings)
- Enable Home Assistant API with encryption
- Use WPA2/WPA3 for WiFi
- Consider VPN for remote updates

## Example Sensors

Both example files include common sensors. You can add more:

```yaml
# Temperature sensor
sensor:
  - platform: dht
    pin: GPIO4
    temperature:
      name: "${friendly_name} Temperature"
    humidity:
      name: "${friendly_name} Humidity"
    model: DHT22

# Motion sensor
binary_sensor:
  - platform: gpio
    pin: GPIO14
    name: "${friendly_name} Motion"
    device_class: motion

# Relay switch
switch:
  - platform: gpio
    pin: GPIO12
    name: "${friendly_name} Relay"
```

## Version Numbering

ESPHome versions are automatically generated. You can also set manually:

```yaml
esphome:
  name: ${device_name}
  platformio_options:
    build_flags:
      - -DESPHOME_VERSION="1.2.3"
```

Then use this version in firmware filename: `esp32-kitchen - 1.2.3.bin`

## Resources

- **ESPHome Documentation**: https://esphome.io/
- **OTA Service Documentation**: See `../doc/ota/` directory
- **MQTT Protocol**: https://mqtt.org/
- **ESP32 Deep Sleep**: https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/sleep_modes.html

## Support

For issues with:
- **ESPHome configuration**: Check ESPHome documentation and logs
- **OTA service integration**: Check service logs and database
- **MQTT connectivity**: Verify broker and topic configuration
- **Firmware uploads**: Enable debug logging on both device and service
