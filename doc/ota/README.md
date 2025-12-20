# OTA (Over-The-Air) Update System Documentation

Welcome! This folder contains comprehensive documentation for the ESPHome OTA (Over-The-Air) firmware update system implemented in this service.

## 📚 Documentation Files

### Quick Start
- **[OTA_QUICK_REFERENCE.md](OTA_QUICK_REFERENCE.md)** - Start here! Quick facts, basic examples, and troubleshooting
- **[README_OTA.md](README_OTA.md)** - Complete guide with architecture overview and feature descriptions

### Detailed Documentation
- **[OTA_PROTOCOL.md](OTA_PROTOCOL.md)** - Deep dive into the ESPHome OTA protocol with message structures and constants
- **[OTA_IMPLEMENTATION.md](OTA_IMPLEMENTATION.md)** - Implementation details and integration guide
- **[OTA_COMPLETE_WORKFLOW.md](OTA_COMPLETE_WORKFLOW.md)** - End-to-end workflow from device registration to firmware update

### Code Examples
- **[EXAMPLES.rs](EXAMPLES.rs)** - 7 practical code examples demonstrating OTA functionality
- **[INDEX.md](INDEX.md)** - Master index of all modules, features, and APIs

## 🎯 What is OTA?

Over-The-Air (OTA) is a method to wirelessly update firmware on devices without physical connection. The ESPHome OTA protocol communicates via:
- **Port**: 3232 (TCP)
- **Protocol Version**: 2
- **Magic bytes**: `[0x6C, 0x26, 0xF7, 0x5C, 0x45]` (5 bytes)
- **Features**: Compression support, SHA256 authentication, chunk acknowledgments

## 🚀 Quick Start Example

```rust
// Upload firmware to a device
ota_service
    .upload_firmware_ota(
        "device-001",           // device_id
        "192.168.1.100",       // device_ip
        "1.2.3",               // firmware_version
        None,                  // auth_password
    )
    .await?;
```

## 📖 Reading Guide

**For Different Audiences:**

### I want to understand what OTA is
→ Read [OTA_QUICK_REFERENCE.md](OTA_QUICK_REFERENCE.md) first

### I'm implementing this for the first time
→ Start with [README_OTA.md](README_OTA.md), then [OTA_IMPLEMENTATION.md](OTA_IMPLEMENTATION.md)

### I need code examples
→ Check [EXAMPLES.rs](EXAMPLES.rs) for 7 practical scenarios

