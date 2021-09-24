use std::{collections::HashSet, path::Path};

use figment::{providers::Format, Figment};
use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct Config {
    pub system: ConfigSystem,
    pub camera: Vec<ConfigCamera>,
    pub mqtt: ConfigMqtt,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ConfigSystem {
    pub log_level: String,
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ConfigCamera {
    #[serde(skip_deserializing)]
    pub generated_id: String,
    pub name: String,
    pub address: String,
    pub port: Option<u16>,
    pub username: String,
    pub password: String,
}

impl ConfigCamera {
    pub fn identifier(&self) -> &str {
        self.generated_id.as_ref()
    }
}

#[derive(Debug, PartialEq, Serialize, Deserialize, Clone)]
pub struct ConfigMqtt {
    pub address: String,
    pub port: u16,
    pub username: String,
    pub password: String,
    pub base_topic: String,
    pub home_assistant_topic: String,
}

pub fn load_config_from_path(path: impl AsRef<Path>) -> Result<Config, String> {
    load_config(figment::providers::Toml::file(path))
}

pub fn load_config(data: impl figment::Provider) -> Result<Config, String> {
    let mut cfg: Config = Figment::new()
        .merge(figment::providers::Env::prefixed("HIKSINK_"))
        .merge(data)
        .extract()
        .map_err(|e| e.to_string())?;

    // Generate the camera ids
    for camera in &mut cfg.camera {
        // Only lowercase characters and _ allowed
        camera.generated_id = camera
            .name
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '_')
            .map(|c| {
                if c == ' ' {
                    '_'
                } else {
                    c.to_ascii_lowercase()
                }
            })
            .collect();
    }
    // Check that IDs are unique
    let mut ids = HashSet::new();
    for cam in &cfg.camera {
        let id: &str = cam.generated_id.as_ref();
        if ids.contains(&id) {
            return Err(format!(
                "Camera {} has duplicate ID: {}",
                cam.name,
                cam.identifier()
            ));
        }
        ids.insert(id);
    }
    Ok(cfg)
}

#[cfg(test)]
mod test {
    use figment::providers::Format;

    #[test]
    fn test_sample_config_valid() {
        const SAMPLE_CONFIG: &str = include_str!("../sample_config.toml");
        insta::assert_yaml_snapshot!(super::load_config(figment::providers::Toml::string(
            SAMPLE_CONFIG
        )));
    }
}
