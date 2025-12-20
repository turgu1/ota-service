# Web Interface

The OTA Service includes a comprehensive web-based monitoring and management interface accessible via a web browser. The interface provides real-time device monitoring, update history tracking, and configuration management capabilities.

## Configuration

Add the following section to your `config.yaml`:

```yaml
web:
  port: 8080                    # Port for the web server
  username: "admin"             # Login username
  password: "admin"             # Login password (change in production!)
  refresh_period: 5             # Auto-refresh interval in seconds
  edit_session_timeout: 15      # Configuration edit session timeout in minutes
```

## Features

### 1. Secure Authentication
- **Session-based login** with optional "remember me" functionality
  - Standard session: 24 hours
  - Remember me: 30 days
- **Password-protected configuration editing** with time-limited edit sessions

### 2. Devices Tab - Real-time Device Monitoring
WebSocket-based live updates showing:
- **Device ID** - Unique identifier for each device
- **IP Address** - Current network address (centered, clickable for easy copying)
- **MAC Address** - Hardware address (centered)
- **Firmware Version** - Current installed version (semantic version sorting)
- **State** - Current device state:
  - 🟢 Idle - Device ready, no updates pending
  - 🔵 OTA Transmit - Firmware upload in progress
  - 🟡 Update Available - New firmware ready to install
- **Last Update** - Timestamp of last device activity (formatted as relative time)
- **RSSI** - WiFi signal strength in dBm with color-coded indicator:
  - Green: > -60 dBm (Excellent)
  - Yellow: -60 to -70 dBm (Good)
  - Orange: -70 to -80 dBm (Fair)
  - Red: < -80 dBm (Poor)
- **Deep Sleep** - Indicates if device uses deep sleep mode
- **OTA Port** - Port used for firmware uploads
- **Update Count** - Number of successful firmware updates
- **Fail Count** - Number of failed update attempts

**Table Features:**
- **Column Sorting** - Click column headers to sort (saved in browser)
- **Resizable Columns** - Drag column borders to adjust width (saved in browser)
- **Semantic Version Sorting** - Firmware versions sorted correctly (e.g., 1.2.10 > 1.2.9)
- **Auto-refresh** - Configurable refresh period for device data
- **Real-time WebSocket updates** - Instant updates when devices register or change state

### 3. Update Log Tab - Firmware Upload History
Track all firmware upload attempts with:
- **Device ID** - Which device was updated
- **Version** - Firmware version that was deployed
- **Status** - SUCCESS ✅ or FAIL ❌
- **Fail Reason** - Detailed error message for failed uploads
- **Attempted At** - Timestamp of upload attempt (formatted date/time)

**Features:**
- **Sortable columns** - Click to sort by any field
- **Semantic version sorting** - Version numbers sorted correctly
- **Persistent sorting** - Sort preferences saved in browser
- **Color-coded status** - Green for success, red for failures
- **Failure diagnostics** - Detailed error messages to troubleshoot issues

### 4. Config Tab - Configuration Management
View and edit service configuration with:
- **Live configuration display** - Shows current settings from config file
- **Sensitive data masking** - Passwords and API tokens displayed as bullets (••••••••)
- **IP address masking** - First two octets visible, last two masked (e.g., 192.168.•.•)
- **Editable fields** - Click any value to edit in modal dialog
- **Password protection** - Sensitive fields require admin password to edit
- **Session-based editing** - 15-minute (configurable) authentication window
- **File and memory updates** - Changes saved to config.yaml file
- **Restart notification** - Clear indication when restart needed for changes

**Configuration Sections:**
- **MQTT** - Broker connection settings (host, port, client_id, username, password, keep_alive, registration_topic)
- **Database** - SQLite database configuration (path, pool_size)
- **Service** - Logging and OTA service settings (name, log_level, log_file_path, max_concurrent_updates, check_interval, ota_password, default_ota_port)
- **Firmware** - Firmware binary storage location (storage_path, erase_firmware_after_upload)
- **Pushover** - Notification settings if configured (enabled, api_token, user_key, device, priority)
- **Web** - Web interface configuration (port, username, password, refresh_period, edit_session_timeout)

