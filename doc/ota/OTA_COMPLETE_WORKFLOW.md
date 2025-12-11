# OTA Complete Workflow

This document describes the complete end-to-end workflow for OTA firmware updates in the system.

## Table of Contents

1. [Overview](#overview)
2. [Phase 1: Device Registration](#phase-1-device-registration)
3. [Phase 2: Firmware Check](#phase-2-firmware-check)
4. [Phase 3: OTA Update Initiation](#phase-3-ota-update-initiation)
5. [Phase 4: OTA Protocol Exchange](#phase-4-ota-protocol-exchange)
6. [Phase 5: Post-Update](#phase-5-post-update)
7. [State Transitions](#state-transitions)
8. [Error Handling](#error-handling)

## Overview

The OTA update process involves five distinct phases:

```
Registration → Firmware Check → Update Initiation → Protocol Exchange → Post-Update
```

Each phase has specific responsibilities and state transitions.

## Phase 1: Device Registration

**Trigger**: Device publishes registration message to MQTT

### Steps

1. **Device publishes to registration topic**
   ```
   Topic: home/ota/registration
   Payload: {
     "device_id": "esp32-001",
     "ip_address": "192.168.1.100",
     "mac_address": "AA:BB:CC:DD:EE:FF",
     "firmware_version": "1.0.0",
     "ota_port": 3232,
     "ota_readiness_topic": "home/esp32-001/ota/ready",
     "ota_mode_topic": "home/esp32-001/ota/mode"
   }
   ```

2. **Service receives registration**
   - Parses JSON payload
   - Creates/updates device record in database
   - Sets device state to `Idle`

3. **Service subscribes to device topics**
   - Subscribes to `ota_readiness_topic`
   - Monitors for "OTA-READY" messages

4. **Database record created**
   ```sql
   INSERT OR REPLACE INTO devices (
     device_id, ip_address, mac_address, 
     firmware_version, ota_port,
     ota_readiness_topic, ota_mode_topic,
     device_state, last_seen
   ) VALUES (?, ?, ?, ?, ?, ?, ?, 'Idle', datetime('now'))
   ```

**Outcome**: Device is registered and monitored

## Phase 2: Firmware Check

**Trigger**: Periodic check interval (configured in `check_interval`)

### Steps

1. **Service scans firmware directory**
   ```
   /var/lib/ota-service/firmware/
   ├── esp32-001-1.0.0.bin
   ├── esp32-001-1.1.0.bin  ← Newer version available!
   └── esp32-002-2.0.0.bin
   ```

2. **Version comparison**
   - Current device version: `1.0.0`
   - Available versions: `1.0.0`, `1.1.0`
   - Newer version found: `1.1.0`
   - Uses semantic version comparison

3. **Database state update**
   ```sql
   UPDATE devices 
   SET device_state = 'NewVersionAvailableTransmitted'
   WHERE device_id = 'esp32-001'
   ```

4. **MQTT notification sent**
   ```
   Topic: home/esp32-001/ota/mode
   Payload: "ON"
   QoS: 1 (At Least Once)
   Retain: true
   ```

**Outcome**: Device notified that update is available

## Phase 3: OTA Update Initiation

**Trigger**: Device publishes "OTA-READY" message

### Steps

1. **Device receives OTA mode notification**
   - Reads retained message from `ota_mode_topic`
   - Enters OTA mode
   - Publishes readiness signal

2. **Device publishes ready signal**
   ```
   Topic: home/esp32-001/ota/ready
   Payload: "OTA-READY"
   QoS: 1
   Retain: true
   ```

3. **Service receives OTA-READY**
   - Matches topic to device record
   - Loads device information from database
   - Identifies newer firmware version

4. **Service prepares for OTA**
   - Reads firmware binary from disk
   - Calculates MD5 hash
   - Updates device state to `OtaTransmit`

5. **Clear retained messages**
   ```
   # Clear ota_mode_topic (the notification)
   Topic: home/esp32-001/ota/mode
   Payload: (empty)
   Retain: true
   ```

**Outcome**: Ready to initiate TCP connection

## Phase 4: OTA Protocol Exchange

**Trigger**: Service initiates TCP connection to device

### Steps

#### 4.1 Connection Establishment

```rust
// Connect to device
let mut ota_client = OtaClient::connect(
    "192.168.1.100",  // device_ip
    3232,              // ota_port
    "password123"      // ota_password (optional)
)?;
```

#### 4.2 Hello Exchange

**Service → Device (HELLO)**
```
Magic Bytes: [0x6C, 0x26, 0xF7, 0x5C, 0x45]  (5 bytes)
Version:     0x02                             (1 byte)
Reserved:    0x00                             (1 byte)
```

**Device → Service (HELLO Response)**
```
Magic Bytes: [0x6C, 0x26, 0xF7, 0x5C, 0x45]  (5 bytes)
Version:     0x02                             (1 byte)
Features:    0x01 (supports compression)      (1 byte)
```

#### 4.3 Authentication (if password configured)

**Service calculates authentication**
```rust
let mut hasher = Sha256::new();
hasher.update(b"password123");  // OTA password
hasher.update(&random_nonce);   // Random 32 bytes from HELLO response
let auth_hash = hasher.finalize();
```

**Service → Device (AUTH)**
```
Command: 0x01 (AUTH)
Data:    [32-byte SHA256 hash]
```

**Device → Service (AUTH Response)**
```
Command: 0x00 (OK)
```

#### 4.4 Update Start

**Service → Device (UPDATE_START)**
```
Command: 0x64 (UPDATE_START)
Data:    [4-byte firmware size, little-endian]
         Example: [0x40, 0x42, 0x0F, 0x00] = 1,000,000 bytes
```

**Device → Service (UPDATE_START Response)**
```
Command: 0x00 (OK)
```

#### 4.5 Firmware Data Transfer

**Service sends firmware in chunks (1024 bytes each)**

```
Chunk 1:
Command: 0x65 (UPDATE_DATA)
Data:    [1024 bytes of firmware]

Device Response: 0x00 (OK)

Chunk 2:
Command: 0x65 (UPDATE_DATA)
Data:    [1024 bytes of firmware]

Device Response: 0x00 (OK)

...

Final Chunk (e.g., 808 bytes):
Command: 0x65 (UPDATE_DATA)
Data:    [808 bytes of firmware]

Device Response: 0x00 (OK)
```

**Progress Logging**
```
INFO: Starting OTA for esp32-001: 1.0.0 -> 1.1.0
DEBUG: Sent chunk 1/977 (0.1%)
DEBUG: Sent chunk 100/977 (10.2%)
DEBUG: Sent chunk 200/977 (20.5%)
...
DEBUG: Sent chunk 977/977 (100.0%)
```

#### 4.6 Update End

**Service → Device (UPDATE_END)**
```
Command: 0x66 (UPDATE_END)
Data:    [16-byte MD5 hash of entire firmware]
```

**Device validates firmware**
- Calculates MD5 of received data
- Compares with received hash
- Responds with result

**Device → Service (UPDATE_END Response)**
```
Command: 0x00 (OK) - Success!
or
Command: 0x81 (ERROR_INVALID_HASH) - MD5 mismatch
```

#### 4.7 Connection Close

```rust
// TCP connection closed
// Device will reboot and run new firmware
```

**Outcome**: Firmware successfully uploaded

## Phase 5: Post-Update

**Trigger**: OTA protocol completes successfully

### Steps

1. **Service updates database**
   ```sql
   UPDATE devices 
   SET firmware_version = '1.1.0',
       device_state = 'Idle',
       last_seen = datetime('now')
   WHERE device_id = 'esp32-001'
   ```

2. **Service logs success**
   ```
   INFO: OTA successful for esp32-001
   ```

3. **Optional: Delete firmware files**
   - If `erase_firmware_after_upload` is enabled
   - Removes the uploaded firmware file and all older versions for the device
   - Example: Deletes `esp32-001 - 1.1.0.bin`, `esp32-001 - 1.0.0.bin`, etc.

4. **Clear retained ready message**
   ```
   Topic: home/esp32-001/ota/ready
   Payload: (empty)
   Retain: true
   ```

5. **Device reboots**
   - Boots with new firmware version
   - Will re-register with updated version

6. **Next registration cycle**
   - Device publishes registration with `firmware_version: "1.1.0"`
   - Service updates device record
   - No new update available (already on latest)

**Outcome**: Update complete, device running new firmware

## State Transitions

### Device State Flow

```
┌──────┐
│ Idle │ ◄────────────────────────┐
└──┬───┘                          │
   │                              │
   │ Newer firmware found         │
   ▼                              │
┌────────────────────────────────┐│
│ NewVersionAvailableTransmitted ││
└──┬─────────────────────────────┘│
   │                              │
   │ Device sends OTA-READY       │
   ▼                              │
┌─────────────┐                   │
│ OtaTransmit │───────────────────┘
└─────────────┘   Update complete
                  or error
```

### State Descriptions

| State | Meaning | Next State |
|-------|---------|------------|
| `Idle` | Device registered, no update pending | `NewVersionAvailableTransmitted` (if newer version found) |
| `NewVersionAvailableTransmitted` | Device notified of available update | `OtaTransmit` (when OTA-READY received) |
| `OtaTransmit` | Actively uploading firmware | `Idle` (when complete or failed) |

## Error Handling

### Connection Errors

**Problem**: Cannot connect to device

```
ERROR: Failed to connect OTA client: Connection refused (os error 111)
```

**Possible Causes**:
- Device is offline
- Wrong IP address
- Device not in OTA mode
- Firewall blocking port 3232

**Recovery**:
- Device state returns to `Idle`
- Will retry on next firmware check cycle

### Authentication Errors

**Problem**: Password mismatch

```
ERROR: OTA failed for esp32-001: Authentication failed
```

**Possible Causes**:
- Wrong password configured
- Device expects password but none provided
- Device doesn't expect password but one provided

**Recovery**:
- Check device configuration
- Update service configuration
- Device state returns to `Idle`

### Transfer Errors

**Problem**: Network interruption during transfer

```
ERROR: OTA failed for esp32-001: Broken pipe (os error 32)
```

**Possible Causes**:
- Network connectivity lost
- Device crashed
- TCP timeout

**Recovery**:
- Device state returns to `Idle`
- Device may reboot to recovery
- Will retry on next notification

### Validation Errors

**Problem**: MD5 hash mismatch

```
ERROR: OTA failed for esp32-001: MD5 validation failed
```

**Possible Causes**:
- Data corruption during transfer
- Firmware file corrupted on disk
- Memory issues on device

**Recovery**:
- Device discards received data
- Device remains on old firmware
- Device state returns to `Idle`
- Will retry on next notification

## Timing and Intervals

### Configuration Options

```yaml
firmware:
  check_interval: 300  # Check for new firmware every 5 minutes
```

### Typical Timeline

```
T+0:00  - Device registers (firmware 1.0.0)
T+0:01  - Service starts firmware check loop
T+0:01  - New firmware detected (1.1.0 available)
T+0:01  - MQTT notification sent (ota/mode = ON)
T+0:02  - Device receives notification, enters OTA mode
T+0:03  - Device publishes OTA-READY
T+0:03  - Service initiates TCP connection
T+0:03  - Hello exchange (1 second)
T+0:04  - Authentication (1 second)
T+0:05  - Update start
T+0:05  - Firmware transfer begins (1MB @ 100KB/s = 10 seconds)
T+0:15  - Firmware transfer complete
T+0:15  - MD5 validation
T+0:16  - Update end, connection close
T+0:16  - Device reboots
T+0:20  - Device boots with new firmware
T+0:21  - Device registers (firmware 1.1.0)
T+0:22  - Service confirms update successful
```

**Total Time**: ~22 seconds for 1MB firmware

## Best Practices

### 1. Firmware Naming

**Always use semantic versioning**:
```
✓ esp32-001 - 1.0.0.bin
✓ esp32-001 - 1.1.0.bin
✓ esp32-001 - 2.0.0.bin
✗ esp32-001-latest.bin
✗ esp32-001.bin
```

### 2. Testing Updates

**Test on a single device first**:
```bash
# Deploy to test device only
cp new-firmware.bin "/var/lib/ota-service/firmware/esp32-test - 2.0.0.bin"

# Monitor logs
journalctl -u ota-service -f

# After success, deploy to production
cp new-firmware.bin /var/lib/ota-service/firmware/esp32-prod-2.0.0.bin
```

### 3. Staged Rollouts

**Update devices in groups**:
```bash
# Week 1: Test devices
cp firmware.bin /var/lib/ota-service/firmware/esp32-test-*-2.0.0.bin

# Week 2: 10% of production
cp firmware.bin /var/lib/ota-service/firmware/esp32-prod-001-2.0.0.bin
cp firmware.bin /var/lib/ota-service/firmware/esp32-prod-002-2.0.0.bin

# Week 3: 50% of production
# ...

# Week 4: All devices
```

### 4. Monitoring

**Watch for issues**:
```bash
# Check device states
sqlite3 /var/lib/ota-service/ota.db "SELECT device_id, firmware_version, device_state FROM devices;"

# Monitor success rate
grep "OTA successful" /var/log/ota-service.log | wc -l
grep "OTA failed" /var/log/ota-service.log | wc -l
```

### 5. Rollback Plan

**Keep previous firmware versions**:
```bash
# Don't enable erase_firmware_after_upload initially
firmware:
  erase_firmware_after_upload: false

# This allows easy rollback:
cp /var/lib/ota-service/firmware/esp32-001-1.9.0.bin \
   /var/lib/ota-service/firmware/esp32-001-2.0.1.bin  # Higher version
```

## Troubleshooting Workflows

### Device Won't Update

**Diagnosis Flow**:

1. **Check device is registered**
   ```bash
   sqlite3 /var/lib/ota-service/ota.db \
     "SELECT * FROM devices WHERE device_id = 'esp32-001';"
   ```

2. **Check firmware file exists**
   ```bash
   ls -la /var/lib/ota-service/firmware/esp32-001-*.bin
   ```

3. **Check version comparison**
   ```bash
   # Current: 1.0.0
   # Available: 1.0.0, 1.1.0
   # Should select 1.1.0
   ```

4. **Check MQTT messages**
   ```bash
   mosquitto_sub -h localhost -t "home/esp32-001/ota/#" -v
   ```

5. **Check device connectivity**
   ```bash
   ping 192.168.1.100
   nc -zv 192.168.1.100 3232
   ```

### Stuck in OtaTransmit State

**Diagnosis Flow**:

1. **Check current state**
   ```sql
   SELECT device_id, device_state, last_seen 
   FROM devices 
   WHERE device_state = 'OtaTransmit';
   ```

2. **Manual reset to Idle**
   ```sql
   UPDATE devices 
   SET device_state = 'Idle' 
   WHERE device_id = 'esp32-001';
   ```

3. **Clear retained messages**
   ```bash
   mosquitto_pub -h localhost -t "home/esp32-001/ota/mode" -r -n
   mosquitto_pub -h localhost -t "home/esp32-001/ota/ready" -r -n
   ```

4. **Trigger re-check**
   - Wait for next check interval
   - Or restart service to force immediate check

## Summary

The OTA workflow is a carefully orchestrated sequence:

1. **Registration** - Device announces presence
2. **Firmware Check** - Service monitors for updates
3. **Initiation** - Device enters OTA mode
4. **Protocol Exchange** - Firmware transferred via TCP
5. **Post-Update** - Database updated, device reboots

Each phase has clear responsibilities, error handling, and state transitions. Understanding this workflow enables effective troubleshooting and monitoring of the OTA update system.
