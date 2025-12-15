# Remote Firmware Deployment Guide

Guide for deploying ESPHome firmware to a remote OTA service using the `remote-deploy-device-firmware.sh` script.

*Last updated: December 15, 2025*

## Overview

The `remote-deploy-device-firmware.sh` script automates the deployment of compiled ESPHome firmware to an OTA service running on a remote server. It uses SSH and SCP to securely transfer firmware files over the network.

**Use this script when:**
- Your OTA service runs on a different machine (server, NAS, etc.)
- You compile ESPHome firmware on your local workstation
- You need to deploy firmware remotely without manual SCP commands

## Prerequisites

### Local Machine Requirements

1. **ESPHome CLI installed**
   ```bash
   pip install esphome
   ```

2. **SSH client installed** (typically pre-installed on Linux/macOS)
   ```bash
   # Verify SSH is available
   which ssh scp
   ```

3. **ESPHome device configuration files** with proper structure

### Remote Server Requirements

1. **OTA service installed and running** (see [SERVICE_INSTALL.md](SERVICE_INSTALL.md))

2. **SSH server running** on the remote host
   ```bash
   # On remote server, verify SSH is running
   sudo systemctl status ssh
   # or
   sudo systemctl status sshd
   ```

3. **User account on remote server** with appropriate permissions

4. **User is member of `ota-service` group** on remote server
   ```bash
   # On remote server
   sudo usermod -a -G ota-service <username>
   # User must log out and back in
   ```

## SSH Authentication Setup

### Using SSH Keys (Recommended)

SSH key-based authentication is strongly recommended for automation and security:

```bash
# 1. Generate SSH key pair (if you don't have one)
ssh-keygen -t ed25519 -C "your_email@example.com"
# Press Enter to accept default location
# Set a passphrase (recommended) or leave empty

# 2. Copy public key to remote server
ssh-copy-id user@remote-server.local

# 3. Test SSH connection (should not ask for password)
ssh user@remote-server.local "echo 'Connection successful'"
```

### Using Password Authentication

While possible, password authentication is less convenient:
- You'll be prompted for password on each deployment
- Not suitable for automation scripts
- Less secure than key-based authentication

## Script Configuration

Before first use, edit the script to set the remote configuration file path:

```bash
# Edit remote-deploy-device-firmware.sh
nano /path/to/ota-service/remote-deploy-device-firmware.sh
```

Find and adjust this line:
```bash
REMOTE_OTA_CONFIG_FILE="/etc/ota-service/config.yaml"
```

**Common paths:**
- Standard daemon installation: `/etc/ota-service/config.yaml`
- Custom installation: `/opt/ota-service/config.yaml` or similar

## Usage

### Basic Usage

```bash
# Navigate to your ESPHome configuration directory
cd /path/to/esphome-configs

# Deploy firmware to remote server
/path/to/ota-service/remote-deploy-device-firmware.sh <device.yaml> <remote-host>
```

### Examples

**Deploy to remote server using hostname:**
```bash
cd ~/esphome-configs
../ota-service/remote-deploy-device-firmware.sh esp32-kitchen.yaml user@homeserver.local
```

**Deploy to remote server using IP address:**
```bash
cd ~/esphome-configs
../ota-service/remote-deploy-device-firmware.sh esp32-bedroom.yaml pi@192.168.1.100
```

**Deploy with different SSH port:**
```bash
# Edit the script temporarily or use SSH config
cd ~/esphome-configs
../ota-service/remote-deploy-device-firmware.sh esp32-garage.yaml user@server.local
```

## ESPHome Configuration Requirements

Your ESPHome YAML file must include these fields in the `substitutions` section:

```yaml
substitutions:
  device_id: "esp32-kitchen"           # Required: Unique device identifier
  device_name: "ESP32 Kitchen"         # Required: Human-readable name
  firmware_version: "1.0.5"            # Required: Semantic version number
```

**Complete example:**
```yaml
substitutions:
  device_id: "esp32-kitchen"
  device_name: "ESP32 Kitchen"
  firmware_version: "1.0.5"

esphome:
  name: ${device_id}
  friendly_name: ${device_name}
  project:
    name: "myproject.${device_id}"
    version: ${firmware_version}

esp32:
  board: esp32dev
  framework:
    type: arduino

# ... rest of configuration
```

## Workflow

### Complete Deployment Workflow

1. **Modify ESPHome configuration** (add features, fix bugs, etc.)

2. **Update firmware version** in the YAML file:
   ```yaml
   substitutions:
     firmware_version: "1.0.6"  # Increment version
   ```

3. **Compile firmware locally:**
   ```bash
   cd ~/esphome-configs
   esphome compile esp32-kitchen.yaml
   ```

