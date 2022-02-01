use std::{pin::Pin, time::Duration};

use super::{
    alert_parser::{AlertItem, AlertParseError},
    device_info::{DeviceInfo, DeviceInfoParseError},
    triggers_parser::{TriggerItem, TriggerParseError},
};
use crate::config::ConfigCamera;
use digest_auth::AuthContext;
use futures::StreamExt;
use reqwest::{header, Response};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc;
use tracing::{debug, error, info, info_span, trace, warn, Instrument};

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub struct CameraEvent {
    pub id: String,
    pub event: CameraEventType,
}

#[derive(Debug, PartialEq, Eq, Deserialize, Serialize, Clone)]
pub enum CameraEventType {
    Connected {
        info: DeviceInfo,
        triggers: Vec<TriggerItem>,
    },
    Disconnected {
        error: String,
    },
    Alert(AlertItem),
}

/// The camera manager handles reconnecting to a camera if it errors out and forwards all camera events to a shared queue
pub fn run_camera(cam: ConfigCamera, queue: mpsc::Sender<CameraEvent>) {
    let logging_span = info_span!("Camera coms", camera=%cam.name, id=%cam.identifier());
    tokio::spawn(
        async move {
            info!("Initiating camera connection...");
            let mut cam = reconnect_cam(cam, &queue).await;
            loop {
                let next = cam.next_event().await;
                match next {
                    Ok(alert) => {
                        let sent = queue
                            .send(CameraEvent {
                                id: cam.config.identifier().to_string(),
                                event: CameraEventType::Alert(alert),
                            })
                            .await;
                        if sent.is_err() {
                            debug!("Camera shutting down...");
                            return;
                        }
                    }
                    Err(e) => {
                        warn!("Camera errored: {}. Attempting reconnection...", e);
                        let _ = queue
                            .send(CameraEvent {
                                id: cam.config.identifier().to_string(),
                                event: CameraEventType::Disconnected {
                                    error: e.to_string(),
                                },
                            })
                            .await;
                        cam = reconnect_cam(cam.config, &queue).await;
                    }
                }
            }
        }
        .instrument(logging_span),
    );
}

async fn reconnect_cam(cam: ConfigCamera, queue: &mpsc::Sender<CameraEvent>) -> Camera {
    loop {
        match Camera::load(cam.clone()).await {
            Ok(c) => {
                info!("Camera connection established");
                let _ = queue
                    .send(CameraEvent {
                        id: c.config.identifier().to_string(),
                        event: CameraEventType::Connected {
                            triggers: c.triggers.clone(),
                            info: c.info.clone(),
                        },
                    })
                    .await;
                return c;
            }
            Err(e) => {
                error!("Error reconnecting to camera {}", e);
                let _ = queue
                    .send(CameraEvent {
                        id: cam.identifier().to_string(),
                        event: CameraEventType::Disconnected {
                            error: format!("Reconnection failure: {}", e),
                        },
                    })
                    .await;
                tokio::time::sleep(tokio::time::Duration::from_millis(3000)).await;
            }
        }
    }
}

pub struct Camera {
    pub config: ConfigCamera,
    pub info: DeviceInfo,
    pub triggers: Vec<TriggerItem>,
    stream: Pin<
        Box<
            dyn futures::Stream<
                    Item = Result<multipart_stream::Part, multipart_stream::parser::Error>,
                > + Send,
        >,
    >,
}

impl Camera {
    pub async fn load(config: ConfigCamera) -> Result<Camera, CameraError> {
        let client = reqwest::Client::builder()
            .tcp_keepalive(Duration::from_secs(60))
            .build()
            .map_err(CameraError::ConnectionError)?;
        let info = {
            let info_text = Self::camera_get_url("/ISAPI/System/deviceInfo", &client, &config)
                .await?
                .text()
                .await
                .map_err(CameraError::CameraInvalidResponseBody)?;
            DeviceInfo::parse(&info_text)?
        };

        let triggers = {
            let triggers_text = Self::camera_get_url("/ISAPI/Event/triggers", &client, &config)
                .await?
                .text()
                .await
                .map_err(CameraError::CameraInvalidResponseBody)?;
            TriggerItem::parse(&triggers_text)?
        };

        let stream = {
            let res =
                Self::camera_get_url("/ISAPI/Event/notification/alertStream", &client, &config)
                    .await?;
            let content_type: mime::Mime = res
                .headers()
                .get(header::CONTENT_TYPE)
                .ok_or_else(|| {
                    CameraError::StreamInvalid("Content type header missing on stream".into())
                })?
                .to_str()
                .map_err(|e| {
                    CameraError::StreamInvalid(format!("Content type header invalid string: {}", e))
                })?
                .parse()
                .map_err(|e| {
                    CameraError::StreamInvalid(format!("Content type invalid format: {}", e))
                })?;
            if content_type.type_() != "multipart" {
                return Err(CameraError::StreamInvalid(format!(
                    "Content type on stream should have been multipart. Instead it was {}",
                    content_type.type_()
                )));
            }
            let boundary = content_type.get_param(mime::BOUNDARY).ok_or_else(|| {
                CameraError::StreamInvalid("Multipart stream has no boundary set".to_string())
            })?;

            Box::pin(multipart_stream::parse(
                res.bytes_stream(),
                boundary.as_str(),
            ))
        };

        Ok(Camera {
            info,
            config,
            triggers,
            stream,
        })
    }

