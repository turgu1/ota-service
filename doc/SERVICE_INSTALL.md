# OTA Service Installation Guide

Complete instructions for installing the OTA Service as a systemd service on Linux systems.

*Last updated: December 12, 2025*

## Overview

This guide walks you through installing the OTA Service as a system service that:
- Starts automatically at boot
- Runs as a dedicated unprivileged user
- Implements security hardening
- Logs to systemd journal
- Restarts automatically on failure

## Prerequisites

- Linux system with systemd (most modern distributions)
- Root/sudo access
- Rust toolchain installed (for building)
- MQTT broker accessible on your network

## Installation Steps

### 1. Build the Service

```bash
cd /path/to/ota-service
cargo build --release
```

The compiled binary will be at `target/release/ota-service`.

### 2. Create Dedicated User

Create a system user for running the service:

```bash
sudo useradd -r -s /bin/false -d /var/lib/ota-service ota-service
```

**Flags explained:**
- `-r`: Creates a system account (UID < 1000)
- `-s /bin/false`: Prevents interactive login
- `-d /var/lib/ota-service`: Sets home directory

### 3. Create Directory Structure

```bash
# Create required directories
sudo mkdir -p /etc/ota-service
sudo mkdir -p /var/lib/ota-service/firmware
sudo mkdir -p /var/log/ota-service

# Set ownership
sudo chown -R ota-service:ota-service /var/lib/ota-service
sudo chown -R ota-service:ota-service /var/log/ota-service

# Set permissions
sudo chmod 755 /var/lib/ota-service
sudo chmod 755 /var/lib/ota-service/firmware
sudo chmod 755 /var/log/ota-service
```

**Directory structure:**
- `/etc/ota-service/` - Configuration files (owned by root)
- `/var/lib/ota-service/` - Database and working directory
- `/var/lib/ota-service/firmware/` - Firmware storage
- `/var/log/ota-service/` - Log files

### 4. Install Binary

```bash
# Copy binary to system location
sudo cp target/release/ota-service /usr/local/bin/

# Set ownership and permissions
sudo chown root:root /usr/local/bin/ota-service
sudo chmod 755 /usr/local/bin/ota-service

# Verify installation
/usr/local/bin/ota-service --version
```

### 5. Install Configuration

```bash
# Copy example configuration
sudo cp config.example.yaml /etc/ota-service/config.yaml

# Set ownership and permissions
sudo chown root:root /etc/ota-service/config.yaml
sudo chmod 644 /etc/ota-service/config.yaml

# Edit configuration with your settings
sudo nano /etc/ota-service/config.yaml
```

**Important configuration updates:**

```yaml
mqtt:
  host: "your-mqtt-broker.local"  # Update this
  username: "your-mqtt-user"       # Update this
  password: "your-mqtt-password"   # Update this

database:
  path: "/var/lib/ota-service/devices.db"

service:
  log_file_path: "/var/log/ota-service/ota-service.log"

firmware:
  storage_path: "/var/lib/ota-service/firmware"
  ota_password: "your_ota_password_hex"  # Update if using OTA auth

web:
  port: 8080
  username: "admin"
  password: "change_this_password"  # CHANGE THIS!
```

### 6. Install Systemd Service

```bash
# Copy service file
sudo cp ota-service.service /etc/systemd/system/

# Set permissions
sudo chmod 644 /etc/systemd/system/ota-service.service

# Reload systemd configuration
sudo systemctl daemon-reload
```

### 7. Enable and Start Service

```bash
# Enable service to start at boot
sudo systemctl enable ota-service

# Start service now
sudo systemctl start ota-service

# Check status
sudo systemctl status ota-service
```

**Expected output:**
```
● ota-service.service - OTA Service for ESPHome Devices
     Loaded: loaded (/etc/systemd/system/ota-service.service; enabled; preset: enabled)
     Active: active (running) since Thu 2025-12-12 10:30:00 EST; 5s ago
       Docs: https://github.com/turgu1/ota-service
   Main PID: 12345 (ota-service)
      Tasks: 8 (limit: 4915)
     Memory: 8.2M
        CPU: 150ms
     CGroup: /system.slice/ota-service.service
             └─12345 /usr/local/bin/ota-service /etc/ota-service/config.yaml
```

