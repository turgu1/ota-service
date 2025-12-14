# Home Assistant MQTT Discovery Integration

Complete guide for integrating the OTA Service with Home Assistant using MQTT discovery.

*Last updated: December 13, 2025*

## Overview

The OTA Service can automatically register itself in Home Assistant using the MQTT Discovery protocol. When enabled, the service publishes device discovery messages that Home Assistant uses to automatically create sensors and monitoring entities.

This integration provides:
- Real-time monitoring of the OTA service status
- Device count and update statistics
- Automatic sensor creation (no manual configuration required)
- Persistent entities that survive Home Assistant restarts

## Features

### Service Monitoring

The integration creates a single device in Home Assistant representing the OTA Service with the following entities:

1. **Device Count Sensor**
   - Shows the total number of registered ESPHome devices
   - Entity ID: `sensor.ota_service_device_count`
   - Useful for monitoring your device fleet size

2. **Updates Available Sensor**
   - Shows how many devices have firmware updates available
   - Entity ID: `sensor.ota_service_updates_available`
   - Triggers automations when updates are ready

3. **Devices Updating Sensor**
   - Shows the number of devices currently receiving firmware updates
   - Entity ID: `sensor.ota_service_updating_count`
   - Monitor active OTA operations in real-time

4. **Last Check Sensor**
   - Timestamp of the last state update
   - Entity ID: `sensor.ota_service_last_check`
   - Device class: `timestamp`
   - Verify the service is actively publishing updates

5. **Successful Updates Sensor**
   - Shows the total number of successful OTA updates since service start
   - Entity ID: `sensor.ota_service_success_count`
   - State class: `total_increasing`
   - Track update success rate and reliability

6. **Failed Updates Sensor**
   - Shows the total number of failed OTA updates since service start
   - Entity ID: `sensor.ota_service_failure_count`
   - State class: `total_increasing`
   - Monitor update failures for troubleshooting

7. **Service Status Binary Sensor**
   - Shows if the OTA service is online and functioning
   - Entity ID: `binary_sensor.ota_service_service_status`
   - Device class: `connectivity`
   - State: `ON` when service is running, `OFF` when unavailable

## Configuration

### Enable Home Assistant Discovery

Add the `home_assistant` section to your `config.yaml`:

```yaml
home_assistant:
  # Enable/disable Home Assistant MQTT discovery
  enabled: true
  
  # MQTT discovery topic prefix (Home Assistant default: "homeassistant")
  # Home Assistant listens to this prefix for device discovery messages
  discovery_prefix: "homeassistant"
  
  # Unique node ID for this OTA service instance
  # Used to identify this service in Home Assistant
  node_id: "ota_service"
  
  # Friendly device name displayed in Home Assistant
  device_name: "OTA Service"
  
  # Device manufacturer name (optional)
  manufacturer: "ESPHome OTA Service"
  
  # Device model name (optional)
  model: "Firmware Update Manager"
  
  # Update interval in seconds for publishing sensor states
  # How often to publish device count, update status, etc. to Home Assistant
  update_interval: 60
```

### Configuration Parameters

#### enabled
- **Type:** Boolean
- **Required:** Yes
- **Default:** `false`
- **Description:** Enable or disable Home Assistant MQTT discovery. When `false`, no discovery messages are published.

#### discovery_prefix
- **Type:** String
- **Required:** No
- **Default:** `"homeassistant"`
- **Description:** MQTT topic prefix where Home Assistant listens for discovery messages. Only change this if you've customized your Home Assistant MQTT discovery prefix.

#### node_id
- **Type:** String
- **Required:** Yes
- **Description:** Unique identifier for this OTA service instance. Must be unique if running multiple OTA services. Used in entity IDs and device identifiers.

#### device_name
- **Type:** String
- **Required:** Yes
- **Description:** Human-friendly name displayed in Home Assistant's device list and entity labels.

#### manufacturer
- **Type:** String
- **Required:** No
- **Default:** `"ESPHome OTA Service"`
- **Description:** Manufacturer name shown in Home Assistant device information.

#### model
- **Type:** String
- **Required:** No
- **Default:** `"Firmware Update Manager"`
- **Description:** Model name shown in Home Assistant device information.

#### update_interval
- **Type:** Integer (seconds)
- **Required:** No
- **Default:** `60`
- **Description:** How often the service publishes sensor state updates to Home Assistant. Lower values provide more real-time updates but increase MQTT traffic.

## Prerequisites

### Home Assistant Requirements

1. **MQTT Integration Enabled**
   - Go to Settings → Devices & Services → Integrations
   - Add MQTT integration if not already configured
   - Configure with your MQTT broker details

2. **MQTT Discovery Enabled** (default)
   - MQTT discovery is enabled by default in Home Assistant
   - Verify in `configuration.yaml`:
   ```yaml
   mqtt:
     discovery: true
     discovery_prefix: homeassistant  # Must match ota-service config
   ```

3. **Same MQTT Broker**
   - Both Home Assistant and OTA Service must connect to the same MQTT broker
   - Verify connectivity from both systems

