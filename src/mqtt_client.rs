use log::{debug, error, info};
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::Mutex;
use tokio::time::sleep;

/// MQTT message wrapper
#[derive(Debug, Clone)]
pub struct MqttMessage {
    pub topic: String,
    pub payload: bytes::Bytes,
}

impl MqttMessage {
    /// Convert payload to UTF-8 string
    pub fn payload_str(&self) -> Result<String, std::string::FromUtf8Error> {
        String::from_utf8(self.payload.to_vec())
    }
}

/// MQTT client wrapper
pub struct MqttClient {
    client: AsyncClient,
    eventloop: Arc<Mutex<Option<EventLoop>>>,
}

impl MqttClient {
    /// Create a new MQTT client
    pub fn new(
        host: &str,
        port: u16,
        client_id: &str,
        username: Option<&str>,
        password: Option<&str>,
        keep_alive: u64,
        lwt_topic: Option<&str>,
        lwt_payload: Option<&str>,
    ) -> Result<Self, String> {
        info!(
            "Creating MQTT client: {}:{} (client_id: {})",
            host, port, client_id
        );

        let mut mqttoptions = MqttOptions::new(client_id, host, port);
        mqttoptions.set_keep_alive(Duration::from_secs(keep_alive));

        if let (Some(user), Some(pass)) = (username, password) {
            mqttoptions.set_credentials(user, pass);
            debug!("MQTT credentials configured");
        }

        // Set Last Will and Testament if provided
        if let (Some(topic), Some(payload)) = (lwt_topic, lwt_payload) {
            mqttoptions.set_last_will(rumqttc::LastWill::new(
                topic,
                payload,
                QoS::AtLeastOnce,
                false,
            ));
            info!("MQTT Last Will configured: {} -> {}", topic, payload);
        }

        let (client, eventloop) = AsyncClient::new(mqttoptions, 100);

        Ok(MqttClient {
            client,
            eventloop: Arc::new(Mutex::new(Some(eventloop))),
        })
    }

    /// Get a reference to the AsyncClient for Home Assistant or other integrations
    pub fn client(&self) -> &AsyncClient {
        &self.client
    }

    /// Wait for connection to be established
    pub async fn wait_connected(&self, max_attempts: u32) -> Result<(), String> {
        info!(
            "Waiting for MQTT connection (max {} attempts)",
            max_attempts
        );

        for attempt in 1..=max_attempts {
            debug!("Connection attempt {}/{}", attempt, max_attempts);
            sleep(Duration::from_secs(1)).await;

            // Try to subscribe to empty topic to test connection
            if self
                .client
                .subscribe("$SYS/broker/version", rumqttc::QoS::AtMostOnce)
                .await
                .is_ok()
            {
                info!("MQTT connection established");
                return Ok(());
            }
        }

        Err(format!(
            "Failed to connect to MQTT broker after {} attempts",
            max_attempts
        ))
    }

    /// Subscribe to a topic
    pub async fn subscribe(&self, topic: &str, qos: QoS) -> Result<(), String> {
        debug!("Subscribing to topic: {} (QoS: {:?})", topic, qos);

        self.client
            .subscribe(topic, qos)
            .await
            .map_err(|e| format!("Failed to subscribe to topic '{}': {}", topic, e))?;

        info!("Subscribed to topic: {}", topic);
        Ok(())
    }

    /// Publish a message to a topic
    pub async fn publish(
        &self,
        topic: &str,
        payload: &str,
        qos: QoS,
        retain: bool,
    ) -> Result<(), String> {
        debug!(
            "Publishing to topic: {} (QoS: {:?}, retain: {})",
            topic, qos, retain
        );

        self.client
            .publish(topic, qos, retain, payload)
            .await
            .map_err(|e| format!("Failed to publish to topic '{}': {}", topic, e))?;

        debug!("Published to topic: {}", topic);
        Ok(())
    }

    /// Clear retained message from a topic
    pub async fn clear_retained(&self, topic: &str) -> Result<(), String> {
        debug!("Clearing retained message from topic: {}", topic);

        self.client
            .publish(topic, QoS::AtLeastOnce, true, "")
            .await
            .map_err(|e| format!("Failed to clear retained message from '{}': {}", topic, e))?;

        debug!("Cleared retained message from topic: {}", topic);
        Ok(())
    }

    /// Get next message from the event loop
    pub async fn next_message(&mut self) -> Option<MqttMessage> {
        let mut guard = self.eventloop.lock().await;
        if let Some(ref mut eventloop) = *guard {
            // Use timeout to avoid blocking forever
            match tokio::time::timeout(Duration::from_millis(100), eventloop.poll()).await {
                Ok(Ok(event)) => {
                    if let Event::Incoming(Packet::Publish(publish)) = event {
                        return Some(MqttMessage {
                            topic: publish.topic.clone(),
                            payload: publish.payload,
                        });
                    }
                }
                Ok(Err(e)) => {
                    error!("MQTT event loop error: {}", e);
                }
                Err(_) => {
                    // Timeout - no message available
                }
            }
        }

        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_mqtt_message_payload_str() {
        let msg = MqttMessage {
            topic: "test/topic".to_string(),
            payload: bytes::Bytes::from("hello world"),
        };

        assert_eq!(msg.payload_str().unwrap(), "hello world");
        assert_eq!(msg.topic, "test/topic");
    }
}