## Verification

### Check Service is Running

```bash
# Check service status
sudo systemctl status ota-service

# View recent logs
sudo journalctl -u ota-service -n 50

# Follow logs in real-time
sudo journalctl -u ota-service -f
```

### Test Web Interface

```bash
# From the server
curl http://localhost:8080

# From another machine
curl http://server-ip:8080
```

Open browser: `http://server-ip:8080`

### Verify Database

```bash
# Check database was created
ls -la /var/lib/ota-service/devices.db

# Check permissions
sudo -u ota-service test -w /var/lib/ota-service/devices.db && echo "OK" || echo "FAIL"
```

### Verify Firmware Directory

```bash
# Check firmware directory
ls -la /var/lib/ota-service/firmware/

# Test write access
sudo -u ota-service touch /var/lib/ota-service/firmware/test.txt && echo "OK" || echo "FAIL"
sudo -u ota-service rm /var/lib/ota-service/firmware/test.txt
```

## Service Management

### Basic Commands

```bash
# Start service
sudo systemctl start ota-service

# Stop service
sudo systemctl stop ota-service

# Restart service
sudo systemctl restart ota-service

# Reload configuration (if service supports it)
sudo systemctl reload ota-service

# Check status
sudo systemctl status ota-service

# Enable auto-start at boot
sudo systemctl enable ota-service

# Disable auto-start at boot
sudo systemctl disable ota-service
```

### Viewing Logs

```bash
# View all logs
sudo journalctl -u ota-service

# View last 100 lines
sudo journalctl -u ota-service -n 100

# Follow logs in real-time
sudo journalctl -u ota-service -f

# View logs since today
sudo journalctl -u ota-service --since today

# View logs from last hour
sudo journalctl -u ota-service --since "1 hour ago"

# View logs with specific priority (error and above)
sudo journalctl -u ota-service -p err
```

### Configuration Updates

After modifying `/etc/ota-service/config.yaml`:

```bash
# Restart service to apply changes
sudo systemctl restart ota-service

# Verify service started successfully
sudo systemctl status ota-service

# Check logs for any errors
sudo journalctl -u ota-service -n 50
```

## Updating the Service

### Update Binary

```bash
# Build new version
cd /path/to/ota-service
git pull  # or download new version
cargo build --release

# Stop service
sudo systemctl stop ota-service

# Backup current binary (optional)
sudo cp /usr/local/bin/ota-service /usr/local/bin/ota-service.backup

# Install new binary
sudo cp target/release/ota-service /usr/local/bin/
sudo chown root:root /usr/local/bin/ota-service
sudo chmod 755 /usr/local/bin/ota-service

# Start service
sudo systemctl start ota-service

# Verify
sudo systemctl status ota-service
```

### Update Service File

If `ota-service.service` was modified:

```bash
# Copy updated service file
sudo cp ota-service.service /etc/systemd/system/

# Reload systemd
sudo systemctl daemon-reload

# Restart service
sudo systemctl restart ota-service
```

## Troubleshooting

### Service Won't Start

**Check logs:**
```bash
sudo journalctl -u ota-service -n 50
```

**Common issues:**

1. **Configuration file missing:**
   ```
   Error: Config file not found: /etc/ota-service/config.yaml
   ```
   Solution: Ensure config.yaml exists and is readable

2. **Permission denied:**
   ```
   Error: Permission denied: /var/lib/ota-service/devices.db
   ```
   Solution: Fix ownership:
   ```bash
   sudo chown -R ota-service:ota-service /var/lib/ota-service
   ```

3. **Port already in use:**
   ```
   Error: Address already in use (os error 98)
   ```
   Solution: Check what's using the port:
   ```bash
   sudo lsof -i :8080
   ```

4. **MQTT connection failed:**
   ```
   Error: Failed to connect to MQTT broker
   ```
   Solution: Verify MQTT broker settings and network connectivity

### Check Service Permissions

