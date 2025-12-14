use crate::config::HomeAssistantConfig;
use crate::database::Database;
use log::{debug, error, info, warn};
use rumqttc::{AsyncClient, QoS};
use serde_json::json;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::Mutex;
use tokio::time::{interval, Duration};

/// Home Assistant MQTT Discovery Manager
pub struct HomeAssistantDiscovery {
    config: HomeAssistantConfig,
    mqtt_client: AsyncClient,
    database: Arc<Mutex<Database>>,
    success_count: Arc<AtomicU64>,
    failure_count: Arc<AtomicU64>,
}

impl HomeAssistantDiscovery {
    /// Create a new Home Assistant discovery manager
    pub fn new(
        config: HomeAssistantConfig,
        mqtt_client: AsyncClient,
        database: Arc<Mutex<Database>>,
        success_count: Arc<AtomicU64>,
        failure_count: Arc<AtomicU64>,
    ) -> Self {
        Self {
            config,
            mqtt_client,
            database,
            success_count,
            failure_count,
        }
    }

    /// Start Home Assistant discovery and state publishing
    pub async fn start(self: Arc<Self>) {
        if !self.config.enabled {
            info!("Home Assistant discovery is disabled");
            return;
        }

        info!("Starting Home Assistant MQTT discovery");

        // Publish discovery messages
        if let Err(e) = self.publish_discovery_messages().await {
            error!("Failed to publish Home Assistant discovery messages: {}", e);
            return;
        }

        info!("Home Assistant discovery messages published successfully");

        // Start periodic state updates
        let update_interval = self.config.update_interval;
        info!(
            "Starting Home Assistant state updates every {} seconds",
            update_interval
        );

        let mut ticker = interval(Duration::from_secs(update_interval));

        loop {
            ticker.tick().await;

            if let Err(e) = self.publish_state_updates().await {
                error!("Failed to publish Home Assistant state updates: {}", e);
            }
        }
    }

    /// Publish MQTT discovery messages for all entities
    async fn publish_discovery_messages(&self) -> Result<(), Box<dyn std::error::Error>> {
        info!("Publishing Home Assistant discovery messages");

        // Device information shared by all entities
        let device = json!({
            "identifiers": [self.config.node_id],
            "name": self.config.device_name,
            "manufacturer": self.config.manufacturer.as_ref().unwrap_or(&"ESPHome OTA Service".to_string()),
            "model": self.config.model.as_ref().unwrap_or(&"Firmware Update Manager".to_string()),
            "sw_version": env!("CARGO_PKG_VERSION"),
        });

        // Sensor: Total Device Count
        self.publish_sensor_discovery(
            "device_count",
            "Device Count",
            "mdi:devices",
            None,
            "total",
            device.clone(),
        )
        .await?;

        // Sensor: Devices with Updates Available
        self.publish_sensor_discovery(
            "updates_available",
            "Updates Available",
            "mdi:update",
            None,
            "measurement",
            device.clone(),
        )
        .await?;

        // Sensor: Devices Currently Updating
        self.publish_sensor_discovery(
            "updating_count",
            "Devices Updating",
            "mdi:download",
            None,
            "measurement",
            device.clone(),
        )
        .await?;

        // Sensor: Last Check Time
        self.publish_sensor_discovery(
            "last_check",
            "Last Check",
            "mdi:clock-check",
            Some("timestamp"),
            "measurement",
            device.clone(),
        )
        .await?;

        // Sensor: Successful Updates
        self.publish_sensor_discovery(
            "success_count",
            "Successful Updates",
            "mdi:check-circle",
            None,
            "total_increasing",
            device.clone(),
        )
        .await?;

        // Sensor: Failed Updates
        self.publish_sensor_discovery(
            "failure_count",
            "Failed Updates",
            "mdi:alert-circle",
            None,
            "total_increasing",
            device.clone(),
        )
        .await?;

        // Binary Sensor: Service Status
        self.publish_binary_sensor_discovery(
            "service_status",
            "Service Status",
            "mdi:server",
            "connectivity",
            device.clone(),
        )
        .await?;

        info!("All Home Assistant discovery messages published");
        Ok(())
    }

    /// Publish a sensor discovery message
    async fn publish_sensor_discovery(
        &self,
        object_id: &str,
        name: &str,
        icon: &str,
        device_class: Option<&str>,
        state_class: &str,
        device: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config_topic = format!(
            "{}/sensor/{}/{}/config",
            self.config.discovery_prefix, self.config.node_id, object_id
        );

        let state_topic = format!(
            "{}/sensor/{}/{}/state",
            self.config.discovery_prefix, self.config.node_id, object_id
        );

        let unique_id = format!("{}_{}", self.config.node_id, object_id);

        let mut config_payload = json!({
            "name": name,
            "unique_id": unique_id,
            "state_topic": state_topic,
            "icon": icon,
            "state_class": state_class,
            "device": device,
        });

        if let Some(dc) = device_class {
            config_payload["device_class"] = json!(dc);
        }

        let payload = serde_json::to_string(&config_payload)?;

        debug!("Publishing discovery: {} -> {}", config_topic, payload);

        self.mqtt_client
            .publish(config_topic, QoS::AtLeastOnce, true, payload)
            .await?;

        Ok(())
    }