### OTA Service Requirements

1. **MQTT Connection**
   - OTA service must be connected to MQTT broker
   - Verify MQTT configuration in `config.yaml`

2. **Home Assistant Configuration**
   - Add `home_assistant` section to `config.yaml`
   - Set `enabled: true`

3. **Restart Service**
   - After configuration changes: `sudo systemctl restart ota-service`
   - Check logs: `sudo journalctl -u ota-service -f`

## Installation Steps

### 1. Configure OTA Service

Edit your OTA service configuration file (e.g., `/etc/ota-service/config.yaml`):

```yaml
# ... existing mqtt, database, firmware configs ...

home_assistant:
  enabled: true
  discovery_prefix: "homeassistant"
  node_id: "ota_service"
  device_name: "OTA Service"
  manufacturer: "ESPHome OTA Service"
  model: "Firmware Update Manager"
  update_interval: 60

# ... rest of config ...
```

### 2. Restart OTA Service

```bash
# If running as systemd service
sudo systemctl restart ota-service

# Verify service started successfully
sudo systemctl status ota-service

# Check logs for Home Assistant initialization
sudo journalctl -u ota-service -n 50 | grep "Home Assistant"
```

Expected log output:
```
INFO: Starting Home Assistant MQTT discovery
INFO: Publishing Home Assistant discovery messages
INFO: All Home Assistant discovery messages published
INFO: Starting Home Assistant state updates every 60 seconds
```

### 3. Verify in Home Assistant

1. **Go to Settings → Devices & Services → MQTT**
2. **Click on "Devices" tab**
3. **Look for "OTA Service" device**
4. **Click on the device to see all entities**

If the device doesn't appear:
- Check MQTT integration is working
- Verify OTA service logs for errors
- Confirm both systems use the same MQTT broker
- Check MQTT discovery is enabled in Home Assistant

## Using the Integration

### Dashboard Card Example

Create a dashboard card to monitor the OTA service:

```yaml
type: entities
title: OTA Service
entities:
  - entity: sensor.ota_service_device_count
    name: Total Devices
  - entity: sensor.ota_service_updates_available
    name: Updates Available
  - entity: sensor.ota_service_updating_count
    name: Currently Updating
  - entity: sensor.ota_service_last_check
    name: Last Check
  - entity: binary_sensor.ota_service_service_status
    name: Service Status
```

### Automation Examples

#### Alert When Updates Available

```yaml
automation:
  - alias: "OTA - Notify When Updates Available"
    trigger:
      - platform: state
        entity_id: sensor.ota_service_updates_available
    condition:
      - condition: numeric_state
        entity_id: sensor.ota_service_updates_available
        above: 0
    action:
      - service: notify.mobile_app
        data:
          title: "Firmware Updates Available"
          message: "{{ states('sensor.ota_service_updates_available') }} device(s) have firmware updates ready"
```

#### Alert When Service Goes Offline

```yaml
automation:
  - alias: "OTA - Service Offline Alert"
    trigger:
      - platform: state
        entity_id: binary_sensor.ota_service_service_status
        to: "off"
        for:
          minutes: 5
    action:
      - service: notify.mobile_app
        data:
          title: "OTA Service Offline"
          message: "The OTA firmware service has gone offline"
          data:
            priority: high
```

#### Daily Update Report

```yaml
automation:
  - alias: "OTA - Daily Status Report"
    trigger:
      - platform: time
        at: "08:00:00"
    action:
      - service: notify.mobile_app
        data:
          title: "OTA Service Status"
          message: >
            Devices: {{ states('sensor.ota_service_device_count') }}
            Updates Available: {{ states('sensor.ota_service_updates_available') }}
            Currently Updating: {{ states('sensor.ota_service_updating_count') }}
```

## Troubleshooting

### Device Not Appearing in Home Assistant

**Symptoms:** OTA Service device doesn't appear in Home Assistant after enabling discovery.

**Solutions:**

1. **Check MQTT Integration**
   ```bash
   # In Home Assistant, verify MQTT is connected
   Settings → Devices & Services → MQTT
   # Should show "Connected" status
   ```

2. **Verify Discovery Prefix**
   - Check Home Assistant `configuration.yaml`:
     ```yaml
     mqtt:
       discovery_prefix: homeassistant
     ```
   - Must match OTA service `home_assistant.discovery_prefix`

3. **Check OTA Service Logs**
   ```bash
   sudo journalctl -u ota-service | grep "Home Assistant"
   ```
   Should see:
   - "Starting Home Assistant MQTT discovery"
   - "Publishing Home Assistant discovery messages"
   - "All Home Assistant discovery messages published"

4. **Restart Home Assistant**
   ```bash
   # Sometimes Home Assistant needs a restart to pick up new devices
   Settings → System → Restart Home Assistant
   ```

### Sensors Not Updating

**Symptoms:** Sensors appear but values don't update.

**Solutions:**

1. **Check Update Interval**
   - Verify `home_assistant.update_interval` in config
   - Wait for at least one interval period
   - Check OTA service logs for update activity