```bash
# Verify user exists
id ota-service

# Check directory ownership
ls -la /var/lib/ota-service
ls -la /var/log/ota-service

# Test database access
sudo -u ota-service sqlite3 /var/lib/ota-service/devices.db ".tables"
```

### View Detailed Status

```bash
# Full service status
sudo systemctl status ota-service -l

# Check if service is enabled
sudo systemctl is-enabled ota-service

# Check if service is active
sudo systemctl is-active ota-service

# Show service dependencies
sudo systemctl list-dependencies ota-service
```

### Manual Service Test

Run the service manually to see detailed output:

```bash
# Stop systemd service first
sudo systemctl stop ota-service

# Run manually as ota-service user
sudo -u ota-service /usr/local/bin/ota-service /etc/ota-service/config.yaml

# Press Ctrl+C to stop

# Restart systemd service
sudo systemctl start ota-service
```

### Network Issues

```bash
# Check if web port is listening
sudo netstat -tlnp | grep 8080

# Or with ss
sudo ss -tlnp | grep 8080

# Test MQTT connectivity
telnet mqtt-broker 1883

# Check firewall rules
sudo iptables -L -n | grep 8080
```

### Performance Monitoring

```bash
# Monitor resource usage
sudo systemctl status ota-service

# Detailed resource usage
sudo systemd-cgtop

# Check memory usage
sudo systemctl show ota-service --property=MemoryCurrent

# Check CPU usage
sudo systemctl show ota-service --property=CPUUsageNSec
```

## Security Considerations

### File Permissions

Recommended permissions:

```bash
# Binary (read/execute for all, owned by root)
-rwxr-xr-x root:root /usr/local/bin/ota-service

# Configuration (read for all, owned by root)
-rw-r--r-- root:root /etc/ota-service/config.yaml

# Working directory (full access for service user)
drwxr-xr-x ota-service:ota-service /var/lib/ota-service

# Database (read/write for service user)
-rw-r--r-- ota-service:ota-service /var/lib/ota-service/devices.db
```

### Systemd Security Features

The service file includes these security hardening features:

- `NoNewPrivileges=true` - Prevents privilege escalation
- `PrivateTmp=true` - Isolated /tmp directory
- `ProtectSystem=strict` - Read-only filesystem except allowed paths
- `ProtectHome=true` - Home directories inaccessible
- `ProtectKernelTunables=true` - Kernel parameters read-only
- `RestrictAddressFamilies=AF_INET AF_INET6 AF_UNIX` - Limited network protocols

### Web Interface Security

**Change default password:**
```yaml
web:
  username: "admin"
  password: "use_strong_password_here"  # CHANGE THIS!
```

**Use firewall to restrict access:**
```bash
# Allow only from local network
sudo ufw allow from 192.168.1.0/24 to any port 8080
```

## Uninstallation

To completely remove the service:

```bash
# Stop and disable service
sudo systemctl stop ota-service
sudo systemctl disable ota-service

# Remove service file
sudo rm /etc/systemd/system/ota-service.service
sudo systemctl daemon-reload

# Remove binary
sudo rm /usr/local/bin/ota-service

# Remove configuration (optional - contains your settings)
sudo rm -rf /etc/ota-service

# Remove data (optional - contains database and firmware)
sudo rm -rf /var/lib/ota-service
sudo rm -rf /var/log/ota-service

# Remove user
sudo userdel ota-service
```

## Additional Resources

- Main README: [../README.md](../README.md)
- Web Interface Guide: [WEB_INTERFACE.md](WEB_INTERFACE.md)
- OTA Protocol Documentation: [ota/OTA_PROTOCOL.md](ota/OTA_PROTOCOL.md)
- Troubleshooting: [ota/OTA_QUICK_REFERENCE.md](ota/OTA_QUICK_REFERENCE.md)

## Support

If you encounter issues:

1. Check logs: `sudo journalctl -u ota-service -f`
2. Enable debug logging in config.yaml: `log_level: "debug"`
3. Run service manually for detailed output
4. Verify all permissions and file paths
5. Check MQTT broker connectivity

---

**Installation complete!** Your OTA Service should now be running and ready to manage firmware updates for your ESPHome devices.
