# ESPHome OTA - Quick Reference

## What is ESPHome OTA?

ESPHome OTA is the native over-the-air update protocol used by ESPHome devices. It allows you to upload new firmware to ESP32 devices wirelessly without physical connection.

## Key Facts

- **Port**: 3232 (TCP)
- **Protocol Version**: 2 (ESPHome OTA v2)
- **Magic bytes**: `[0x6C, 0x26, 0xF7, 0x5C, 0x45]` (5 bytes)
- **Chunk size**: 1024 bytes
- **Acknowledgments**: Every 8192 bytes (8 chunks)
- **Authentication**: MD5 or SHA256 with hex string concatenation
- **Typical firmware size**: 500KB - 2MB
- **Default timeout**: 30 seconds (all network operations)
- **Response codes**: 8 success codes (0x40-0x47), 13 error codes (0x80-0x8C, 0xFF)

## How It Works

1. **Connection**: TCP client connects to device on port 3232
2. **Magic Bytes**: Client sends 5-byte magic sequence
3. **Version Exchange**: Device responds with protocol version
4. **Features**: Client announces support (compression, SHA256 auth)
5. **Authentication** (if required): MD5 or SHA256 challenge-response
6. **Size & MD5**: Client sends firmware size (big-endian) and MD5 checksum
7. **Chunk Transfer**: Firmware uploaded in 1024-byte chunks with ACKs every 8KB
8. **Completion**: Device confirms successful update

## In Your Service

### Quick Upload Example

```rust
// Upload firmware to a device
ota_service
    .upload_firmware_ota(
        "esp32-001",           // device_id
        "192.168.1.100",       // device IP
        "1.2.3",               // firmware version
        None,                  // no password
    )
    .await?;
```

### With Authentication

```rust
ota_service
    .upload_firmware_ota(
        "esp32-001",
        "192.168.1.100",
        "1.2.3",
        Some("your_hex_password"),  // Password as hexadecimal string
    )
    .await?;
```

### Features

- **Timeout Protection**: All network reads have 30s timeout (configurable)
- **Error Handling**: Comprehensive error codes with descriptive messages
- **MD5 Verification**: Actual MD5 checksum calculated and verified
- **Chunk ACKs**: Device acknowledges every 8192 bytes received
- **Pushover Alerts**: Optional notifications on success/failure/new devices

## File Structure

```
/data/Dev/ota-service/
├── src/
│   ├── ota_client.rs          # OTA protocol implementation
│   ├── service.rs             # High-level OTA service (has upload_firmware_ota method)
│   └── ...
└── doc/ota/
    └── OTA_PROTOCOL.md            # Full protocol documentation
```

## Device Requirements

Your ESP32 must be running ESPHome firmware with OTA enabled:

```yaml
# ESPHome configuration example
ota:
  password: "optional_password"

wifi:
  ssid: "your-ssid"
  password: "your-password"
```

## Workflow in Your Service

```
┌──────────────────────────────────────────┐
│ 1. Periodic Check (every N seconds)      │
│    Scans firmware directory              │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│ 2. Find Latest Firmware                  │
│    Only uses LARGEST version number      │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│ 3. Compare with Device Version           │
│    Is latest > device current?           │
└────────────────┬─────────────────────────┘
                 │
         ┌───────┴────────┐
         │                │
        YES              NO
         │                │
         ▼                ▼
    ┌────────┐      ┌──────────┐
    │Notify  │      │ Skip     │
    │Device  │      │ Device   │
    └────┬───┘      └──────────┘
         │
         ▼
┌──────────────────────────────────────────┐
│ 4. Device Responds (OTA-READY)           │
│    Device signals ready for update       │
└────────────────┬─────────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────┐
│ 5. OTA Upload via Configured Port        │
│    (Default: 3232)                       │
│    ESPHome native OTA protocol           │
│    Firmware transferred in chunks        │
└────────────────┬─────────────────────────┘
                 │
         ┌───────┴────────┐
         │                │
      SUCCESS          FAILURE
         │                │
         ▼                ▼
    ┌────────┐      ┌──────────┐
    │Device  │      │ Log error│
    │Restarts│      │ Retry    │
    └────────┘      └──────────┘
```

## Common Scenarios

### Scenario 1: New Device Joins

```
1. Device connects to MQTT, publishes version "1.0.0"
2. Service receives registration message
3. Service stores device in database with version "1.0.0"
4. At next check interval, scans firmware directory
5. Finds "esp32-001 - 2.0.0.bin" available
6. Compares: 2.0.0 > 1.0.0 ✓
7. Sends "NEW-FIRMWARE-VERSION" to device's readiness topic
8. Device responds with "OTA-READY"
9. Service uploads "esp32-001 - 2.0.0.bin" via OTA
10. Device installs and restarts
11. Device re-registers with new version "2.0.0"
```