    /// Get a full http://<url></path>. e.g. path should be `/ISAPI/Event/triggers`
    async fn camera_get_url(
        path: &str,
        client: &reqwest::Client,
        config: &ConfigCamera,
    ) -> Result<Response, CameraError> {
        let url = format!(
            "http://{}{}{}",
            config.address,
            config.port.map(|p| format!(":{}", p)).unwrap_or_default(),
            path
        );
        get_url(client, &url, &config.username, &config.password).await
    }

    pub async fn next_event(&mut self) -> Result<AlertItem, CameraError> {
        let next = self
            .stream
            .next()
            .await
            .ok_or(CameraError::ConnectionClosed)?
            .map_err(|e| {
                CameraError::StreamInvalid(format!("Couldn't get next part of stream: {}", e))
            })?;
        let part_str = String::from_utf8(next.body.to_vec()).map_err(|e| {
            CameraError::StreamInvalid(format!("Stream returned non-UTF-8 text: {}", e))
        })?;
        trace!(cam=?self.config.identifier(), contents=?part_str, "Camera Alert");
        Ok(AlertItem::parse(&part_str)?)
    }
}

async fn get_url(
    client: &reqwest::Client,
    url: &str,
    username: &str,
    password: &str,
) -> Result<Response, CameraError> {
    let url = reqwest::Url::parse(url).map_err(|e| CameraError::UrlError(e.to_string()))?;
    let res = client
        .get(url.clone())
        .send()
        .await
        .map_err(CameraError::ConnectionError)?;
    if res.status() != 401 {
        return Err(CameraError::AuthenticationFailed(format!(
            "Could not get digest from server. Status code: {}",
            res.status()
        )));
    }

    let auth = {
        let resp_auth = res.headers().get_all(header::WWW_AUTHENTICATE);
        let resp_auth = resp_auth
            .iter()
            .map(|h| h.to_str())
            .filter_map(|h| h.ok())
            .find(|h| h.starts_with("Digest"))
            .ok_or_else(|| {
                CameraError::AuthenticationFailed("Digest not supported by camera.".into())
            })?;
        let context = AuthContext::new(username, password, url.path());
        let mut promt = digest_auth::parse(resp_auth).map_err(|e| {
            CameraError::AuthenticationFailed(format!(
                "Digest from camera could not be parsed: {}",
                e
            ))
        })?;
        promt.respond(&context).map_err(|e| {
            CameraError::AuthenticationFailed(format!("Unable to formulate digest response: {}", e))
        })?
    };

    let res = client
        .get(url)
        .header("Authorization", auth.to_header_string())
        .send()
        .await
        .map_err(CameraError::ConnectionError)?;
    if res.status() == 401 {
        return Err(CameraError::AuthenticationFailed(
            "Username or password incorrect".into(),
        ));
    }
    if res.status() == 403 {
        return Err(CameraError::AuthenticationFailed(
            "User does not have correct permissions. Ensure 'Notify Surveillance Center' is granted.".into(),
        ));
    }
    if res.status() != 200 {
        return Err(CameraError::AuthenticationFailed(format!(
            "Invalid status code after auth token sent: {:?}",
            res.status()
        )));
    }
    Ok(res)
}

quick_error! {
    #[derive(Debug)]
    pub enum CameraError {
        UrlError(error: String) {
            display("Unable to parse URL: {}", error)
        }
        ConnectionError(error: reqwest::Error) {
            display("Unable to connect to camera: {}", error)
            source(error)
        }
        CameraInvalidResponseBody(error: reqwest::Error) {
            display("Camera returned mangled response body: {}", error)
            source(error)
        }
        AuthenticationFailed (error: String) {
            display("Could not authenticate with camera: {}", error)
        }
        StreamInvalid(error: String) {
            display("Stream could not be resolved to a multipart form: {}", error)
        }
        ConnectionClosed {
            display("Camera closed connection")
        }
        DeviceInfoInvalid(error: DeviceInfoParseError) {
            from()
            source(error)
        }
        TriggersInvalid(error: TriggerParseError) {
            from()
            source(error)
        }
        AlertInvalid(error: AlertParseError) {
            from()
            source(error)
        }
    }
}
