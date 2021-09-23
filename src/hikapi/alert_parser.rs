use super::EventIdentifier;
use minidom::Element;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct RegionCoordinates {
    pub x: u32,
    pub y: u32,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct DetectionRegion {
    pub id: String,
    pub sensitivity: u8,
    pub coordinates: Vec<RegionCoordinates>,
}
#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct AlertItem {
    pub identifier: EventIdentifier,
    pub active: bool,
    pub regions: Vec<DetectionRegion>,
    pub post_count: u64,
    pub description: String,
    pub date: String,
}

impl AlertItem {
    pub fn parse(s: &str) -> Result<AlertItem, AlertParseError> {
        let root: Element = s.parse()?;
        if root.name() != "EventNotificationAlert" {
            return Err(AlertParseError::FieldMissing(
                "EventNotificationAlert".into(),
            ));
        }
        let event_type = root
            .get_child("eventType", minidom::NSChoice::Any)
            .ok_or_else(|| AlertParseError::FieldMissing("eventType".to_string()))?
            .text();
        let event_active = {
            let event_state = root
                .get_child("eventState", minidom::NSChoice::Any)
                .ok_or_else(|| AlertParseError::FieldMissing("eventState".to_string()))?;
            let event_active = event_state.text();
            match event_active.as_ref() {
                "active" => true,
                "inactive" => false,
                _ => return Err(AlertParseError::EventStateInvalid(event_active)),
            }
        };
        let event_description = root
            .get_child("eventDescription", minidom::NSChoice::Any)
            .ok_or_else(|| AlertParseError::FieldMissing("eventDescription".to_string()))?
            .text();
        let event_date = root
            .get_child("dateTime", minidom::NSChoice::Any)
            .ok_or_else(|| AlertParseError::FieldMissing("dateTime".to_string()))?
            .text();
        let active_post_count = {
            let pc = root
                .get_child("activePostCount", minidom::NSChoice::Any)
                .ok_or_else(|| AlertParseError::FieldMissing("activePostCount".to_string()))?;
            pc.text().parse::<u64>().map_err(|e| {
                AlertParseError::NumberExpected("activePostCount".into(), e.to_string())
            })?
        };
        let channel = root
            .get_child("channelID", minidom::NSChoice::Any)
            .or_else(|| root.get_child("dynChannelID", minidom::NSChoice::Any))
            .map(|c| c.text());
        let regions = pull_region_list(&root)?;

        let event_type = event_type
            .parse()
            .map_err(|e| AlertParseError::EventTypeInvalid(event_type, e))?;
        let identifier = EventIdentifier::new(channel, event_type);

        Ok(AlertItem {
            identifier,
            active: event_active,
            regions,
            post_count: active_post_count,
            description: event_description,
            date: event_date,
        })
    }
}

fn pull_region_list(el: &minidom::Element) -> Result<Vec<DetectionRegion>, AlertParseError> {
    let mut rl = Vec::new();

    let container = el.get_child("DetectionRegionList", minidom::NSChoice::Any);
    if let Some(container) = container {
        for child in container.children() {
            if child.name() != "DetectionRegionEntry" {
                return Err(AlertParseError::InvalidChild(
                    "DetectionRegionEntry".to_string(),
                    child.name().into(),
                ));
            }
            let id = child
                .get_child("regionID", minidom::NSChoice::Any)
                .ok_or_else(|| AlertParseError::FieldMissing("regionID".to_string()))?
                .text();
            let sensitivity = child
                .get_child("sensitivityLevel", minidom::NSChoice::Any)
                .ok_or_else(|| AlertParseError::FieldMissing("sensitivityLevel".to_string()))?
                .text()
                .parse::<u8>()
                .map_err(|e| {
                    AlertParseError::NumberExpected("sensitivityLevel".into(), e.to_string())
                })?;

            let mut region_coordinates = Vec::new();
            if let Some(coords) = child.get_child("RegionCoordinatesList", minidom::NSChoice::Any) {
                for child in coords.children() {
                    let x: u32 = child
                        .get_child("positionX", minidom::NSChoice::Any)
                        .ok_or_else(|| AlertParseError::FieldMissing("positionX".to_string()))?
                        .text()
                        .parse::<u32>()
                        .map_err(|e| {
                            AlertParseError::NumberExpected("positionX".into(), e.to_string())
                        })?;
                    let y: u32 = child
                        .get_child("positionY", minidom::NSChoice::Any)
                        .ok_or_else(|| AlertParseError::FieldMissing("positionY".to_string()))?
                        .text()
                        .parse::<u32>()
                        .map_err(|e| {
                            AlertParseError::NumberExpected("positionXY".into(), e.to_string())
                        })?;
                    region_coordinates.push(RegionCoordinates { x, y });
                }
            }
            rl.push(DetectionRegion {
                id,
                sensitivity,
                coordinates: region_coordinates,
            });
        }
    }
    Ok(rl)
}

