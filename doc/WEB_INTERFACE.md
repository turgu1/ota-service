# Web Interface

The OTA Service includes a web-based monitoring interface accessible via a web browser.

## Configuration

Add the following section to your `config.yaml`:

```yaml
web:
  port: 8080              # Port for the web server
  username: "admin"       # Login username
  password: "admin"       # Login password (change in production!)
  refresh_period: 5       # Auto-refresh interval in seconds
```

## Features

- **Secure Authentication**: Session-based login with optional "remember me" functionality
  - Standard session: 24 hours
  - Remember me: 30 days

- **Real-time Device Monitoring**: WebSocket-based updates showing:
  - Device ID
  - IP and MAC addresses
  - Current firmware version
  - Device state (Idle, OTA in Progress, Update Available)
  - Last update time
  - WiFi signal strength (RSSI)
  - Deep sleep configuration
  - OTA port
  - Update and failure counts

- **Auto-refresh**: Configurable refresh period for device data

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

4. The device table will automatically update based on the configured refresh period

## Security Notes

- **Change default credentials**: The example configuration uses `admin/admin`. Change these in production!
- **Use HTTPS**: Consider placing the service behind a reverse proxy (nginx, Caddy) with HTTPS for secure remote access
- **Firewall**: Restrict access to the web port to trusted networks only

## Screenshots

The interface provides a clean, modern dashboard showing all registered devices in a sortable table with color-coded status indicators.
