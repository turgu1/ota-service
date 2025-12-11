use log::{debug, error, info};
use reqwest::Client;
use serde_json::json;
use std::collections::HashMap;

/// Pushover notification client
pub struct PushoverClient {
    api_token: String,
    user_key: String,
    client: Client,
    enabled: bool,
}

impl PushoverClient {
    /// Create a new Pushover client
    pub fn new(api_token: String, user_key: String, enabled: bool) -> Self {
        info!("Initializing Pushover client (enabled: {})", enabled);

        PushoverClient {
            api_token,
            user_key,
            client: Client::new(),
            enabled,
        }
    }

    /// Send notification for successful OTA update
    pub async fn notify_success(
        &self,
        device_id: &str,
        version: &str,
        priority: i32,
    ) -> Result<(), String> {
        if !self.enabled {
            debug!("Pushover disabled, skipping success notification");
            return Ok(());
        }

        info!(
            "Sending success notification for device {} (version: {})",
            device_id, version
        );

        let title = format!("OTA Update Success: {}", device_id);
        let message = format!(
            "Device {} has been successfully updated to version {}",
            device_id, version
        );

        self.send_notification(&title, &message, priority).await
    }

    /// Send notification for failed OTA update
    pub async fn notify_failure(
        &self,
        device_id: &str,
        error: &str,
        priority: i32,
    ) -> Result<(), String> {
        if !self.enabled {
            debug!("Pushover disabled, skipping failure notification");
            return Ok(());
        }

        error!(
            "Sending failure notification for device {}: {}",
            device_id, error
        );

        let title = format!("OTA Update Failed: {}", device_id);
        let message = format!("Device {} update failed: {}", device_id, error);

        self.send_notification(&title, &message, priority).await
    }

    /// Send generic notification
    async fn send_notification(
        &self,
        title: &str,
        message: &str,
        priority: i32,
    ) -> Result<(), String> {
        let url = "https://api.pushover.net/1/messages.json";

        let mut params = HashMap::new();
        params.insert("token", self.api_token.as_str());
        params.insert("user", self.user_key.as_str());
        params.insert("title", title);
        params.insert("message", message);
        let priority_str = priority.to_string();
        params.insert("priority", &priority_str);

        debug!("Sending Pushover notification: {}", title);

        let response = self
            .client
            .post(url)
            .form(&params)
            .send()
            .await
            .map_err(|e| format!("Failed to send Pushover notification: {}", e))?;

        if response.status().is_success() {
            debug!("Pushover notification sent successfully");
            Ok(())
        } else {
            let status = response.status();
            let body = response
                .text()
                .await
                .unwrap_or_else(|_| "Unable to read response body".to_string());

            error!(
                "Pushover API returned error (status: {}): {}",
                status, body
            );
            Err(format!(
                "Pushover API error (status {}): {}",
                status, body
            ))
        }
    }

    /// Test notification to verify configuration
    pub async fn test_notification(&self) -> Result<(), String> {
        if !self.enabled {
            return Err("Pushover is disabled".to_string());
        }

        info!("Sending test notification");

        self.send_notification(
            "OTA Service Test",
            "This is a test notification from the OTA service",
            0,
        )
        .await
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_pushover_client_creation() {
        let client = PushoverClient::new(
            "test_token".to_string(),
            "test_user".to_string(),
            true,
        );

        assert_eq!(client.api_token, "test_token");
        assert_eq!(client.user_key, "test_user");
        assert!(client.enabled);
    }

    #[test]
    fn test_pushover_client_disabled() {
        let client = PushoverClient::new(
            "test_token".to_string(),
            "test_user".to_string(),
            false,
        );

        assert!(!client.enabled);
    }

    #[tokio::test]
    async fn test_notify_when_disabled() {
        let client = PushoverClient::new(
            "test_token".to_string(),
            "test_user".to_string(),
            false,
        );

        // Should succeed without sending when disabled
        assert!(client.notify_success("device1", "1.0.0", 0).await.is_ok());
        assert!(client
            .notify_failure("device1", "test error", 0)
            .await
            .is_ok());
    }
}
