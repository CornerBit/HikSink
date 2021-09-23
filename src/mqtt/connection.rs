use super::manager;
use crate::{config::Config, hikapi::CameraEvent};
use rumqttc::{AsyncClient, Incoming, MqttOptions};
use tokio::sync::mpsc;
use tracing::{debug, error, info};

use std::time::Duration;

pub fn initiate_connection(config: &Config) -> Result<mpsc::Sender<CameraEvent>, String> {
    let (camera_tx, mut camera_rx) = mpsc::channel::<CameraEvent>(20);
    let mut manager = manager::Manager::new(
        config.camera.clone(),
        manager::MqttTopics::new(
            config.mqtt.base_topic.clone(),
            config.mqtt.home_assistant_topic.clone(),
        ),
    );

    let mut mqttoptions =
        MqttOptions::new("hik-sink", config.mqtt.address.clone(), config.mqtt.port);
    mqttoptions
        .set_keep_alive(5)
        .set_pending_throttle(Duration::from_millis(10));
    mqttoptions.set_credentials(config.mqtt.username.clone(), config.mqtt.password.clone());
    // We need to retain the session state between broker reboots so we don't lose our subscriptions
    mqttoptions.set_clean_session(false);
    mqttoptions.set_last_will(manager.mqtt_lwt().into());

    let (connection_notify_tx, mut connection_notify_rx) = mpsc::unbounded_channel::<()>();
    let (client, mut eventloop) = AsyncClient::new(mqttoptions, 10);

    // Launch the event loop as a task
    tokio::task::spawn(async move {
        loop {
            let event = eventloop.poll().await;
            match event {
                Ok(event) => match event {
                    rumqttc::Event::Incoming(Incoming::Publish(_)) => {
                        // Currently unused, but we can subscribe to topics to get messages here
                    }
                    rumqttc::Event::Incoming(Incoming::ConnAck(_)) => {
                        // Connection was established. Notify the client to send all discovery messages
                        info!("Connected to MQTT broker.");
                        let _ = connection_notify_tx.send(());
                    }
                    _ => {}
                },
                Err(e) => {
                    error!("MQTT Connection error encountered: {}", e);
                    tokio::time::sleep(Duration::from_secs(1)).await;
                }
            }
        }
    });

    // Launch the client as a task
    tokio::task::spawn(async move {
        loop {
            let messages = tokio::select! {
                camera_update = camera_rx.recv() => {
                    let camera_update = camera_update.expect("Camera event stream closed");
                    debug!(id=?camera_update.id, event=?camera_update.event, "Camera event");
                    manager.next_event(camera_update)
                }

                _ = connection_notify_rx.recv() => {
                    // Publish all discovery
                    manager.mqtt_connection_established()
                }
            };
            for message in messages {
                if let Err(e) = client
                    .publish(
                        message.topic,
                        message.qos.into(),
                        message.retain,
                        message.payload.render(),
                    )
                    .await
                {
                    error!("Unable to publish MQTT message: {}", e);
                }
            }
        }
    });

    Ok(camera_tx)
}
