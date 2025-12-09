mod config;
mod device;
mod firmware_generator;
mod mqtt_client;

use crate::config::Configuration;
use crate::device::SimulatedDevice;
use crate::firmware_generator::FirmwareGenerator;
use colored::Colorize;
use log::{error, info, LevelFilter};
use std::env;
use std::process;

#[tokio::main]
async fn main() {
    // Get config file path from command line arguments
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 {
        eprintln!("Usage: {} <config_file>", args[0]);
        process::exit(1);
    }

    let config_path = &args[1];

    // Load configuration
    let config = Configuration::from_file(config_path).unwrap_or_else(|e| {
        eprintln!("Failed to load configuration: {}", e);
        process::exit(1);
    });

    // Validate configuration
    if let Err(e) = config.validate() {
        eprintln!("Invalid configuration: {}", e);
        process::exit(1);
    }

    // Initialize logging with colors
    let log_level = match config.simulator.log_level.to_lowercase().as_str() {
        "error" => LevelFilter::Error,
        "warn" => LevelFilter::Warn,
        "info" => LevelFilter::Info,
        "debug" => LevelFilter::Debug,
        "trace" => LevelFilter::Trace,
        _ => LevelFilter::Info,
    };

    fern::Dispatch::new()
        .format(|out, message, record| {
            let level_string = match record.level() {
                log::Level::Error => "ERROR".red(),
                log::Level::Warn => "WARN".yellow(),
                log::Level::Info => "INFO".green(),
                log::Level::Debug => "DEBUG".blue(),
                log::Level::Trace => "TRACE".purple(),
            };
            out.finish(format_args!(
                "{} [{}] {}",
                chrono::Local::now().format("%Y-%m-%d %H:%M:%S"),
                level_string,
                message
            ))
        })
        .level(log_level)
        .chain(std::io::stdout())
        .apply()
        .unwrap();

    info!("ESP32 OTA Simulator starting");
    info!("Configuration loaded from: {}", config_path);
    info!("Number of devices: {}", config.simulator.num_devices);

    // Spawn firmware generator task
    let mut firmware_generator = FirmwareGenerator::new(
        config.firmware.storage_path.clone(),
        config.simulator.num_devices,
        config.simulator.device_id_prefix.clone(),
        config.firmware.initial_version.clone(),
        config.firmware.generation_interval_min,
        config.firmware.generation_interval_max,
    )
    .unwrap_or_else(|e| {
        error!("Failed to create firmware generator: {}", e);
        process::exit(1);
    });

    tokio::spawn(async move {
        firmware_generator.start_generation_task().await;
    });

    // Spawn simulated devices
    let mut device_handles = vec![];

    for i in 1..=config.simulator.num_devices {
        let device_id = format!("{}{:03}", config.simulator.device_id_prefix, i);
        let ota_port = config.simulator.base_ota_port + (i as u16 - 1);

        let device = SimulatedDevice::new(
            device_id.clone(),
            ota_port,
            config.mqtt.clone(),
            config.firmware.ota_password.clone(),
            config.firmware.initial_version.clone(),
            config.deep_sleep.min_sleep_seconds,
            config.deep_sleep.max_sleep_seconds,
            config.deep_sleep.max_wakeup_seconds,
        );

        let handle = tokio::spawn(async move {
            if let Err(e) = device.run().await {
                error!("[{}] Device error: {}", device_id, e);
            }
        });

        device_handles.push(handle);
    }

    info!("All devices spawned, running simulation...");

    // Wait for all device tasks
    for handle in device_handles {
        let _ = handle.await;
    }
}
