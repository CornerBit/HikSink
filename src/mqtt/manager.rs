use crate::{
    config::ConfigCamera,
    hikapi::{CameraEvent, CameraEventType, DetectionRegion, DeviceInfo, TriggerItem},
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use tracing::{error, warn};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Manager {
    cameras: Vec<CameraDetails>,
    topics: MqttTopics,
}

impl Manager {
    pub fn new(cameras: Vec<ConfigCamera>, topics: MqttTopics) -> Manager {
        Manager {
            topics,
            cameras: cameras
                .into_iter()
                .map(|camera| CameraDetails {
                    config: camera,
                    info: None,
                    triggers: Vec::new(),
                    connected: false,
                    log: "Initial connection in progress...".to_string(),
                })
                .collect(),
        }
    }
    /// Get the LWT for the entire Hik Sink bridge
    pub fn mqtt_lwt(&self) -> MqttMessage {
        MqttMessage::new(
            self.topics.get_global_availability(),
            MqttQoS::AtLeastOnce,
            true,
            "offline",
        )
    }
    /// Call this when an MQTT connection is established. This returns all state topics to be published, discovery messages, and an online notification
    pub fn mqtt_connection_established(&self) -> Vec<MqttMessage> {
        let mut messages = Vec::new();

        // Ensure all camera states are up to date
        for cam in &self.cameras {
            messages.append(&mut cam.message_complete_refresh(&self.topics));
        }

        // Publish global online message
        messages.push(MqttMessage::new(
            self.topics.get_global_availability(),
            MqttQoS::AtLeastOnce,
            true,
            "online",
        ));

        // Publish stats
        messages.push(self.message_global_stats());

        // Publish all discovery topics
        for cam in &self.cameras {
            messages.append(&mut cam.message_complete_discovery(&self.topics))
        }
        messages.append(&mut self.message_gloal_stats_discovery());

        messages
    }
    /// Updates system stats as an MQTT message
    fn message_global_stats(&self) -> MqttMessage {
        let num_cameras = self.cameras.len();
        let num_cameras_connected = self.cameras.iter().filter(|c| c.connected).count();
        let num_triggers: usize = self.cameras.iter().map(|c| c.triggers.len()).sum();
        MqttMessage::new(
            self.topics.get_global_stats(),
            MqttQoS::AtLeastOnce,
            true,
            serde_json::json!({
                "cameras_connected": num_cameras_connected,
                "cameras_disconnected": num_cameras - num_cameras_connected,
                "cameras_total": num_cameras,
                "triggers_total": num_triggers,
            }),
        )
    }
    /// Updates the discovery for the global stats
    fn message_gloal_stats_discovery(&self) -> Vec<MqttMessage> {
        let discovery = |key: &str, name: &str, uom: &str| {
            MqttMessage::new(
                self.topics.get_global_stats_discovery(key),
                MqttQoS::AtLeastOnce,
                true,
                serde_json::json!({
                    "availability": [
                        {
                            "topic": self.topics.get_global_availability(),
                        },
                    ],
                    "device": {
                        "identifiers": [
                            "hiksink_bridge",
                        ],
                        "manufacturer": "Hiksink",
                        "name": "HikSink Bridge",
                        "sw_version": format!("v{}", env!("CARGO_PKG_VERSION")),
                    },
                    "json_attributes_topic": self.topics.get_global_stats(),
                    "name": name,
                    "state_topic": self.topics.get_global_stats(),
                    "unique_id": format!("hiksink_stat_{}", key),
                    "value_template": format!("{{{{ value_json.{} }}}}", key),
                    "unit_of_measurement": uom,
                }),
            )
        };

        vec![
            discovery("cameras_connected", "Cameras Connected", "Cameras"),
            discovery("cameras_disconnected", "Cameras Disconnected", "Cameras"),
            discovery("cameras_total", "Total Cameras", "Cameras"),
            discovery("triggers_total", "Total Triggers", "Triggers"),
        ]
    }
    pub fn next_event(&mut self, event: CameraEvent) -> Vec<MqttMessage> {
        let mut messages = Vec::new();
        if let Some(cam) = self
            .cameras
            .iter_mut()
            .find(|c| c.config.identifier() == event.id)
        {
            match event.event {
                CameraEventType::Connected { info, triggers } => {
                    // We don't check for deleted triggers. This shouldn't happen since triggers are static for the same camera model
                    cam.triggers = triggers
                        .into_iter()
                        .map(|trigger| TriggerDetails {
                            trigger,
                            alerting: false,
                            regions: Vec::new(),
                            last_alert: Utc::now(),
                        })
                        .collect();
                    cam.info = Some(info);
                    cam.log = "Connected".into();
                    cam.connected = true;
                    messages.append(&mut cam.message_complete_refresh(&self.topics));
                    messages.append(&mut cam.message_complete_discovery(&self.topics));
                    messages.push(self.message_global_stats());
                }
                CameraEventType::Disconnected { error } => {
                    cam.connected = false;
                    cam.log = format!("Connection Error: {}", error);
                    messages.push(cam.message_log(&self.topics));
                    messages.push(cam.message_availability(&self.topics));
                }
                CameraEventType::Alert(alert) => {
                    // Find the matching trigger
                    let mut changed = false;
                    let alert_identifier = alert.identifier;
                    if let Some(trigger) = cam
                        .triggers
                        .iter_mut()
                        .find(|t| t.trigger.identifier == alert_identifier)
                    {
                        // Only update if changed (to prevent spamming messages)
                        if trigger.alerting != alert.active || trigger.regions != alert.regions {
                            changed = true;
                            trigger.alerting = alert.active;
                            trigger.regions = alert.regions;
                        }
                    } else {
                        #[allow(clippy::collapsible_else_if)]
                        if !alert_identifier.event_type.is_video_loss() {
                            // The video loss event is special in that it is not typically listed (for non-NVR models) in the initial trigger scan.
                            // It has no practical use for cameras as a video loss would be due to a connection failure.
                            warn!(
                                "Camera {} send an alert for a trigger which does not exist",
                                cam.config.identifier()
                            );
                        }
                    }

                    if changed {
                        // Unwrap here is safe since `changed` only set when trigger was updated
                        let trigger = cam
                            .triggers
                            .iter()
                            .find(|t| t.trigger.identifier == alert_identifier)
                            .unwrap();
                        messages.push(trigger.message_state(&self.topics, cam));
                    }
                }
            }
        } else {
            // This should not be possible, but is checked to prevent a complete crash in the event of programmer error.
            error!("Invalid camera event: {:?}", event);
        }
        messages
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
struct CameraDetails {
    pub config: ConfigCamera,
    pub info: Option<DeviceInfo>,
    pub triggers: Vec<TriggerDetails>,
    pub connected: bool,
    /// Stores either connection info or a connection error
    pub log: String,
}

impl CameraDetails {
    /// Publishes a complete refresh of camera availability and all trigger states
    pub fn message_complete_refresh(&self, topics: &MqttTopics) -> Vec<MqttMessage> {
        let mut messages = Vec::with_capacity(self.triggers.len() + 1);
        // Ensure the states of the camera's triggers are up to date
        messages.append(&mut self.message_trigger_states(topics));
        // Ensure the camera's availability is up to date
        messages.push(self.message_log(topics));
        messages.push(self.message_availability(topics));
        messages
    }
    /// Publishes all discovery topics for home assistant
    pub fn message_complete_discovery(&self, topics: &MqttTopics) -> Vec<MqttMessage> {
        if let Some(info) = self.info.as_ref() {
            self.triggers
                .iter()
                .map(|trigger| trigger.message_discovery(topics, self, info))
                .collect()
        } else {
            Vec::new()
        }
    }
    /// Publishes whether the camera is available (online)
    pub fn message_availability(&self, topics: &MqttTopics) -> MqttMessage {
        MqttMessage::new(
            topics.get_camera_availability(self),
            MqttQoS::AtLeastOnce,
            true,
            match self.connected {
                true => "online",
                false => "offline",
            },
        )
    }
    /// Publishes the connection details
    pub fn message_log(&self, topics: &MqttTopics) -> MqttMessage {
        MqttMessage::new(
            topics.get_camera_log(self),
            MqttQoS::AtLeastOnce,
            true,
            self.log.as_ref(),
        )
    }
    /// Publishes the state of all triggers
    pub fn message_trigger_states(&self, topics: &MqttTopics) -> Vec<MqttMessage> {
        self.triggers
            .iter()
            .map(|trigger| trigger.message_state(topics, self))
            .collect()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
struct TriggerDetails {
    pub trigger: TriggerItem,
    pub alerting: bool,
    pub regions: Vec<DetectionRegion>,
    pub last_alert: DateTime<Utc>,
}
impl TriggerDetails {
    /// Publish the state of the trigger
    pub fn message_state(&self, topics: &MqttTopics, cam: &CameraDetails) -> MqttMessage {
        MqttMessage::new(
            topics.get_trigger_state(cam, self),
            MqttQoS::AtLeastOnce,
            true,
            serde_json::json!({
                "alerting": self.alerting,
                "regions": self.regions,
            }),
        )
    }
    /// Publish discovery info for this trigger
    pub fn message_discovery(
        &self,
        topics: &MqttTopics,
        cam: &CameraDetails,
        info: &DeviceInfo,
    ) -> MqttMessage {
        let name = format!("{} {}", cam.config.name, self.trigger.identifier);
        let sw_version = format!(
            "HikSink v{} / Camera Firmware {} ({})",
            env!("CARGO_PKG_VERSION"),
            info.firmware_version,
            info.firmware_release_date
        );
        let mut discovery = serde_json::json!({
            "availability": [
                {
                    "topic": topics.get_global_availability(),
                },
                {
                    "topic": topics.get_camera_availability(cam),
                }
            ],
            "device": {
                "identifiers": [
                    format!("{}_hiksink", cam.config.identifier()),
                    info.serial_number,
                    info.mac_address,
                ],
                "manufacturer": "Hikvision",
                "name": cam.config.name,
                "sw_version": sw_version,
                "model": format!("{} ({})", info.model, info.device_type),
            },
            "json_attributes_topic": topics.get_trigger_state(cam, self),
            "name": name,
            "payload_off": false,
            "payload_on": true,
            "state_topic": topics.get_trigger_state(cam, self),
            "unique_id": format!("{}_hiksink", topics.get_discovery_identifier_trigger(cam, self)),
            "value_template": "{{ value_json.alerting }}"
        });
        // Add the fields that are only present if they are custom
        if let Some(icon) = self.trigger.identifier.event_type.icon() {
            discovery
                .as_object_mut()
                .unwrap()
                .insert("icon".into(), icon.into());
        }
        if let Some(device_class) = self.trigger.identifier.event_type.device_class() {
            discovery
                .as_object_mut()
                .unwrap()
                .insert("device_class".into(), device_class.into());
        }
        MqttMessage::new(
            topics.get_trigger_discovery(cam, self),
            MqttQoS::AtLeastOnce,
            true,
            discovery,
        )
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MqttTopics {
    pub base: String,
    pub home_assistant: String,
}

impl MqttTopics {
    pub fn new(base: String, home_assistant: String) -> Self {
        Self {
            base,
            home_assistant,
        }
    }

    pub(self) fn get_global_availability(&self) -> String {
        format!("{}/availability", self.base)
    }
    pub(self) fn get_global_stats(&self) -> String {
        format!("{}/stats", self.base)
    }
    pub(self) fn get_camera_base(&self, cam: &CameraDetails) -> String {
        format!("{}/device_{}", self.base, cam.config.identifier())
    }
    pub(self) fn get_camera_availability(&self, cam: &CameraDetails) -> String {
        format!("{}/availability", self.get_camera_base(cam))
    }
    pub(self) fn get_camera_log(&self, cam: &CameraDetails) -> String {
        format!("{}/log", self.get_camera_base(cam))
    }
    pub(self) fn get_trigger_base(&self, cam: &CameraDetails, trigger: &TriggerDetails) -> String {
        let identifier = &trigger.trigger.identifier;
        if let Some(channel) = identifier.channel.as_ref() {
            format!(
                "{}/ch{}/{}",
                self.get_camera_base(cam),
                channel,
                identifier.event_type.to_string()
            )
        } else {
            format!(
                "{}/{}",
                self.get_camera_base(cam),
                identifier.event_type.to_string()
            )
        }
    }
    pub(self) fn get_trigger_state(&self, cam: &CameraDetails, trigger: &TriggerDetails) -> String {
        self.get_trigger_base(cam, trigger)
    }

    pub(self) fn get_discovery_identifier_trigger(
        &self,
        cam: &CameraDetails,
        trigger: &TriggerDetails,
    ) -> String {
        let channel_identifier = trigger
            .trigger
            .identifier
            .channel
            .as_ref()
            .map(|c| format!("_ch{}", c))
            .unwrap_or_default();
        let type_identifier = format!("_{}", trigger.trigger.identifier.event_type.to_string());
        format!(
            "device_{}{}{}",
            cam.config.identifier(),
            channel_identifier,
            type_identifier
        )
    }

    pub(self) fn get_global_stats_discovery(&self, key: &str) -> String {
        format!("{}/sensor/hiksink/{}/config", self.home_assistant, key)
    }

    pub(self) fn get_trigger_discovery(
        &self,
        cam: &CameraDetails,
        trigger: &TriggerDetails,
    ) -> String {
        format!(
            "{}/binary_sensor/hiksink/{}/config",
            self.home_assistant,
            self.get_discovery_identifier_trigger(cam, trigger)
        )
    }
}
impl Default for MqttTopics {
    fn default() -> Self {
        Self {
            base: "hikvision_cameras".into(),
            home_assistant: "homeassistant".into(),
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct MqttMessage {
    pub topic: String,
    pub qos: MqttQoS,
    pub retain: bool,
    pub payload: MqttPayload,
}

impl MqttMessage {
    pub fn new(topic: String, qos: MqttQoS, retain: bool, payload: impl Into<MqttPayload>) -> Self {
        Self {
            topic,
            qos,
            retain,
            payload: payload.into(),
        }
    }
}
impl From<MqttMessage> for rumqttc::LastWill {
    fn from(m: MqttMessage) -> Self {
        rumqttc::LastWill::new(m.topic, m.payload.render(), m.qos.into(), m.retain)
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
#[allow(clippy::enum_variant_names)]
pub enum MqttQoS {
    AtMostOnce = 0,
    AtLeastOnce = 1,
    ExactlyOnce = 2,
}
impl From<MqttQoS> for rumqttc::QoS {
    fn from(q: MqttQoS) -> Self {
        use rumqttc::QoS;
        match q {
            MqttQoS::AtMostOnce => QoS::AtMostOnce,
            MqttQoS::AtLeastOnce => QoS::AtLeastOnce,
            MqttQoS::ExactlyOnce => QoS::ExactlyOnce,
        }
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub enum MqttPayload {
    Constant(String),
    Json(serde_json::Value),
}

impl MqttPayload {
    pub fn render(self) -> Vec<u8> {
        match self {
            MqttPayload::Constant(c) => c.into(),
            MqttPayload::Json(j) => j.to_string().into(),
        }
    }
}

impl From<&str> for MqttPayload {
    fn from(v: &str) -> Self {
        Self::Constant(v.into())
    }
}

impl From<String> for MqttPayload {
    fn from(v: String) -> Self {
        Self::Constant(v)
    }
}

impl From<serde_json::Value> for MqttPayload {
    fn from(v: serde_json::Value) -> Self {
        Self::Json(v)
    }
}

#[cfg(test)]
mod test {
    use crate::{
        config::ConfigCamera,
        hikapi::{
            AlertItem, CameraEvent, CameraEventType, DetectionRegion, DeviceInfo, EventIdentifier,
            EventType, RegionCoordinates, TriggerItem,
        },
    };

    use super::{Manager, MqttPayload, MqttTopics};

    fn sample_cameras() -> Vec<ConfigCamera> {
        vec![ConfigCamera {
            generated_id: "cam1".into(),
            name: "Camera 1".into(),
            address: "192.168.20.2".into(),
            port: None,
            username: "admin".into(),
            password: "password".into(),
        }]
    }

    fn sample_device_info() -> DeviceInfo {
        DeviceInfo {
            device_name: "Cam 1".into(),
            device_id: "7ccc4404-e05d-4376-8ebf-81127da67c11".into(),
            model: "DS-2DE4A425IW-DE".into(),
            serial_number: "DS-2DE4A425IW-DE20180101AAWRC52000000W".into(),
            mac_address: "ff:ff:ff:ff:ff:ff".into(),
            firmware_version: "V5.5.71".into(),
            firmware_release_date: "build 180725".into(),
            device_type: "IPDome".into(),
        }
    }

    #[test]
    fn test_initial_state() {
        let cams = sample_cameras();
        let manager = Manager::new(cams, MqttTopics::default());
        insta::assert_yaml_snapshot!(manager);
    }

    #[test]
    fn test_lwt() {
        let cams = sample_cameras();
        let manager = Manager::new(cams, MqttTopics::default());
        insta::assert_yaml_snapshot!(manager.mqtt_lwt());
    }

    #[test]
    fn test_mqtt_connection_initial() {
        let cams = sample_cameras();
        let manager = Manager::new(cams, MqttTopics::default());
        insta::assert_yaml_snapshot!(manager.mqtt_connection_established());
    }

    #[test]
    fn test_camera_connection() {
        let cams = sample_cameras();
        let mut manager = Manager::new(cams.clone(), MqttTopics::default());

        let messages = manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Connected {
                triggers: vec![
                    EventIdentifier::new(Some("1".into()), EventType::Motion).into(),
                    EventIdentifier::new(Some("1".into()), EventType::Io).into(),
                ],
                info: sample_device_info(),
            },
        });
        insta::assert_yaml_snapshot!(manager, {
            ".cameras[].triggers[].last_alert" => "[last_alert]"
        });
        // TODO: redact package version (sw_version) once supported in insta
        insta::assert_yaml_snapshot!(messages);
    }

    #[test]
    fn test_camera_alert_invalid() {
        let cams = sample_cameras();
        let mut manager = Manager::new(cams.clone(), MqttTopics::default());

        // Setup trigger
        let trigger1: TriggerItem =
            EventIdentifier::new(Some("1".into()), EventType::Motion).into();
        manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Connected {
                triggers: vec![trigger1],
                info: sample_device_info(),
            },
        });

        // Test nothing changes with invalid trigger
        let old_manager = manager.clone();
        let messages = manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Alert(AlertItem {
                active: true,
                date: "".to_string(),
                description: "".to_string(),
                post_count: 1,
                regions: vec![],
                identifier: EventIdentifier::new(Some("2".into()), EventType::Motion),
            }),
        });

        assert_eq!(manager, old_manager);
        assert_eq!(messages.len(), 0);
    }

    #[test]
    fn test_camera_alert_basic() {
        let cams = sample_cameras();
        let mut manager = Manager::new(cams.clone(), MqttTopics::default());

        // Setup trigger
        let trigger1: TriggerItem =
            EventIdentifier::new(Some("1".into()), EventType::Motion).into();
        manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Connected {
                triggers: vec![trigger1.clone()],
                info: sample_device_info(),
            },
        });

        // Send alert
        let messages = manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Alert(AlertItem {
                active: true,
                date: "".to_string(),
                description: "".to_string(),
                post_count: 1,
                regions: vec![],
                identifier: trigger1.identifier,
            }),
        });

        insta::assert_yaml_snapshot!(manager, {
            ".cameras[].triggers[].last_alert" => "[last_alert]"
        });
        insta::assert_yaml_snapshot!(messages);
    }

    #[test]
    fn test_camera_alert_regions() {
        let cams = sample_cameras();
        let mut manager = Manager::new(cams.clone(), MqttTopics::default());

        // Setup trigger
        let trigger1: TriggerItem =
            EventIdentifier::new(Some("1".into()), EventType::Motion).into();
        manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Connected {
                triggers: vec![trigger1.clone()],
                info: sample_device_info(),
            },
        });

        // Send alert with regions
        let messages = manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Alert(AlertItem {
                active: true,
                date: "".to_string(),
                description: "".to_string(),
                post_count: 1,
                regions: vec![DetectionRegion {
                    id: "0".into(),
                    sensitivity: 50,
                    coordinates: vec![
                        RegionCoordinates { x: 425, y: 600 },
                        RegionCoordinates { x: 160, y: 400 },
                    ],
                }],
                identifier: trigger1.identifier,
            }),
        });

        insta::assert_yaml_snapshot!(manager, {
            ".cameras[].triggers[].last_alert" => "[last_alert]"
        });
        insta::assert_yaml_snapshot!(messages);
    }

    #[test]
    fn test_camera_alert_regions_restored() {
        let cams = sample_cameras();
        let mut manager = Manager::new(cams.clone(), MqttTopics::default());

        // Setup trigger
        let trigger1: TriggerItem =
            EventIdentifier::new(Some("1".into()), EventType::Motion).into();
        manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Connected {
                triggers: vec![trigger1.clone()],
                info: sample_device_info(),
            },
        });

        // Send alert with regions
        manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Alert(AlertItem {
                active: true,
                date: "".to_string(),
                description: "".to_string(),
                post_count: 1,
                regions: vec![DetectionRegion {
                    id: "0".into(),
                    sensitivity: 50,
                    coordinates: vec![
                        RegionCoordinates { x: 425, y: 600 },
                        RegionCoordinates { x: 160, y: 400 },
                    ],
                }],
                identifier: trigger1.identifier.clone(),
            }),
        });
        // Disable alert and remove regions
        let messages = manager.next_event(CameraEvent {
            id: cams[0].identifier().to_string(),
            event: CameraEventType::Alert(AlertItem {
                active: false,
                date: "".to_string(),
                description: "".to_string(),
                post_count: 1,
                regions: vec![],
                identifier: trigger1.identifier,
            }),
        });

        insta::assert_yaml_snapshot!(manager, {
            ".cameras[].triggers[].last_alert" => "[last_alert]"
        });
        insta::assert_yaml_snapshot!(messages);
    }

    #[test]
    fn test_rendered_mqtt_payload() {
        let mq: MqttPayload = "offline".into();
        assert_eq!(mq.render(), "offline".as_bytes());
        let mq: MqttPayload = "offline".to_string().into();
        assert_eq!(mq.render(), "offline".as_bytes());
        let mq: MqttPayload =
            serde_json::json!({"test": "output", "nested": {"test": "output"}}).into();
        insta::assert_yaml_snapshot!(String::from_utf8(mq.render()).unwrap(), @r###"
        ---
        "{\"nested\":{\"test\":\"output\"},\"test\":\"output\"}"
        "###);
    }
}