**Edit Modal Features:**
- Two-step authentication for password fields
- Input validation
- Error feedback
- Type-aware value parsing (integers, booleans, durations, strings)

### 5. Service Restart
- **Restart button** - Appears in header after configuration changes
- **Confirmation dialog** - Prevents accidental restarts
- **Graceful shutdown** - Allows requests to complete before restart
- **Auto-reload** - Page reloads automatically after restart

## Usage

1. Start the OTA service with your configuration file:
   ```bash
   cargo run --release -- config.yaml
   ```

2. Open a web browser and navigate to:
   ```
   http://localhost:8080
   ```
   (Replace `localhost` with your server's IP address if accessing remotely, and adjust the port if you configured a different one)

3. Log in with the username and password from your configuration

4. Navigate between tabs:
   - **DEVICES** - Monitor all registered devices
   - **UPDATE LOG** - Review firmware upload history
   - **CONFIG** - View and edit configuration settings

## Configuration Editing Workflow

1. **Navigate to CONFIG tab**
2. **Click on any configuration value** to edit
3. **For password fields** (passwords, API tokens):
   - Enter admin password first
   - Session valid for 15 minutes (configurable)
   - Subsequent edits don't require password re-entry within session
4. **For other fields**:
   - Direct editing after first authentication
5. **Enter new value** and save
6. **Restart button appears** in header
7. **Click Restart** to apply changes that require service restart
8. **Page automatically reloads** after restart completes

## API Endpoints

The web interface communicates with these backend endpoints:

- `POST /login` - Authenticate user
- `POST /logout` - End session
- `GET /api/devices` - Get device list and upload history
- `GET /api/config` - Get current configuration (with sensitive data masked)
- `POST /api/config/validate-password` - Validate admin password for edit session
- `POST /api/config/update` - Update configuration value
- `POST /api/restart` - Restart the service
- `GET /ws` - WebSocket connection for real-time updates

## Security Notes

- **Change default credentials**: The example configuration uses `admin/admin`. Change these in production!
- **Use HTTPS**: Consider placing the service behind a reverse proxy (nginx, Caddy) with HTTPS for secure remote access
- **Firewall**: Restrict access to the web port to trusted networks only
- **Session management**: Sessions expire after 24 hours of inactivity (30 days with "remember me")
- **Configuration editing**: Protected by admin password with time-limited edit sessions
- **Data masking**: Sensitive data automatically masked in configuration display

## Browser Compatibility

The web interface is compatible with modern browsers:
- Chrome/Edge (recommended)
- Firefox
- Safari
- Opera

JavaScript must be enabled for the interface to function.

## Troubleshooting

### Cannot connect to web interface
- Verify service is running: `ps aux | grep ota-service`
- Check configured port in config.yaml
- Verify firewall allows connections to the web port
- Check service logs for errors

### WebSocket connection fails
- WebSocket uses the same port as HTTP
- Ensure no proxy is blocking WebSocket connections
- Check browser console for connection errors

### Configuration changes not applied
- Some changes require service restart (click Restart button)
- Check file permissions on config.yaml
- Review service logs for configuration errors

### Real-time updates not working
- Verify WebSocket connection in browser developer tools
- Check that refresh_period is set in configuration
- Ensure service can write to database

## Screenshots

The interface provides a clean, modern dashboard with:
- **Header** - Service title, connection status, last update time, and action buttons
- **Tab navigation** - Easy switching between Devices, Update Log, and Config views
- **Responsive table layout** - Adapts to screen size
- **Color-coded indicators** - Visual feedback for status, signal strength, and success/failure
- **Modal dialogs** - Clean interface for configuration editing
- **Professional styling** - Modern design with smooth animations