quick_error! {
    #[derive(Debug, Serialize, Deserialize, Clone)]
    pub enum AlertParseError {
        XmlInvalid(error: String) {
            from(e: minidom::Error) -> (e.to_string())
        }
        FieldMissing(field: String) {
            display("Field was expected but missing: {}", field)
        }
        NumberExpected(field: String, error: String) {
            display("{} should be a number: {}", field, error)
        }
        EventTypeInvalid(provided: String, error: String) {
            display("Event type `{}` was incorrectly formatted: {}", provided, error)
        }
        EventStateInvalid(found: String) {
            display("Event state should be active / inactive. Got: {}", found)
        }
        InvalidChild(expected: String, found: String) {
            display("Child node in XML invalid. Expected {}, found {}", expected, found)
        }
    }
}

#[cfg(test)]
mod test {
    use super::AlertItem;
    use serde::Deserialize;
    const SAMPLES_CAM: &str = include_str!("../../samples/samples_cam.txt");
    const SAMPLES_NVR: &str = include_str!("../../samples/samples_nvr.txt");
    const SAMPLES_PTZ: &str = include_str!("../../samples/samples_ptz.txt");

    #[test]
    fn test_parse_all_samples() {
        let mut text = SAMPLES_CAM.to_string();
        text.push_str(SAMPLES_NVR);
        text.push_str(SAMPLES_PTZ);

        let mut all_parsed = Vec::new();
        for sample in text.lines() {
            #[derive(Deserialize)]
            struct Line {
                pub content: String,
            }

            let sample: Line = serde_json::from_str(sample).unwrap();
            let sample = sample.content;

            let parsed = AlertItem::parse(&sample).unwrap();
            all_parsed.push(parsed);
        }

        insta::assert_yaml_snapshot!(all_parsed);
    }

