//! Start daemon subcommand
//!
//! Check root permission, database compatibility, then start daemon.

use std::{fs, path::Path, thread, time::Duration};

use nvml_wrapper::Nvml;
use rusqlite::Connection;

use crate::{
    config::{load_global_effective, validate_effective},
    constants::DB_PATH,
    daemon::{is_daemon_running, start_daemon},
    db::check_db_compatible,
    macros::{green, yellow},
    os::require_root,
    types::{EffectiveStartConfig, SchedulerKind},
};

use super::utils::exit_with_error;

/// Start daemon
///
/// # Arguments
///
/// * `force` - Force start, deletes old database
/// * `gpus` - Optional GPU list managed by CGM, comma-separated (e.g. "0,1,2,3") or "all" for all GPUs
/// * `interval` - Optional scheduling interval in seconds
/// * `scheduler` - Optional scheduler strategy
/// * `threshold` - Optional GPU memory threshold (%), above this value considered externally occupied
pub(super) fn run(
    force: bool,
    gpus: Option<String>,
    interval: Option<u32>,
    scheduler: Option<SchedulerKind>,
    threshold: Option<u32>,
) {
    require_root();

    // Check if daemon is already running
    if is_daemon_running() {
        println!("{}", yellow!("Daemon is already running."));
        return;
    }

    let config = merge_config(gpus, interval, scheduler, threshold)
        .unwrap_or_else(|error| exit_with_error(&error));

    // If database file exists, check compatibility or delete
    if force {
        if Path::new(DB_PATH).exists() {
            fs::remove_file(DB_PATH).ok();
            fs::remove_file(format!("{}-wal", DB_PATH)).ok();
            fs::remove_file(format!("{}-shm", DB_PATH)).ok();
            println!("{}", yellow!("Removed old database."));
        }
    } else if Path::new(DB_PATH).exists() {
        match Connection::open(DB_PATH) {
            Ok(conn) => {
                if !check_db_compatible(&conn) {
                    exit_with_error("Database schema incompatible. Use --force to recreate.");
                }
            }
            Err(e) => exit_with_error(&format!("Failed to open database: {}", e)),
        }
    }

    // Parse gpus string "0,1,2,3" -> vec![0, 1, 2, 3]
    let gpu_indices: Vec<usize> = {
        let nvml = Nvml::init().expect("Failed to initialize NVML");
        let device_count = nvml
            .device_count()
            .expect("Failed to query NVML device count") as usize;
        if config.gpus.value == "all" {
            (0..device_count).collect()
        } else {
            config
                .gpus
                .value
                .split(',')
                .filter_map(|s| s.trim().parse().ok())
                .filter(|&id| id < device_count)
                .collect()
        }
    };

    if gpu_indices.is_empty() {
        exit_with_error("No valid GPU indices provided.");
    } else {
        println!("{}", green!("Managing GPUs: {:?}", gpu_indices));
    }

    // Start daemon
    start_daemon(
        gpu_indices,
        config.interval.value,
        config.scheduler.value,
        config.threshold.value,
    );
    thread::sleep(Duration::from_secs(1));
    if is_daemon_running() {
        println!("{}", green!("Daemon started successfully."));
    } else {
        exit_with_error("Failed to start daemon.");
    }
}

/// Merge CLI parameters and global configuration into effective start options.
///
/// # Arguments
///
/// * `gpus` - Optional GPU list managed by CGM, comma-separated (e.g. "0,1,2,3") or "all" for all GPUs
/// * `interval` - Optional scheduling interval in seconds
/// * `scheduler` - Optional scheduler strategy
/// * `threshold` - Optional GPU memory threshold (%)
///
/// # Returns
///
/// The effective start configuration or an error message.
fn merge_config(
    gpus: Option<String>,
    interval: Option<u32>,
    scheduler: Option<SchedulerKind>,
    threshold: Option<u32>,
) -> Result<EffectiveStartConfig, String> {
    let mut config = load_global_effective()?;
    if let Some(gpus) = gpus {
        config.start.gpus.value = gpus;
    }
    if let Some(interval) = interval {
        config.start.interval.value = interval;
    }
    if let Some(scheduler) = scheduler {
        config.start.scheduler.value = scheduler;
    }
    if let Some(threshold) = threshold {
        config.start.threshold.value = threshold;
    }
    validate_effective(&config)?;

    Ok(config.start)
}