### I need to debug OTA issues
→ See [OTA_QUICK_REFERENCE.md#troubleshooting](OTA_QUICK_REFERENCE.md)

### I need to understand the protocol details
→ Read [OTA_PROTOCOL.md](OTA_PROTOCOL.md)

### I need the complete workflow
→ Check [OTA_COMPLETE_WORKFLOW.md](OTA_COMPLETE_WORKFLOW.md)

## 🏗️ Architecture

```
┌─────────────────────────────────────┐
│         OtaService (High-level)     │
│  upload_firmware_ota(device_id, ip) │
└────────────────┬────────────────────┘
                 │
                 ▼
┌─────────────────────────────────────┐
│        OtaClient (Mid-level)        │
│  Manages protocol communication     │
└────────────────┬────────────────────┘
                 │
                 ▼
┌──────────────────────────────────────────────┐
│   ESPHome OTA Protocol (Configurable Port)   │
│   Default: 3232                              │
│  TCP communication with device               │
└──────────────────────────────────────────────┘
```

## 📋 Key Components

| Component | File | Purpose |
|-----------|------|---------|
| **OtaClient** | `src/ota_client.rs` | Low-level OTA protocol implementation |
| **OtaService** | `src/service.rs` | High-level API, device integration |
| **Device DB** | `src/database.rs` | Device state and history tracking |
| **Firmware Manager** | `src/firmware.rs` | Firmware file discovery and versioning |

## 🔄 Update Workflow

```
1. Device Registration
   └─> Service learns device IP, current firmware version

2. Periodic Check (configurable interval)
   └─> Service scans firmware directory

3. Version Comparison
   └─> Service identifies if newer firmware available
   └─> Always uses LARGEST version number

4. Availability Notification
   └─> Service sends "NEW-FIRMWARE-VERSION" via MQTT

5. Device Response
   └─> Device responds "OTA-READY"

6. OTA Upload (Configurable Port, Default: 3232)
   └─> Service uploads firmware via ESPHome protocol
   └─> Device installs and restarts

7. Re-registration
   └─> Device reconnects with new version
```

## ⚙️ Configuration

Your `config.yaml` should include:

```yaml
service:
  max_concurrent_updates: 10
  check_interval: 300  # 5 minutes
  default_ota_port: 3232  # Default OTA port (devices can override)
  ota_password: "your_OTA_password"  # Optional OTA authentication

firmware:
  storage_path: "/var/lib/ota-service/firmware"
  erase_firmware_after_upload: false  # Delete firmware after successful upload

pushover:  # Optional push notifications
  enabled: true
  api_token: "your_api_token"
  user_key: "your_user_key"
  priority: 0
```

## 🧪 Testing Your Setup

1. **Verify ESPHome on Device**
   ```bash
   # Check device logs - you should see:
   # [OTA] OTA server started
   # [OTA] Listening on port 3232
   ```

2. **Test Connectivity**
   ```bash
   nc -zv 192.168.1.100 3232
   # Connection successful!
   ```

3. **Place Test Firmware**
   ```bash
   cp firmware.bin "/var/lib/ota-service/firmware/device-001 - 1.2.3.bin"
   ```

4. **Check Logs**
   ```bash
   journalctl -u ota-service -f
   # Look for: "OTA upload successful..."
   ```

## 🔍 File Organization

```
doc/ota/
├── README.md                  (this file)
├── OTA_QUICK_REFERENCE.md     (quick facts & troubleshooting)
├── README_OTA.md              (complete guide)
├── OTA_PROTOCOL.md            (protocol details)
├── OTA_IMPLEMENTATION.md      (implementation guide)
├── OTA_COMPLETE_WORKFLOW.md   (end-to-end workflow)
├── INDEX.md                   (API reference)
└── EXAMPLES.rs                (7 code examples)

src/
├── ota_client.rs              (OTA protocol implementation)
├── service.rs                 (high-level API - upload_firmware_ota method)
├── database.rs                (device state tracking)
└── firmware.rs                (firmware discovery & versioning)
```

## 💡 Key Features

✅ **ESPHome Protocol v2** - Latest protocol with enhanced features  
✅ **Port 3232 Support** - Standard OTA port for ESPHome  
✅ **MD5 & SHA256 Authentication** - Secure firmware uploads  
✅ **Chunk Acknowledgments** - Every 8192 bytes for reliability  
✅ **Automatic Version Selection** - Always uses latest/largest version  
✅ **State Machine** - Tracks device update lifecycle (Idle → OtaTransmit → Idle)  
✅ **Timeout Protection** - All network operations have configurable timeouts  
✅ **Comprehensive Error Handling** - 13 different error response codes  
✅ **Pushover Notifications** - Optional push alerts for success/failure/new devices  
✅ **Async/Await** - Non-blocking concurrent operations  
✅ **Configuration-Driven** - All parameters in YAML config  
✅ **Extensive Logging** - Debug, info, and error level logging throughout  

## 🐛 Troubleshooting

### Device not responding
- Verify device IP address is correct
- Check device is online: `ping 192.168.1.100`
- Test OTA port: `nc -zv 192.168.1.100 3232`
- Verify ESPHome OTA is enabled in device config

### Upload times out
- Increase timeout: `OtaClient::new(ip, 3232).with_timeout(120)`
- Check network connectivity
- Verify device has enough free memory

### Wrong firmware uploaded
- Check filename format: `"device_id - version.bin"`
- Verify semantic versioning (e.g., "1.0.0" not "1.0")
- Service always selects largest version - check available files

### Connection refused
- Device might be offline or asleep
- Port 3232 might be blocked by firewall
- ESPHome might not have OTA enabled

See full troubleshooting in [OTA_QUICK_REFERENCE.md](OTA_QUICK_REFERENCE.md)

## 📞 Quick Reference

| Task | Documentation |
|------|---|
| Learn basics | [OTA_QUICK_REFERENCE.md](OTA_QUICK_REFERENCE.md) |
| Implement integration | [OTA_IMPLEMENTATION.md](OTA_IMPLEMENTATION.md) |
| See code examples | [EXAMPLES.rs](EXAMPLES.rs) |
| Understand protocol | [OTA_PROTOCOL.md](OTA_PROTOCOL.md) |
| View complete workflow | [OTA_COMPLETE_WORKFLOW.md](OTA_COMPLETE_WORKFLOW.md) |
| Check API reference | [INDEX.md](INDEX.md) |

## 🎓 Learning Path

1. **Start**: [OTA_QUICK_REFERENCE.md](OTA_QUICK_REFERENCE.md) (5 min read)
2. **Understand**: [README_OTA.md](README_OTA.md) (10 min read)
3. **See Examples**: [EXAMPLES.rs](EXAMPLES.rs) (browse scenarios)
4. **Deep Dive**: [OTA_PROTOCOL.md](OTA_PROTOCOL.md) (detailed read)
5. **Implement**: [OTA_IMPLEMENTATION.md](OTA_IMPLEMENTATION.md) (implementation guide)
6. **Reference**: [INDEX.md](INDEX.md) (API lookup)

## 🔗 Related Files

- **Source Code**: `src/ota_client.rs` - 340 lines of OTA protocol implementation
- **Service Integration**: `src/service.rs` - `upload_firmware_ota()` method
- **Configuration**: `Cargo.toml` - tokio, tokio-util, md5, bytes dependencies
- **Example Configuration**: `config.example.yaml` - firmware section with check_interval

## 📝 Version History

- **v1.0** - Initial ESPHome OTA protocol implementation
  - HelloRequest/Response handshake
  - FileRequest/Response negotiation
  - Firmware chunk transfer
  - UpdateFinished/Failed handling
  - State machine integration with device database

---

**Happy OTA updating! 🚀**
