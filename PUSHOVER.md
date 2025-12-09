# Pushover Integration for OTA Service

The OTA Service now supports push notifications via [Pushover](https://pushover.net) to alert you when firmware updates succeed or fail.

## Features

- **Success Notifications**: Get notified when firmware is successfully uploaded to a device
- **Failure Notifications**: Receive high-priority alerts when OTA updates fail
- **Configurable Priority**: Set default notification priority (-2 to 2)
- **Device-Specific**: Send notifications to specific devices
- **Optional**: Can be completely disabled or omitted from configuration

## Getting Started

### 1. Create a Pushover Account

1. Sign up at [pushover.net](https://pushover.net)
2. Purchase a license ($5 one-time purchase) or use the 30-day trial
3. Install the Pushover app on your mobile device

### 2. Get Your API Credentials

1. Log in to the Pushover website
2. Your **User Key** is displayed on your dashboard
3. Create an Application/API Token:
   - Go to "Create an Application/API Token"
   - Enter a name (e.g., "OTA Service")
   - Copy the generated **API Token/Key**

### 3. Configure the OTA Service

Add the following section to your `config.yaml`:

```yaml
# Pushover notification configuration (optional)
pushover:
  enabled: true
  api_token: "your_api_token_here"
  user_key: "your_user_key_here"
  device: "optional_device_name"  # Optional: send to specific device
  priority: 0  # -2=lowest, -1=low, 0=normal, 1=high, 2=emergency
```

## Configuration Options

| Option | Type | Required | Default | Description |
|--------|------|----------|---------|-------------|
| `enabled` | boolean | No | `true` | Enable/disable Pushover notifications |
| `api_token` | string | Yes* | - | Your Pushover API token/key |
| `user_key` | string | Yes* | - | Your Pushover user key |
| `device` | string | No | - | Specific device name to send to (optional) |
| `priority` | integer | No | `0` | Default notification priority (-2 to 2) |

*Required only when `enabled: true`

## Notification Priorities

Pushover supports different priority levels:

- **-2 (Lowest)**: No notification/alert
- **-1 (Low)**: Always send as quiet notification
- **0 (Normal)**: Standard priority
- **1 (High)**: Bypass user's quiet hours
- **2 (Emergency)**: Require acknowledgment (not currently used)

The service automatically sets appropriate priorities:
- **Success notifications**: Normal priority (0)
- **Failure notifications**: High priority (1) to ensure immediate attention

## Example Configurations

### Minimal Configuration

```yaml
pushover:
  enabled: true
  api_token: "azGDORePK8gMaC0QOYAMyEEuzJnyUi"
  user_key: "uQiRzpo4DXghDmr9QzzfQu27cmVRsG"
```

### Full Configuration

```yaml
pushover:
  enabled: true
  api_token: "azGDORePK8gMaC0QOYAMyEEuzJnyUi"
  user_key: "uQiRzpo4DXghDmr9QzzfQu27cmVRsG"
  device: "iphone"
  priority: 0
```

### Disabled Configuration

```yaml
pushover:
  enabled: false
  api_token: "azGDORePK8gMaC0QOYAMyEEuzJnyUi"
  user_key: "uQiRzpo4DXghDmr9QzzfQu27cmVRsG"
```

Or simply omit the entire `pushover` section from your config.

## Notification Examples

### Success Notification

**Title**: `OTA Success: device-001`

**Message**: `Firmware successfully uploaded to device-001 (192.168.1.100) - Version: 1.2.3`

**Priority**: Normal (0)

### Failure Notification

**Title**: `OTA Failed: device-001`

**Message**: `Failed to upload firmware to device-001 (192.168.1.100): Timeout waiting for chunk 42 acknowledgment`

**Priority**: High (1)

## Testing

To test your Pushover configuration:

1. Configure Pushover in your `config.yaml`
2. Start the OTA service
3. Check the logs for: `Pushover notifications enabled`
4. Trigger a firmware update
5. You should receive a notification on your device

## Troubleshooting

### "Pushover API error: application token is invalid"

- Double-check your `api_token` in the configuration
- Ensure you created an Application/API Token (not just using your user key)

### "Pushover API error: user identifier is invalid"

- Verify your `user_key` is correct
- Copy it directly from your Pushover dashboard

### Notifications not appearing

- Check that `enabled: true` in your configuration
- Verify the Pushover app is installed on your device
- Check the OTA service logs for error messages
- Ensure your Pushover account is active (not expired trial)

### "Failed to send Pushover notification: connection error"

- Check your internet connection
- Verify firewall allows HTTPS outbound to `api.pushover.net`
- Check if you're behind a proxy that requires configuration

## API Rate Limits

Pushover has the following limits:
- 10,000 messages per month per application
- No rate limit on individual requests

For typical OTA usage (a few updates per day), you won't approach these limits.

## Privacy & Security

- API tokens and user keys are stored in your configuration file
- Ensure your `config.yaml` has appropriate file permissions (e.g., `chmod 600`)
- Messages are sent over HTTPS to Pushover's API
- No sensitive device data is included in notifications (only device ID and IP)

## Disabling Notifications

To temporarily disable notifications without removing the configuration:

```yaml
pushover:
  enabled: false
  # ... rest of config ...
```

Or remove the entire `pushover` section from `config.yaml`.
