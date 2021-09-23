use minidom::Element;
use serde::{Deserialize, Serialize};

use super::EventIdentifier;

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct TriggerItem {
    pub identifier: EventIdentifier,
    pub hik_id: String,
    pub description: String,
}

impl TriggerItem {
    pub fn parse(s: &str) -> Result<Vec<TriggerItem>, TriggerParseError> {
        let root: Element = s.parse()?;
        let event_triggers = root
            .get_child("EventNotification", minidom::NSChoice::Any)
            .unwrap_or(&root)
            .get_child("EventTriggerList", minidom::NSChoice::Any)
            .unwrap_or(&root)
            .children();
        let mut parsed = vec![];

        for event_trigger in event_triggers {
            let hik_id = event_trigger
                .get_child("id", minidom::NSChoice::Any)
                .ok_or_else(|| TriggerParseError::FieldMissing("id".to_string()))?
                .text();
            let event_type = event_trigger
                .get_child("eventType", minidom::NSChoice::Any)
                .ok_or_else(|| TriggerParseError::FieldMissing("eventType".to_string()))?
                .text();
            let description = event_trigger
                .get_child("eventDescription", minidom::NSChoice::Any)
                .map(|e| e.text())
                .unwrap_or_else(String::new);
            let channel = event_trigger
                .get_child("videoInputChannelID", minidom::NSChoice::Any)
                .or_else(|| {
                    event_trigger.get_child("dynVideoInputChannelID", minidom::NSChoice::Any)
                })
                .or_else(|| event_trigger.get_child("inputIOPortID", minidom::NSChoice::Any))
                .or_else(|| event_trigger.get_child("dynInputIOPortID", minidom::NSChoice::Any))
                .map(|e| e.text());

            let event_type = event_type
                .parse()
                .map_err(|e| TriggerParseError::EventTypeInvalid(event_type, e))?;
            let identifier = EventIdentifier::new(channel, event_type);

            parsed.push(TriggerItem {
                hik_id,
                identifier,
                description,
            })
        }

        Ok(parsed)
    }
}

impl From<EventIdentifier> for TriggerItem {
    fn from(e: EventIdentifier) -> Self {
        TriggerItem {
            description: String::new(),
            hik_id: format!(
                "{}{}",
                e.event_type.to_string(),
                e.channel
                    .as_ref()
                    .map(|c| format!("-{}", c))
                    .unwrap_or_default()
            ),
            identifier: e,
        }
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum TriggerParseError {
        XmlInvalid(error: minidom::Error) {
            from()
        }
        ChannelMissing {
            display("Event should contain either channelID or dynChannelID but neither were found")
        }
        FieldMissing(field: String) {
            display("Field was expected but missing: {}", field)
        }
        EventTypeInvalid(provided: String, error: String) {
            display("Event type `{}` was incorrectly formatted: {}", provided, error)
        }
    }
}

#[cfg(test)]
mod test {
    use super::TriggerItem;
    const TRIGGERS_CAM: &str = include_str!("../../samples/triggers_cam.xml");
    const TRIGGERS_NVR: &str = include_str!("../../samples/triggers_nvr.xml");
    const TRIGGERS_PTZ: &str = include_str!("../../samples/triggers_ptz.xml");

    #[test]
    fn test_parse_camera_samples() {
        let parsed = TriggerItem::parse(TRIGGERS_CAM).unwrap();
        insta::assert_yaml_snapshot!(parsed);
    }

    #[test]
    fn test_parse_nvr_samples() {
        let parsed = TriggerItem::parse(TRIGGERS_NVR).unwrap();
        insta::assert_yaml_snapshot!(parsed);
    }

    #[test]
    fn test_parse_ptz_samples() {
        let parsed = TriggerItem::parse(TRIGGERS_PTZ).unwrap();
        insta::assert_yaml_snapshot!(parsed);
    }
}
