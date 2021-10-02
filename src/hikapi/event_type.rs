use std::{fmt, str::FromStr};

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Hash, Clone)]
pub struct EventIdentifier {
    pub channel: Option<String>,
    pub event_type: EventType,
}

impl EventIdentifier {
    pub fn new(channel: Option<String>, event_type: EventType) -> Self {
        Self {
            channel,
            event_type,
        }
    }
}

impl fmt::Display for EventIdentifier {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        if let Some(ch) = &self.channel {
            write!(f, "CH{} ", ch)?;
        }
        write!(f, "{}", self.event_type.friendly_name())
    }
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Hash, Clone)]
pub enum EventType {
    Io,
    Motion,
    LineDetection,
    UnattendedBaggage,
    AttendedBaggage,
    RegionEntrance,
    RegionExiting,
    SceneChangeDetection,
    FieldDetection,
    FaceDetection,
    FaceSnap,
    AudioException,
    VideoLoss,
    Tamper,
    VideoMismatch,
    BadVideo,
    StorageDetection,
    RecordingFailure,
    DiskFull,
    DiskError,
    NicBroken,
    IpConflict,
    IllegalAccess,
    Unknown(String),
}

impl EventType {
    /// Returns `true` if the event type is [`VideoLoss`].
    ///
    /// [`VideoLoss`]: EventType::VideoLoss
    pub fn is_video_loss(&self) -> bool {
        matches!(self, Self::VideoLoss)
    }

    /// Friendly name for output to home assistant / discovery protocols
    pub fn friendly_name(&self) -> String {
        match self {
            EventType::Io => "I/O Port".to_string(),
            EventType::Motion => "Motion".to_string(),
            EventType::LineDetection => "Line Crossing".to_string(),
            EventType::UnattendedBaggage => "Unattended Baggage".to_string(),
            EventType::AttendedBaggage => "Attended Baggage".to_string(),
            EventType::RegionEntrance => "Region Entering".to_string(),
            EventType::RegionExiting => "Region Exiting".to_string(),
            EventType::SceneChangeDetection => "Scene Change".to_string(),
            EventType::FieldDetection => "Field Detection".to_string(),
            EventType::FaceDetection => "Face Detection".to_string(),
            EventType::FaceSnap => "Face Snapshot".to_string(),
            EventType::AudioException => "Audio Exception".to_string(),
            EventType::VideoLoss => "Video Loss".to_string(),
            EventType::Tamper => "Tamper".to_string(),
            EventType::VideoMismatch => "Video Mismatch".to_string(),
            EventType::BadVideo => "Bad Video".to_string(),
            EventType::StorageDetection => "Storage Detection".to_string(),
            EventType::RecordingFailure => "Recording Failure".to_string(),
            EventType::DiskFull => "Disk Full".to_string(),
            EventType::DiskError => "Disk Error".to_string(),
            EventType::NicBroken => "Network Card Broken".to_string(),
            EventType::IpConflict => "IP Address Conflict".to_string(),
            EventType::IllegalAccess => "Illegal Access".to_string(),
            EventType::Unknown(s) => s.clone(),
        }
    }

    /// Maps to a homeassistant binary sensor device class
    /// See https://www.home-assistant.io/integrations/binary_sensor/#device-class
    pub fn device_class(&self) -> Option<&str> {
        match self {
            EventType::Io => None,
            EventType::Motion
            | EventType::LineDetection
            | EventType::UnattendedBaggage
            | EventType::AttendedBaggage
            | EventType::RegionEntrance
            | EventType::RegionExiting
            | EventType::SceneChangeDetection
            | EventType::FieldDetection
            | EventType::FaceDetection
            | EventType::FaceSnap
            | EventType::AudioException
            | EventType::Unknown(_) => Some("motion"),
            EventType::VideoLoss
            | EventType::Tamper
            | EventType::VideoMismatch
            | EventType::BadVideo
            | EventType::StorageDetection
            | EventType::RecordingFailure
            | EventType::DiskFull
            | EventType::DiskError
            | EventType::NicBroken
            | EventType::IpConflict
            | EventType::IllegalAccess => Some("problem"),
        }
    }

    /// Maps to a home assistant design icon
    pub fn icon(&self) -> Option<&str> {
        match self {
            EventType::Io => Some("mdi:electric-switch"),
            EventType::Motion => None,
            EventType::LineDetection => None,
            EventType::UnattendedBaggage | EventType::AttendedBaggage => Some("mdi:bag-suitcase"),
            EventType::RegionEntrance => Some("mdi:import"),
            EventType::RegionExiting => Some("mdi:export"),
            EventType::SceneChangeDetection => None,
            EventType::FieldDetection => None,
            EventType::FaceDetection | EventType::FaceSnap => Some("mdi:face-recognition"),
            EventType::AudioException => Some("mdi:microphone"),
            EventType::Tamper => None,
            EventType::VideoLoss | EventType::VideoMismatch | EventType::BadVideo => {
                Some("mdi:camera-off")
            }
            EventType::StorageDetection
            | EventType::RecordingFailure
            | EventType::DiskFull
            | EventType::DiskError => Some("mdi:harddisk"),
            EventType::NicBroken | EventType::IpConflict => Some("mdi:lan-disconnect"),
            EventType::IllegalAccess => Some("mdi:account-alert"),
            EventType::Unknown(_) => None,
        }
    }
}

