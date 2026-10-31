//! Start daemon subcommand
//!
//! Validate the selected mode, reserve the unique socket, and initialize daemon state.

use std::{fs, os::unix::fs::MetadataExt, path::Path};

use nix::unistd::Uid;
use nvml_wrapper::Nvml;
use rusqlite::Connection;

use crate::{
    client,
    config::{load_effective, validate_effective},
    constants::INSTALL_GUIDE_URL,
    daemon::start_daemon,
    db::check_db_compatible,
    macros::{green, yellow},
    runtime::{Runtime, RuntimePaths, socket},
    types::{DaemonMode, EffectiveStartConfig, GpuSelection, SchedulerKind},
};

use super::utils::exit_with_error;

/// Start daemon
///
/// # Arguments
///
/// * `user` - Select personal mode; false selects explicit system mode.
/// * `force` - Force start, deletes old database
/// * `gpus` - Optional GPU list managed by CGM, comma-separated (e.g. "0,1,2,3") or "all" for all GPUs
/// * `interval` - Optional scheduling interval in seconds
/// * `scheduler` - Optional scheduler strategy
/// * `threshold` - Optional GPU memory threshold (%), above this value considered externally occupied
pub(super) fn run(
    user: bool,
    force: bool,
    gpus: Option<String>,
    interval: Option<u32>,
    scheduler: Option<SchedulerKind>,
    threshold: Option<u32>,
) {
    if user == Uid::effective().is_root() {
        if user {
            exit_with_error("User mode cannot run as root.");
        } else {
            exit_with_error(&format!(
                concat!(
                    "System mode requires root.\n",
                    "Run sudo cgm start --system after a system installation.\n",
                    "Installation guide: {}"
                ),
                INSTALL_GUIDE_URL
            ));
        }
    }
    if !user {
        check_system_binary().unwrap_or_else(|error| exit_with_error(&error));
    }

    let listener = socket::bind().unwrap_or_else(|error| {
        if error.kind() == std::io::ErrorKind::AddrInUse {
            if let Ok((_, daemon)) = client::query_gpu_status() {
                exit_with_error(&format!(
                    "CGM is already running in {} mode: owner {} (UID {}), PID {}. Stop it before starting another daemon.",
                    daemon.mode, daemon.owner_name, daemon.owner_uid, daemon.pid
                ));
            }
            exit_with_error("CGM socket is occupied.");
        }
        exit_with_error(&format!("Failed to bind daemon socket: {}", error));
    });

    let config = merge_config(gpus, interval, scheduler, threshold)
        .unwrap_or_else(|error| exit_with_error(&error));
    let runtime = Runtime::new(if user {
        DaemonMode::User
    } else {
        DaemonMode::System
    })
    .unwrap_or_else(|error| exit_with_error(&error));
    let paths = &runtime.paths;
    paths
        // TODO:
        .secure(user)
        .unwrap_or_else(|error| exit_with_error(&error));

    // If database file exists, check compatibility or delete
    if force {
        force_cleanup(paths).unwrap_or_else(|error| exit_with_error(&error));
    } else if paths.db.exists() {
        match Connection::open(&paths.db) {
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
        match &config.gpus.value {
            GpuSelection::All => (0..device_count).collect(),
            GpuSelection::Allowed(ids) => ids
                .iter()
                .copied()
                .filter(|&id| id < device_count)
                .collect(),
        }
    };

    if gpu_indices.is_empty() {
        exit_with_error("No valid GPU indices provided.");
    } else {
        println!("{}", green!("Managing GPUs: {:?}", gpu_indices));
    }

    // Start daemon
    start_daemon(
        listener,
        runtime,
        gpu_indices,
        config.interval.value,
        config.scheduler.value,
        config.threshold.value,
    )
    .unwrap_or_else(|error| exit_with_error(&error));
    println!("{}", green!("Daemon started successfully."));
}

/// Reject system binaries or ancestor directories writable by non-root users.
///
/// # Returns
///
/// Success for an administrator-managed executable, or installation guidance.
fn check_system_binary() -> Result<(), String> {
    let executable = std::env::current_exe().map_err(|e| e.to_string())?;
    for path in executable.ancestors() {
        let metadata = fs::metadata(path).map_err(|e| format!("{}: {e}", path.display()))?;
        if metadata.uid() != 0 || metadata.mode() & 0o022 != 0 || metadata.mode() & 0o001 == 0 {
            return Err(format!(
                concat!(
                    "System mode requires a root-managed executable and parent directories accessible to all users: {}.\n",
                    "Installation guide: {}",
                ),
                path.display(),
                INSTALL_GUIDE_URL
            ));
        }
    }

    Ok(())
}

/// Remove database files and default job logs before rebuilding state.
///
/// # Arguments
///
/// * `paths` - Paths selected for this daemon's data prefix.
///
/// # Returns
///
/// Success after removing database files and default logs, or a filesystem error.
fn force_cleanup(paths: &RuntimePaths) -> Result<(), String> {
    for path in paths.sqlite_files() {
        match fs::remove_file(&path) {
            Ok(()) => println!("{}", yellow!("Removed {}.", path.display())),
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(format!("Failed to remove {}: {}", path.display(), error)),
        }
    }
    remove_job_logs(&paths.job_logs)
}

/// Remove persisted job logs when rebuilding the database.
///
/// # Arguments
///
/// * `log_dir` - Directory containing persisted job log files.
///
/// # Returns
///
/// Success after clearing the log directory, or a filesystem error.
fn remove_job_logs(log_dir: &Path) -> Result<(), String> {
    let entries = match fs::read_dir(log_dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(error.to_string()),
    };
    for entry in entries {
        let entry = entry.map_err(|error| error.to_string())?;
        let path = entry.path();
        let kind = entry.file_type().map_err(|error| error.to_string())?;
        let result = if kind.is_dir() {
            fs::remove_dir_all(&path)
        } else {
            fs::remove_file(&path)
        };
        result.map_err(|error| format!("Failed to remove {}: {}", path.display(), error))?;
    }

    Ok(())
}

/// Merge CLI parameters and the selected configuration into effective start options.
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
    let mut config = load_effective()?;
    if let Some(gpus) = gpus {
        config.start.gpus.value = GpuSelection::parse(&gpus)?;
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

#[cfg(test)]
mod tests {
    use std::{fs, time::SystemTime};

    use super::{force_cleanup, remove_job_logs};
    use crate::runtime::RuntimePaths;

    #[test]
    fn removes_job_logs_but_keeps_log_directory() {
        let log_dir = std::env::temp_dir().join(format!(
            "cgm-start-test-{}",
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        fs::create_dir_all(&log_dir).unwrap();
        fs::write(log_dir.join("job-1.log"), "old log").unwrap();

        remove_job_logs(&log_dir).unwrap();

        assert!(!log_dir.join("job-1.log").exists());
        assert!(log_dir.exists());
        fs::remove_dir_all(log_dir).unwrap();
    }

    #[test]
    fn force_cleanup_removes_database_and_job_logs() {
        let root = std::env::temp_dir().join(format!(
            "cgm-force-test-{}-{}",
            std::process::id(),
            SystemTime::now()
                .duration_since(SystemTime::UNIX_EPOCH)
                .unwrap()
                .as_nanos()
        ));
        let paths = RuntimePaths::from_root(root.clone());
        fs::create_dir_all(&paths.job_logs).unwrap();
        fs::write(paths.job_logs.join("job-1.log"), "old").unwrap();
        for path in paths.sqlite_files() {
            fs::write(path, "old").unwrap();
        }

        force_cleanup(&paths).unwrap();

        for path in paths.sqlite_files() {
            assert!(!path.exists());
        }
        assert!(!paths.job_logs.join("job-1.log").exists());
        assert!(paths.job_logs.exists());
        fs::remove_dir_all(root).unwrap();
    }
}
