use crate::config::MqttConfig;
use log::error;
use rumqttc::{AsyncClient, Event, EventLoop, MqttOptions, Packet, QoS};
use std::time::Duration;

/// MQTT client wrapper for device communication
pub struct DeviceMqttClient {
    client: AsyncClient,
    event_loop: Option<EventLoop>,
}

impl DeviceMqttClient {
    /// Create a new MQTT client for a device
    pub fn new(device_id: &str, config: &MqttConfig) -> Result<Self, String> {
        let mut mqtt_options = MqttOptions::new(device_id, &config.host, config.port);

        mqtt_options.set_keep_alive(Duration::from_secs(config.keep_alive));

        // Set credentials if provided
        if let (Some(username), Some(password)) = (&config.username, &config.password) {
            mqtt_options.set_credentials(username, password);
        }

        let (client, event_loop) = AsyncClient::new(mqtt_options, 10);

        Ok(DeviceMqttClient {
            client,
            event_loop: Some(event_loop),
        })
    }

    /// Publish a message to a topic
    pub async fn publish(
        &mut self,
        topic: &str,
        payload: &str,
        qos: QoS,
        retain: bool,
    ) -> Result<(), String> {
        self.client
            .publish(topic, qos, retain, payload)
            .await
            .map_err(|e| format!("Failed to publish to {}: {}", topic, e))?;

        // Poll multiple times to process the publish, especially for QoS 2
        if let Some(ref mut event_loop) = self.event_loop {
            let poll_count = match qos {
                QoS::ExactlyOnce => 5, // QoS 2 needs more polls for full handshake
                QoS::AtLeastOnce => 2, // QoS 1 needs ACK
                QoS::AtMostOnce => 1,  // QoS 0 just needs to send
            };

            for _ in 0..poll_count {
                let _ = event_loop.poll().await;
                tokio::time::sleep(tokio::time::Duration::from_millis(10)).await;
            }
        }
        Ok(())
    }

    /// Subscribe to a topic
    pub async fn subscribe(&mut self, topic: &str, qos: QoS) -> Result<(), String> {
        self.client
            .subscribe(topic, qos)
            .await
            .map_err(|e| format!("Failed to subscribe to {}: {}", topic, e))?;

        // Poll once to process the subscription
        if let Some(ref mut event_loop) = self.event_loop {
            let _ = event_loop.poll().await;
        }
        Ok(())
    }

    /// Poll for next event/message
    pub async fn poll(&mut self) -> Option<Event> {
        if let Some(ref mut event_loop) = self.event_loop {
            match event_loop.poll().await {
                Ok(event) => Some(event),
                Err(e) => {
                    error!("MQTT poll error: {}", e);
                    None
                }
            }
        } else {
            None
        }
    }

    /// Wait for a specific message on a topic with timeout
    pub async fn _wait_for_message(
        &mut self,
        expected_topic: &str,
        timeout_secs: u64,
    ) -> Option<String> {
        let timeout = tokio::time::Duration::from_secs(timeout_secs);
        let start = tokio::time::Instant::now();

        while start.elapsed() < timeout {
            if let Some(event) = self.poll().await {
                if let Event::Incoming(Packet::Publish(publish)) = event {
                    if publish.topic == expected_topic {
                        if let Ok(payload) = String::from_utf8(publish.payload.to_vec()) {
                            return Some(payload);
                        }
                    }
                }
            }
            tokio::time::sleep(tokio::time::Duration::from_millis(100)).await;
        }
        None
    }

    /// Clear retained message by publishing empty payload
    pub async fn clear_retained(&mut self, topic: &str) -> Result<(), String> {
        self.publish(topic, "", QoS::AtLeastOnce, true).await
    }
}