4. **Deploy to remote OTA service:**
   ```bash
   ../ota-service/remote-deploy-device-firmware.sh esp32-kitchen.yaml user@server.local
   ```

5. **Verify deployment:**
   - Check script output for success message
   - Monitor OTA service logs on remote server
   - Wait for device to receive update (next check cycle)

### Monitoring Remote OTA Service

While deploying or after deployment, you can monitor the remote OTA service:

```bash
# SSH into remote server
ssh user@remote-server.local

# View OTA service logs
sudo journalctl -u ota-service -f

# Check firmware files
ls -lh /var/lib/ota-service/firmware/

# Check OTA service status
sudo systemctl status ota-service
```

## Script Behavior

### What the Script Does

1. **Validates local files:**
   - Checks ESPHome YAML file exists
   - Verifies compiled firmware binary exists

2. **Tests SSH connection:**
   - Verifies remote host is reachable
   - Confirms SSH authentication works

3. **Reads remote configuration:**
   - SSH into remote server
   - Extracts firmware folder path from config file

4. **Validates remote permissions:**
   - Checks firmware folder exists
   - Verifies write permissions

5. **Extracts device information:**
   - Parses YAML file for device_id, device_name, firmware_version
   - Generates proper firmware filename

6. **Transfers firmware:**
   - Uses SCP to copy firmware to remote server
   - Names file: `<device_id> - <version>.bin`

7. **Confirms deployment:**
   - Lists all firmware versions for the device
   - Displays summary information

### Output Example

```
INFO: Current Directory: /home/user/esphome-configs
INFO: Remote Host: user@homeserver.local
INFO: Remote OTA Service Config: /etc/ota-service/config.yaml
INFO: ESPHome Device Config: esp32-kitchen.yaml

INFO: Testing SSH connection to remote host...
SUCCESS: SSH connection established
INFO: Extracting firmware storage path from remote OTA service config...
SUCCESS: Remote firmware folder: /var/lib/ota-service/firmware
INFO: Verifying remote firmware folder permissions...
SUCCESS: Remote firmware folder is accessible and writable
INFO: Extracting device information from ESPHome YAML...
SUCCESS: Device ID: esp32-kitchen
SUCCESS: Device Name: ESP32 Kitchen
SUCCESS: Firmware Version: 1.0.6

INFO: Generated firmware filename: esp32-kitchen - 1.0.6.bin
INFO: Remote destination path: /var/lib/ota-service/firmware/esp32-kitchen - 1.0.6.bin

INFO: Looking for compiled firmware at: .esphome/build/ESP32 Kitchen/.pioenvs/ESP32 Kitchen/firmware.bin
SUCCESS: Found firmware binary (824KiB)
INFO: Copying firmware to remote OTA service directory via SCP...
SUCCESS: Firmware deployed successfully to remote host!

INFO: Summary:
  Source: .esphome/build/ESP32 Kitchen/.pioenvs/ESP32 Kitchen/firmware.bin
  Remote Host: user@homeserver.local
  Remote Destination: /var/lib/ota-service/firmware/esp32-kitchen - 1.0.6.bin
  Device ID: esp32-kitchen
  Firmware Version: 1.0.6

SUCCESS: The OTA service will detect this firmware on the next check cycle.

INFO: All firmware versions for device 'esp32-kitchen' on remote host:
-rw-rw-r-- 1 ota-service ota-service 824K Dec 15 10:30 esp32-kitchen - 1.0.6.bin
-rw-rw-r-- 1 ota-service ota-service 820K Dec 10 14:22 esp32-kitchen - 1.0.5.bin
```

## Troubleshooting

### SSH Connection Issues

**Problem:** `Cannot connect to remote host`

**Solutions:**
```bash
# Test basic SSH connectivity
ssh user@remote-server.local

# Check if SSH is running on remote server
ssh user@remote-server.local "systemctl status ssh"

# Verify SSH key is properly installed
ssh -v user@remote-server.local

# If using non-standard SSH port, configure in ~/.ssh/config:
cat >> ~/.ssh/config << EOF
Host homeserver
    HostName homeserver.local
    User myuser
    Port 2222
EOF
```

### Permission Denied on Remote Server

**Problem:** `No write permission to remote firmware folder`

**Solutions:**
```bash
# SSH into remote server
ssh user@remote-server.local

# Check group membership
groups

# If not in ota-service group, ask administrator to add you
sudo usermod -a -G ota-service $USER

# Log out and back in
exit
ssh user@remote-server.local

# Verify group membership
groups | grep ota-service

# Test write permission
touch /var/lib/ota-service/firmware/test.txt
rm /var/lib/ota-service/firmware/test.txt
```

### Firmware Not Found Locally