impl FromStr for EventType {
    type Err = String;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        // Hikvision is inconsistent with the case of the event types, even within the same camera model, so we ignore case.
        Ok(match s.to_ascii_lowercase().as_str() {
            "io" => EventType::Io,
            "vmd" => EventType::Motion,
            "linedetection" => EventType::LineDetection,
            "unattendedbaggage" => EventType::UnattendedBaggage,
            "attendedbaggage" => EventType::AttendedBaggage,
            "regionentrance" => EventType::RegionEntrance,
            "regionexiting" => EventType::RegionExiting,
            "scenechangedetection" => EventType::SceneChangeDetection,
            "fielddetection" => EventType::FieldDetection,
            "facedetection" => EventType::FaceDetection,
            "facesnap" => EventType::FaceSnap,
            "audioexception" => EventType::AudioException,
            "videoloss" => EventType::VideoLoss,
            "tamperdetection" => EventType::Tamper,
            "shelteralarm" => EventType::Tamper,
            "videomismatch" => EventType::VideoMismatch,
            "badvideo" => EventType::BadVideo,
            "storagedetection" => EventType::StorageDetection,
            "recordingfailure" => EventType::RecordingFailure,
            "diskfull" => EventType::DiskFull,
            "diskerror" => EventType::DiskError,
            "nicbroken" => EventType::NicBroken,
            "ipconflict" => EventType::IpConflict,
            "illaccess" => EventType::IllegalAccess,
            _ => {
                // Ensure the input is valid
                if s.chars().all(|c| c.is_ascii_alphanumeric()) {
                    EventType::Unknown(s.to_string())
                } else {
                    return Err(
                        "Event type contained non-alphabetic or non-numeric characters".into(),
                    );
                }
            }
        })
    }
}

impl ToString for EventType {
    fn to_string(&self) -> String {
        match self {
            EventType::Io => "Io".to_string(),
            EventType::Motion => "Motion".to_string(),
            EventType::LineDetection => "LineDetection".to_string(),
            EventType::UnattendedBaggage => "UnattendedBaggage".to_string(),
            EventType::AttendedBaggage => "AttendedBaggage".to_string(),
            EventType::RegionEntrance => "RegionEntrance".to_string(),
            EventType::RegionExiting => "RegionExiting".to_string(),
            EventType::SceneChangeDetection => "SceneChangeDetection".to_string(),
            EventType::FieldDetection => "FieldDetection".to_string(),
            EventType::FaceDetection => "FaceDetection".to_string(),
            EventType::FaceSnap => "FaceSnap".to_string(),
            EventType::AudioException => "AudioException".to_string(),
            EventType::VideoLoss => "VideoLoss".to_string(),
            EventType::Tamper => "Tamper".to_string(),
            EventType::VideoMismatch => "VideoMismatch".to_string(),
            EventType::BadVideo => "BadVideo".to_string(),
            EventType::StorageDetection => "StorageDetection".to_string(),
            EventType::RecordingFailure => "RecordingFailure".to_string(),
            EventType::DiskFull => "DiskFull".to_string(),
            EventType::DiskError => "DiskError".to_string(),
            EventType::NicBroken => "NicBroken".to_string(),
            EventType::IpConflict => "IpConflict".to_string(),
            EventType::IllegalAccess => "IllegalAccess".to_string(),
            EventType::Unknown(s) => s.clone(),
        }
    }
}

#[cfg(test)]
mod test {
    use super::EventType;

    #[test]
    fn test_parses_all_known() {
        let tests = [
            "IO",
            "VMD",
            "attendedBaggage",
            "audioexception",
            "badvideo",
            "diskerror",
            "diskfull",
            "faceSnap",
            "facedetection",
            "fielddetection",
            "illAccess",
            "ipconflict",
            "linedetection",
            "nicbroken",
            "recordingfailure",
            "regionEntrance",
            "regionExiting",
            "scenechangedetection",
            "storageDetection",
            "tamperdetection",
            "unattendedBaggage",
            "videoloss",
            "videomismatch",
        ];
        for t in tests {
            // Test normal case
            let res = t.parse();
            assert!(
                !matches!(res, Ok(EventType::Unknown(_))),
                "Invalid parse of normal case {:?}: {:?}",
                t,
                res
            );

            // Test lowercase
            let res = t.to_ascii_lowercase().parse();
            assert!(
                !matches!(res, Ok(EventType::Unknown(_))),
                "Invalid parse of lower case {:?}: {:?}",
                t,
                res
            );
        }
    }
    #[test]
    fn test_handles_unkown() {
        assert_eq!(
            "random".parse(),
            Ok(EventType::Unknown("random".to_string()))
        );
        assert!("random space".parse::<EventType>().is_err());
        assert!("line-detection".parse::<EventType>().is_err());
    }
}