    /// Publish a binary sensor discovery message
    async fn publish_binary_sensor_discovery(
        &self,
        object_id: &str,
        name: &str,
        icon: &str,
        device_class: &str,
        device: serde_json::Value,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let config_topic = format!(
            "{}/binary_sensor/{}/{}/config",
            self.config.discovery_prefix, self.config.node_id, object_id
        );

        let state_topic = format!(
            "{}/binary_sensor/{}/{}/state",
            self.config.discovery_prefix, self.config.node_id, object_id
        );

        let unique_id = format!("{}_{}", self.config.node_id, object_id);

        let config_payload = json!({
            "name": name,
            "unique_id": unique_id,
            "state_topic": state_topic,
            "icon": icon,
            "device_class": device_class,
            "payload_on": "ON",
            "payload_off": "OFF",
            "device": device,
        });

        let payload = serde_json::to_string(&config_payload)?;

        debug!("Publishing discovery: {} -> {}", config_topic, payload);

        self.mqtt_client
            .publish(config_topic, QoS::AtLeastOnce, true, payload)
            .await?;

        Ok(())
    }

    /// Publish current state updates for all sensors
    async fn publish_state_updates(&self) -> Result<(), Box<dyn std::error::Error>> {
        debug!("Publishing Home Assistant state updates");

        // Query device statistics from database
        let db = self.database.lock().await;

        let all_devices = db
            .get_all_devices()
            .map_err(|e| format!("Failed to get devices: {}", e))?;

        let total_devices = all_devices.len() as i64;

        let updates_available = all_devices
            .iter()
            .filter(|d| {
                // Check if device has available version that differs from current
                // This would need to be checked against firmware files
                false // Simplified for now
            })
            .count() as i64;

        let updating_count = all_devices
            .iter()
            .filter(|d| matches!(d.state, crate::database::DeviceState::OtaTransmit))
            .count() as i64;

        drop(db); // Release database lock

        // Publish sensor states
        self.publish_sensor_state("device_count", &total_devices.to_string())
            .await?;
        self.publish_sensor_state("updates_available", &updates_available.to_string())
            .await?;
        self.publish_sensor_state("updating_count", &updating_count.to_string())
            .await?;

        // Last check time (current timestamp in ISO 8601 format)
        let last_check = chrono::Utc::now().to_rfc3339();
        self.publish_sensor_state("last_check", &last_check).await?;

        // Success count
        let success = self.success_count.load(Ordering::Relaxed);
        self.publish_sensor_state("success_count", &success.to_string())
            .await?;

        // Failure count
        let failure = self.failure_count.load(Ordering::Relaxed);
        self.publish_sensor_state("failure_count", &failure.to_string())
            .await?;

        // Service status (always ON if we're publishing)
        self.publish_binary_sensor_state("service_status", "ON")
            .await?;

        debug!(
            "Published states: devices={}, updates={}, updating={}, status=ON",
            total_devices, updates_available, updating_count
        );

        Ok(())
    }

    /// Publish a sensor state value
    async fn publish_sensor_state(
        &self,
        object_id: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state_topic = format!(
            "{}/sensor/{}/{}/state",
            self.config.discovery_prefix, self.config.node_id, object_id
        );

        debug!("Publishing state: {} -> {}", state_topic, value);

        self.mqtt_client
            .publish(state_topic, QoS::AtLeastOnce, false, value.to_string())
            .await?;

        Ok(())
    }

    /// Publish a binary sensor state value
    async fn publish_binary_sensor_state(
        &self,
        object_id: &str,
        value: &str,
    ) -> Result<(), Box<dyn std::error::Error>> {
        let state_topic = format!(
            "{}/binary_sensor/{}/{}/state",
            self.config.discovery_prefix, self.config.node_id, object_id
        );

        debug!("Publishing state: {} -> {}", state_topic, value);

        self.mqtt_client
            .publish(state_topic, QoS::AtLeastOnce, false, value.to_string())
            .await?;

        Ok(())
    }

    /// Publish an immediate state update (can be called on-demand)
    pub async fn publish_immediate_update(&self) -> Result<(), Box<dyn std::error::Error>> {
        self.publish_state_updates().await
    }
}