**Problem:** `Compiled firmware not found`

**Solutions:**
```bash
# Make sure you're in the correct directory
cd /path/to/esphome-configs

# Compile the firmware first
esphome compile esp32-kitchen.yaml

# Check the build directory
ls -la .esphome/build/*/. pioenvs/*/firmware.bin

# If build directory is elsewhere, adjust your working directory
```

### Config File Not Found on Remote Server

**Problem:** `Could not extract firmware storage_path from remote config`

**Solutions:**
```bash
# Verify remote config file location
ssh user@remote-server.local "ls -la /etc/ota-service/config.yaml"

# Check config file contents
ssh user@remote-server.local "cat /etc/ota-service/config.yaml"

# Edit script to use correct path if different
nano remote-deploy-device-firmware.sh
# Update REMOTE_OTA_CONFIG_FILE variable
```

### SCP Transfer Fails

**Problem:** SCP transfer fails or times out

**Solutions:**
```bash
# Test manual SCP
scp /tmp/test.txt user@remote-server.local:/tmp/

# Check network connectivity
ping remote-server.local

# Verify remote disk space
ssh user@remote-server.local "df -h /var/lib/ota-service/firmware"

# Check remote firewall settings
ssh user@remote-server.local "sudo ufw status"
```

## Advanced Usage

### Using SSH Config File

Create `~/.ssh/config` to simplify remote host specification:

```bash
# ~/.ssh/config
Host ota-server
    HostName homeserver.local
    User otauser
    Port 22
    IdentityFile ~/.ssh/id_ed25519

Host ota-prod
    HostName production.example.com
    User deploybot
    Port 2222
    IdentityFile ~/.ssh/deploy_key
```

Then use short aliases:
```bash
../ota-service/remote-deploy-device-firmware.sh esp32-kitchen.yaml ota-server
../ota-service/remote-deploy-device-firmware.sh esp32-bedroom.yaml ota-prod
```

### Batch Deployment

Deploy multiple devices in sequence:

```bash
#!/bin/bash
# batch-deploy.sh

DEVICES=(
    "esp32-kitchen.yaml"
    "esp32-bedroom.yaml"
    "esp32-garage.yaml"
)

REMOTE_HOST="user@homeserver.local"

for device in "${DEVICES[@]}"; do
    echo "Deploying $device..."
    ../ota-service/remote-deploy-device-firmware.sh "$device" "$REMOTE_HOST"
    if [ $? -eq 0 ]; then
        echo "✓ $device deployed successfully"
    else
        echo "✗ $device deployment failed"
    fi
    echo ""
done
```

### Integration with CI/CD

Use the script in automated pipelines:

```yaml
# .gitlab-ci.yml example
deploy-firmware:
  stage: deploy
  script:
    - esphome compile esp32-device.yaml
    - ./remote-deploy-device-firmware.sh esp32-device.yaml deploy@ota-server.local
  only:
    - main
```

## Security Considerations

1. **Use SSH Keys:** Never commit private keys to version control
2. **Restrict SSH Access:** Limit SSH access to specific IP ranges if possible
3. **Group Permissions:** Only add trusted users to `ota-service` group
4. **Audit Deployments:** Monitor firmware deployments via logs
5. **Use Strong Passphrases:** Protect SSH keys with strong passphrases
6. **Firewall Rules:** Configure firewall to allow only necessary SSH connections

## Related Documentation

- [SERVICE_INSTALL.md](SERVICE_INSTALL.md) - Installing OTA service on remote server
- [HOME_ASSISTANT.md](HOME_ASSISTANT.md) - Home Assistant integration
- [../README.md](../README.md) - Main project documentation
- [WEB_INTERFACE.md](WEB_INTERFACE.md) - Web interface usage

## Comparison: Local vs Remote Deployment

| Feature | local-deploy-device-firmware.sh | remote-deploy-device-firmware.sh |
|---------|--------------------------------|----------------------------------|
| **Usage** | Local OTA service | Remote OTA service via SSH/SCP |
| **Arguments** | `<device.yaml>` | `<device.yaml> <remote-host>` |
| **Transfer Method** | Direct file copy (`cp`) | SCP over SSH |
| **Prerequisites** | Local file access | SSH access, SSH keys |
| **Group Membership** | Local user in ota-service | Remote user in ota-service |
| **Speed** | Fast (local copy) | Depends on network speed |
| **Use Case** | Service on same machine | Service on different machine |

## Support

For issues or questions:
1. Check the troubleshooting section above
2. Review OTA service logs: `sudo journalctl -u ota-service`
3. Verify SSH connectivity and permissions
4. Check ESPHome YAML configuration format
5. Consult the main [README.md](../README.md) for general service information
