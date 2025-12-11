mod config;
mod database;
mod firmware;
mod mqtt;
mod mqtt_client;
mod ota_client;
mod pushover;
mod service;
mod version;

use config::Configuration;
use fern::colors::{Color, ColoredLevelConfig};
use log::{error, info};

#[tokio::main]
async fn main() {
    // Setup logging
    if let Err(e) = setup_logging() {
        eprintln!("Failed to setup logging: {}", e);
        std::process::exit(1);
    }

    info!("Starting OTA Service");

    // Load configuration
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yaml".to_string());

    info!("Loading configuration from: {}", config_path);

    let config = match Configuration::from_file(&config_path) {
        Ok(config) => config,
        Err(e) => {
            error!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        error!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    info!("Configuration loaded and validated successfully");

    // Start the service
    if let Err(e) = service::run(config).await {
        error!("Service error: {}", e);
        std::process::exit(1);
    }

    info!("OTA Service stopped");
}

/// Setup logging with colored terminal output and plain file output
fn setup_logging() -> Result<(), fern::InitError> {
    // Define colors for different log levels
    let colors = ColoredLevelConfig::new()
        .error(Color::Red)
        .warn(Color::Yellow)
        .info(Color::Green)
        .debug(Color::Blue)
        .trace(Color::Magenta);

    // Console logger with colors
    let console_logger = fern::Dispatch::new()
        .format(move |out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                colors.color(record.level()),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(std::io::stdout());

    // File logger without colors
    let file_logger = fern::Dispatch::new()
        .format(|out, message, record| {
            out.finish(format_args!(
                "[{}] [{}] [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                record.level(),
                record.target(),
                message
            ))
        })
        .level(log::LevelFilter::Debug)
        .chain(fern::log_file("ota-service.log")?);

    // Combine both loggers
    fern::Dispatch::new()
        .chain(console_logger)
        .chain(file_logger)
        .apply()?;

    Ok(())
}
