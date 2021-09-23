use tracing::{debug, info, trace};

#[macro_use]
extern crate quick_error;

mod config;
mod hikapi;
mod mqtt;

#[tokio::main]
async fn main() {
    let filter = tracing_subscriber::EnvFilter::from_default_env()
        // Default to INFO or higher.
        .add_directive(tracing_subscriber::filter::LevelFilter::DEBUG.into());
    let stdout_subscriber = tracing_subscriber::fmt()
        // Filter from user
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(stdout_subscriber).unwrap();

    info!("HikSink MQTT bridge running");

    let cfg_path = std::env::var("HIKSINK_CONFIG")
        .ok()
        .unwrap_or_else(|| "config.toml".to_string());
    debug!("Loading config file from {}...", cfg_path);
    let cfg = config::load_config(cfg_path).unwrap();
    trace!("Config: {:?}", cfg);

    // Connect to MQTT
    let tx = mqtt::initiate_connection(&cfg).unwrap();

    // Start connections to cameras
    for cam in cfg.camera {
        hikapi::run_camera(cam, tx.clone());
    }

    let () = futures::future::pending().await;
}