2. **Verify MQTT Connection**
   ```bash
   # Subscribe to sensor topics to verify messages
   mosquitto_sub -h mqtt-broker -t "homeassistant/sensor/ota_service/#" -v
   ```

3. **Check Service Status**
   ```bash
   sudo systemctl status ota-service
   # Should be "active (running)"
   ```

### Duplicate Entities

**Symptoms:** Multiple entities with similar names appear.

**Solutions:**

1. **Check Node ID Uniqueness**
   - Ensure `home_assistant.node_id` is unique
   - If running multiple instances, each needs a unique node_id

2. **Remove Old Entities**
   ```
   Home Assistant → Settings → Devices & Services → MQTT
   → Devices → Find duplicates → Delete unwanted device
   ```

3. **Clear Retained Messages**
   ```bash
   # If you changed node_id, clear old retained discovery messages
   mosquitto_pub -h mqtt-broker -t "homeassistant/sensor/old_node_id/#" -r -n
   mosquitto_pub -h mqtt-broker -t "homeassistant/binary_sensor/old_node_id/#" -r -n
   ```

### Service Status Always Shows "OFF"

**Symptoms:** Service status binary sensor never shows "ON".

**Solutions:**

1. **Verify Service is Running**
   ```bash
   sudo systemctl status ota-service
   ```

2. **Check Update Interval**
   - Status updates are published every `update_interval` seconds
   - Wait for at least one interval before expecting updates

3. **Check MQTT Publish Permissions**
   - Verify OTA service MQTT user has publish permissions
   - Check MQTT broker ACLs if configured

## MQTT Topic Structure

Understanding the MQTT topics used by the integration:

### Discovery Topics (Retained)

Published once at startup, retained for Home Assistant restarts:

```
homeassistant/sensor/ota_service/device_count/config
homeassistant/sensor/ota_service/updates_available/config
homeassistant/sensor/ota_service/updating_count/config
homeassistant/sensor/ota_service/last_check/config
homeassistant/binary_sensor/ota_service/service_status/config
```

### State Topics (Non-Retained)

Published periodically based on `update_interval`:

```
homeassistant/sensor/ota_service/device_count/state
homeassistant/sensor/ota_service/updates_available/state
homeassistant/sensor/ota_service/updating_count/state
homeassistant/sensor/ota_service/last_check/state
homeassistant/binary_sensor/ota_service/service_status/state
```

## Advanced Configuration

### Multiple OTA Service Instances

If running multiple OTA services (e.g., separate networks, testing):

```yaml
# Instance 1 - Production
home_assistant:
  enabled: true
  node_id: "ota_service_prod"
  device_name: "OTA Service (Production)"
  discovery_prefix: "homeassistant"

# Instance 2 - Development
home_assistant:
  enabled: true
  node_id: "ota_service_dev"
  device_name: "OTA Service (Development)"
  discovery_prefix: "homeassistant"
```

Each instance will appear as a separate device in Home Assistant.

### Custom Discovery Prefix

If you've customized Home Assistant's discovery prefix:

```yaml
# Home Assistant configuration.yaml
mqtt:
  discovery: true
  discovery_prefix: custom_prefix

# OTA Service config.yaml
home_assistant:
  enabled: true
  discovery_prefix: "custom_prefix"  # Must match
  # ... rest of config
```

### Reduced Update Frequency

For networks with bandwidth constraints:

```yaml
home_assistant:
  enabled: true
  update_interval: 300  # Update every 5 minutes instead of 60 seconds
  # ... rest of config
```

## Security Considerations

1. **MQTT Authentication**
   - Always use authentication on your MQTT broker
   - Don't expose MQTT broker to the internet without TLS

2. **Home Assistant Access**
   - Discovery messages contain no sensitive information
   - State messages contain only device counts and timestamps
   - No device IDs, IP addresses, or credentials are published

3. **Network Isolation**
   - Consider running MQTT on an isolated network
   - Use VLANs to separate IoT devices from main network

## Additional Resources

- [Home Assistant MQTT Discovery Documentation](https://www.home-assistant.io/integrations/mqtt/#mqtt-discovery)
- [MQTT Integration Guide](https://www.home-assistant.io/integrations/mqtt/)
- [OTA Service Main Documentation](../README.md)
- [Web Interface Guide](WEB_INTERFACE.md)
- [Installation Guide](SERVICE_INSTALL.md)

## Support

If you encounter issues with the Home Assistant integration:

1. Check the troubleshooting section above
2. Enable debug logging: `log_level: "debug"` in OTA service config
3. Check both OTA service and Home Assistant logs
4. Verify MQTT broker is functioning correctly
5. Test MQTT connectivity with mosquitto_sub/mosquitto_pub

For issues specific to Home Assistant:
- [Home Assistant Community Forum](https://community.home-assistant.io/)
- [Home Assistant Discord](https://discord.gg/home-assistant)

---

*For more information about the OTA Service, see the [main README](../README.md).*