### Scenario 2: Multiple Firmware Versions Available

```
Firmware files in storage:
├── esp32-001 - 1.0.0.bin
├── esp32-001 - 1.5.0.bin
├── esp32-001 - 2.0.0.bin    ← SELECTED (largest)
└── esp32-001 - 2.0.1.bin    ← SELECTED (largest)

Device current version: 1.5.0

Action: Upload 2.0.1.bin (always uses largest available)
```

## Message Types

| Message | From | To | Purpose |
|---------|------|----|----|
| HelloRequest | Client | Device | Start OTA session |
| HelloResponse | Device | Client | Device ready, sends capabilities |
| FileRequest | Client | Device | Announce firmware size |
| FileResponse | Device | Client | Device ready to receive data |
| UpdateFinished | Device | Client | Update successful |
| UpdateFailed | Device | Client | Update failed |

## Troubleshooting

### Device not responding
```
Check:
1. Device IP address is correct
2. Device is online (ping it)
3. Port 3232 is accessible (nc -zv 192.168.1.100 3232)
4. Device ESPHome version supports OTA
```

### Upload times out
```
All network operations have 30-second default timeout.
Increase if needed:
let client = OtaClient::new(ip, 3232).with_timeout(120); // 2 minutes

Common timeout points:
- Version response
- Feature negotiation
- Authentication exchange
- Chunk acknowledgments (every 8192 bytes)
- Final status confirmation
```

### Authentication fails (0x82 error)
```
Error: ErrorAuthInvalid - Authentication invalid (password mismatch)

Solutions:
1. Verify password is in hexadecimal format
2. Check password matches device OTA configuration
3. Ensure authentication method (MD5 vs SHA256) is supported
4. Review logs for digest calculation details
```

### MD5 checksum mismatch (0x8B error)
```
Error: ErrorMd5Mismatch - MD5 checksum mismatch

The service calculates actual MD5 of firmware data.
Solutions:
1. Re-download/verify firmware file integrity
2. Check for file corruption during transfer
3. Verify firmware file hasn't changed during upload
```

### Connection refused
```
Verify:
1. Device is running ESPHome firmware
2. OTA is enabled in device config
3. Port 3232 is not blocked by firewall
4. Device is not in deep sleep mode
```

### Wrong firmware uploaded
```
Your service automatically selects the LARGEST version.
If device got wrong version:
1. Check firmware filenames match pattern: "device_id - version.bin"
2. Verify semantic versioning (e.g., "1.0.0" not "1.0")
3. Check version numbers are sorted correctly
```

## Error Response Codes

**Common Errors:**
- `0x82` - Authentication failed (password mismatch)
- `0x8B` - MD5 checksum mismatch
- `0x89` - ESP32 not enough space (firmware too large)
- `0x83` - Flash write error (device storage issue)

See OTA_PROTOCOL.md for complete list of 13 error codes.

## Performance Tips

1. **Chunk size**: 1024 bytes with ACKs every 8192 bytes (protocol v2)
2. **Timeout**: Default 30s on all operations, increase for slow networks
3. **Concurrent updates**: Limited by config `max_concurrent_updates`
4. **Check interval**: Balance between battery life (sleepy devices) and update latency
5. **Pushover**: Enable notifications to get immediate feedback on success/failure

## Security Considerations

- MD5 and SHA256 password authentication supported
- Passwords should be hexadecimal strings
- MD5 checksum verification of firmware data
- Assumes trusted network (no encryption in OTA protocol)
- Device IP should be on same network or VPN
- Consider firewall rules to restrict port 3232 access

## Testing Your Setup

### 1. Verify ESPHome on Device

```bash
# From device logs (check in ESPHome dashboard)
[OTA] OTA server started
[OTA] Listening on port 3232
```

### 2. Test Connectivity

```bash
# From service host
nc -zv 192.168.1.100 3232
# Output: Connection to 192.168.1.100 3232 port [tcp/*] succeeded!
```

### 3. Upload Test Firmware

Place test firmware in firmware storage directory:
```bash
mkdir -p /var/lib/ota-service/firmware
cp my_firmware.bin "/var/lib/ota-service/firmware/esp32-001 - 1.2.3.bin"
```

### 4. Check Logs

```bash
journalctl -u ota-service -f
# Look for: "OTA upload successful for device..."
```

## References

- ESPHome OTA Protocol: https://github.com/esphome/esphome/blob/dev/esphome/components/ota/ota.cpp
- ESP32 OTA Documentation: https://docs.espressif.com/projects/esp-idf/en/latest/esp32/api-reference/system/ota.html