    #[test]
    fn test_ignores_invalid_xml() {
        insta::assert_yaml_snapshot!(AlertItem::parse(""), @r###"
        ---
        Err:
          XmlInvalid: the end of the document has been reached prematurely
        "###);

        // Missing event type
        insta::assert_yaml_snapshot!(AlertItem::parse(indoc::indoc!{r#"
            <EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
            <ipAddress>128.100.0.5</ipAddress>
            <portNo>80</portNo>
            <protocol>HTTP</protocol>
            <macAddress>ff:ff:ff:ff:ff:ff</macAddress>
            <channelID>1</channelID>
            <dateTime>2021-07-02T14:25:36+08:00</dateTime>
            <activePostCount>0</activePostCount>
            <eventState>inactive</eventState>
            <eventDescription>videoloss alarm</eventDescription>
            <channelName></channelName>
            <Extensions version="1.0" xmlns="urn:psialliance-org">
            <serialNumber xmlns="urn:selfextension:psiaext-ver10-xsd">DS-2CD2185FWD-I20180101AAWR111111111</serialNumber>
            <eventPush xmlns="urn:selfextension:psiaext-ver10-xsd">IO&amp;&amp;DS-2CD2185FWD-I20180101AAWR111111111,2021-07-02T14:25:36+08:00,1.0</eventPush>
            </Extensions>
            </EventNotificationAlert>
        "#}), @r###"
        ---
        Err:
          FieldMissing: eventType
        "###);

        // Outer fields are incorrect
        insta::assert_yaml_snapshot!(AlertItem::parse(indoc::indoc!{r#"
            <WrongOuter version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
            <ipAddress>128.100.0.5</ipAddress>
            <portNo>80</portNo>
            <protocol>HTTP</protocol>
            <macAddress>ff:ff:ff:ff:ff:ff</macAddress>
            <channelID>1</channelID>
            <dateTime>2021-07-02T14:25:36+08:00</dateTime>
            <activePostCount>0</activePostCount>
            <eventType>videoloss</eventType>
            <eventState>inactive</eventState>
            <eventDescription>videoloss alarm</eventDescription>
            <channelName></channelName>
            <Extensions version="1.0" xmlns="urn:psialliance-org">
            <serialNumber xmlns="urn:selfextension:psiaext-ver10-xsd">DS-2CD2185FWD-I20180101AAWR111111111</serialNumber>
            <eventPush xmlns="urn:selfextension:psiaext-ver10-xsd">IO&amp;&amp;DS-2CD2185FWD-I20180101AAWR111111111,2021-07-02T14:25:36+08:00,1.0</eventPush>
            </Extensions>
            </WrongOuter>
        "#}), @r###"
        ---
        Err:
          FieldMissing: EventNotificationAlert
        "###);

        // Post count not a number
        insta::assert_yaml_snapshot!(AlertItem::parse(indoc::indoc!{r#"
            <EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
            <ipAddress>128.100.0.5</ipAddress>
            <portNo>80</portNo>
            <protocol>HTTP</protocol>
            <macAddress>ff:ff:ff:ff:ff:ff</macAddress>
            <channelID>1</channelID>
            <dateTime>2021-07-02T14:25:36+08:00</dateTime>
            <activePostCount>a</activePostCount>
            <eventType>videoloss</eventType>
            <eventState>inactive</eventState>
            <eventDescription>videoloss alarm</eventDescription>
            <channelName></channelName>
            <Extensions version="1.0" xmlns="urn:psialliance-org">
            <serialNumber xmlns="urn:selfextension:psiaext-ver10-xsd">DS-2CD2185FWD-I20180101AAWR111111111</serialNumber>
            <eventPush xmlns="urn:selfextension:psiaext-ver10-xsd">IO&amp;&amp;DS-2CD2185FWD-I20180101AAWR111111111,2021-07-02T14:25:36+08:00,1.0</eventPush>
            </Extensions>
            </EventNotificationAlert>
        "#}), @r###"
        ---
        Err:
          NumberExpected:
            - activePostCount
            - invalid digit found in string
        "###);

        // Active not a bool
        insta::assert_yaml_snapshot!(AlertItem::parse(indoc::indoc!{r#"
            <EventNotificationAlert version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
            <ipAddress>128.100.0.5</ipAddress>
            <portNo>80</portNo>
            <protocol>HTTP</protocol>
            <macAddress>ff:ff:ff:ff:ff:ff</macAddress>
            <channelID>1</channelID>
            <dateTime>2021-07-02T14:25:36+08:00</dateTime>
            <activePostCount>0</activePostCount>
            <eventType>videoloss</eventType>
            <eventState>bad</eventState>
            <eventDescription>videoloss alarm</eventDescription>
            <channelName></channelName>
            <Extensions version="1.0" xmlns="urn:psialliance-org">
            <serialNumber xmlns="urn:selfextension:psiaext-ver10-xsd">DS-2CD2185FWD-I20180101AAWR111111111</serialNumber>
            <eventPush xmlns="urn:selfextension:psiaext-ver10-xsd">IO&amp;&amp;DS-2CD2185FWD-I20180101AAWR111111111,2021-07-02T14:25:36+08:00,1.0</eventPush>
            </Extensions>
            </EventNotificationAlert>
        "#}), @r###"
        ---
        Err:
          EventStateInvalid: bad
        "###);
    }
}
