use minidom::Element;
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct DeviceInfo {
    pub device_name: String,
    pub device_id: String,
    pub model: String,
    pub serial_number: String,
    pub mac_address: String,
    pub firmware_version: String,
    pub firmware_release_date: String,
    pub device_type: String,
}

impl DeviceInfo {
    pub fn parse(s: &str) -> Result<DeviceInfo, DeviceInfoParseError> {
        let root: Element = s.parse()?;
        if root.name() != "DeviceInfo" {
            return Err(DeviceInfoParseError::RootNodeIncorrect(root.name().into()));
        }

        Ok(DeviceInfo {
            device_name: root
                .get_child("deviceName", minidom::NSChoice::Any)
                .ok_or_else(|| DeviceInfoParseError::FieldMissing("deviceName".to_string()))?
                .text(),
            device_id: root
                .get_child("deviceID", minidom::NSChoice::Any)
                .ok_or_else(|| DeviceInfoParseError::FieldMissing("deviceID".to_string()))?
                .text(),
            model: root
                .get_child("model", minidom::NSChoice::Any)
                .ok_or_else(|| DeviceInfoParseError::FieldMissing("model".to_string()))?
                .text(),
            serial_number: root
                .get_child("serialNumber", minidom::NSChoice::Any)
                .ok_or_else(|| DeviceInfoParseError::FieldMissing("serialNumber".to_string()))?
                .text(),
            mac_address: root
                .get_child("macAddress", minidom::NSChoice::Any)
                .ok_or_else(|| DeviceInfoParseError::FieldMissing("macAddress".to_string()))?
                .text(),
            firmware_version: root
                .get_child("firmwareVersion", minidom::NSChoice::Any)
                .ok_or_else(|| DeviceInfoParseError::FieldMissing("firmwareVersion".to_string()))?
                .text(),
            firmware_release_date: root
                .get_child("firmwareReleasedDate", minidom::NSChoice::Any)
                .ok_or_else(|| {
                    DeviceInfoParseError::FieldMissing("firmwareReleasedDate".to_string())
                })?
                .text(),
            device_type: root
                .get_child("deviceType", minidom::NSChoice::Any)
                .ok_or_else(|| DeviceInfoParseError::FieldMissing("deviceType".to_string()))?
                .text(),
        })
    }
}

quick_error! {
    #[derive(Debug)]
    pub enum DeviceInfoParseError {
        XmlInvalid(error: minidom::Error) {
            from()
        }
        RootNodeIncorrect(name: String) {
            display("Returned root node invalid: {}", name)
        }
        FieldMissing(field: String) {
            display("Field was expected but missing: {}", field)
        }
    }
}

#[cfg(test)]
mod test {
    use super::DeviceInfo;

    #[test]
    fn test_base_camera() {
        let parsed = DeviceInfo::parse(indoc::indoc! {r#"
            <?xml version="1.0" encoding="UTF-8"?>
            <DeviceInfo version="2.0" xmlns="http://www.hikvision.com/ver20/XMLSchema">
            <deviceName>PTZ</deviceName>
            <deviceID>7ccc4404-e05d-4376-8ebf-81127da67c11</deviceID>
            <deviceDescription>IPDome</deviceDescription>
            <deviceLocation>hangzhou</deviceLocation>
            <systemContact>Hikvision.China</systemContact>
            <model>DS-2DE4A425IW-DE</model>
            <serialNumber>DS-2DE4A425IW-DE20180101AAWRC52000000W</serialNumber>
            <macAddress>ff:ff:ff:ff:ff:ff</macAddress>
            <firmwareVersion>V5.5.71</firmwareVersion>
            <firmwareReleasedDate>build 180725</firmwareReleasedDate>
            <encoderVersion>V7.3</encoderVersion>
            <encoderReleasedDate>build 180320</encoderReleasedDate>
            <bootVersion>V1.3.4</bootVersion>
            <bootReleasedDate>100316</bootReleasedDate>
            <hardwareVersion>0x0</hardwareVersion>
            <deviceType>IPDome</deviceType>
            <telecontrolID>88</telecontrolID>
            <supportBeep>false</supportBeep>
            <supportVideoLoss>false</supportVideoLoss>
            <firmwareVersionInfo>B-R-R7-0</firmwareVersionInfo>
            </DeviceInfo>
        "#})
        .unwrap();
        insta::assert_yaml_snapshot!(parsed);
    }

    #[test]
    fn test_bad_camera() {
        assert!(DeviceInfo::parse("").is_err());
    }
}
