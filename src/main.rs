mod config;
mod database;
mod firmware;
mod home_assistant;
mod mqtt;
mod mqtt_client;
mod ota_client;
mod pushover;
mod service;
mod version;
mod web;

use config::Configuration;
use fern::colors::{Color, ColoredLevelConfig};
use log::{error, info};

#[tokio::main]
async fn main() {
    // Load configuration first (before logging setup)
    let config_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "config.yaml".to_string());

    let config = match Configuration::from_file(&config_path) {
        Ok(config) => config,
        Err(e) => {
            eprintln!("Failed to load configuration: {}", e);
            std::process::exit(1);
        }
    };

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("Configuration validation failed: {}", e);
        std::process::exit(1);
    }

    // Setup logging with configured level
    if let Err(e) = setup_logging(&config) {
        eprintln!("Failed to setup logging: {}", e);
        std::process::exit(1);
    }

    info!("Starting OTA Service");
    info!("Configuration loaded from: {}", config_path);
    info!("Configuration validated successfully");

    // Start the service
    if let Err(e) = service::run(config, config_path).await {
        error!("Service error: {}", e);
        std::process::exit(1);
    }

    info!("OTA Service stopped");
}

/// Setup logging with colored terminal output and plain file output
fn setup_logging(config: &Configuration) -> Result<(), fern::InitError> {
    // Parse log level from config
    let log_level = match config.service.log_level.to_lowercase().as_str() {
        "trace" => log::LevelFilter::Trace,
        "debug" => log::LevelFilter::Debug,
        "info" => log::LevelFilter::Info,
        "warn" => log::LevelFilter::Warn,
        "error" => log::LevelFilter::Error,
        _ => log::LevelFilter::Info, // Default to Info if invalid
    };

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
        .level(log_level)
        .chain(std::io::stdout());

    // File logger without colors (if log file path is configured)
    let mut dispatch = fern::Dispatch::new().chain(console_logger);

    if let Some(log_path) = &config.service.log_file_path {
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
            .level(log_level)
            .chain(fern::log_file(log_path)?);

        dispatch = dispatch.chain(file_logger);
    }

    dispatch.apply()?;

    Ok(())
}
