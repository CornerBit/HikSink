use std::path::PathBuf;

use structopt::StructOpt;
use tracing::{info, trace};

#[macro_use]
extern crate quick_error;

mod config;
mod hikapi;
mod mqtt;

#[derive(Debug, StructOpt)]
#[structopt(name = "hik_sink", about = "Hiksink camera events to MQTT service.")]
struct CliArgs {
    #[structopt(
        parse(from_os_str),
        short = "c",
        long = "config",
        default_value = "config.toml",
        help = "Path to configuration file. See sample_config.toml for format.",
        env = "HIKSINK_CONFIG"
    )]
    config: PathBuf,
}

#[tokio::main]
async fn main() {
    let args = CliArgs::from_args();
    let cfg = config::load_config_from_path(args.config).unwrap();

    let filter = tracing_subscriber::EnvFilter::new(&cfg.system.log_level);
    let stdout_subscriber = tracing_subscriber::fmt()
        // Filter from user
        .with_env_filter(filter)
        .finish();
    tracing::subscriber::set_global_default(stdout_subscriber).unwrap();

    info!("HikSink MQTT bridge running");
    trace!("Config: {:?}", cfg);
    // Connect to MQTT
    let tx = mqtt::initiate_connection(&cfg).unwrap();

    // Start connections to cameras
    for cam in cfg.camera {
        hikapi::run_camera(cam, tx.clone());
    }

    let () = futures::future::pending().await;
}
