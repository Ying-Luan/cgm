//! Start daemon subcommand
//!
//! Check root permission, database compatibility, then start daemon.

use std::{fs, path::Path, process, thread, time::Duration};

use nvml_wrapper::Nvml;
use rusqlite::Connection;

use crate::{
    constants::DB_PATH,
    daemon::{is_daemon_running, start_daemon},
    db::check_db_compatible,
    macros::{green, red, yellow},
    os::require_root,
    types::SchedulerKind,
};

/// Start daemon
///
/// # Arguments
///
/// * `force` - Force start, deletes old database
/// * `gpus` - GPU list managed by CGM, comma-separated (e.g. "0,1,2,3") or "all" for all GPUs
/// * `interval` - Scheduling interval in seconds
/// * `scheduler` - Scheduler strategy
/// * `threshold` - GPU memory threshold (%), above this value considered externally occupied
pub fn run(force: bool, gpus: String, interval: u32, scheduler: SchedulerKind, threshold: u32) {
    // Only root user can start daemon
    require_root();

    // Check if daemon is already running
    if is_daemon_running() {
        println!("{}", yellow!("Daemon is already running."));
        return;
    }

    // Check if threshold is valid
    if threshold > 100 {
        eprintln!(
            "{}",
            red!("Invalid threshold value. Must be between 0 and 100.")
        );
        process::exit(1);
    }

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
                    eprintln!("Database schema incompatible. Use --force to recreate.");
                    process::exit(1);
                }
            }
            Err(e) => {
                eprintln!("{}", red!("Failed to open database: {}", e));
                process::exit(1);
            }
        }
    }

    // Parse gpus string "0,1,2,3" -> vec![0, 1, 2, 3]
    let gpu_indices: Vec<usize> = {
        let nvml = Nvml::init().expect("Failed to initialize NVML");
        let device_count = nvml
            .device_count()
            .expect("Failed to query NVML device count") as usize;
        if gpus == "all" {
            (0..device_count).collect()
        } else {
            gpus.split(',')
                .filter_map(|s| s.trim().parse().ok())
                .filter(|&id| id < device_count)
                .collect()
        }
    };

    if gpu_indices.is_empty() {
        eprintln!("{}", red!("No valid GPU indices provided."));
        process::exit(1);
    } else {
        println!("{}", green!("Managing GPUs: {:?}", gpu_indices));
    }

    // Start daemon
    start_daemon(gpu_indices, interval, scheduler, threshold);
    thread::sleep(Duration::from_secs(1));
    if is_daemon_running() {
        println!("{}", green!("Daemon started successfully."));
    } else {
        eprintln!("{}", red!("Failed to start daemon."));
    }
}
