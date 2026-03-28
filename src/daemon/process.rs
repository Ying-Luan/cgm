//! Daemon start and stop
//!
//! Provides start_daemon() and stop_daemon() functions.

use std::{
    fs, fs::File, os::unix::fs::PermissionsExt, path::Path, sync::Arc, thread, time::Duration,
};

use daemonize::Daemonize;
use nix::{
    sys::signal::{self, Signal},
    unistd::Pid,
};

use crate::{
    constants::{CGM_ERR_PATH, CGM_OUT_PATH, CGM_RUN_DIR, DB_PATH, JOB_LOG_FOLDER, PID_PATH},
    daemon::{
        scheduler::{FifoScheduler, GreedyScheduler, Scheduler, run_executor},
        server::Server,
    },
    db::DataBase,
    hardware::GpuManager,
    macros::{green, red},
    types::SchedulerKind,
};

impl From<SchedulerKind> for Box<dyn Scheduler> {
    fn from(kind: SchedulerKind) -> Self {
        match kind {
            SchedulerKind::Fifo => Box::new(FifoScheduler::new()),
            SchedulerKind::Greedy => Box::new(GreedyScheduler::new()),
        }
    }
}

/// Start daemon
///
/// Creates daemonize instance, forks child process, initializes database and GPU manager,
/// starts scheduler thread and socket listener.
///
/// # Arguments
///
/// * `gpu_indices` - GPU index list managed by CGM
/// * `interval` - Scheduling interval in seconds
/// * `scheduler` - Scheduler strategy
/// * `threshold` - GPU memory threshold (%)
pub fn start_daemon(
    gpu_indices: Vec<usize>,
    interval: u32,
    scheduler: SchedulerKind,
    threshold: u32,
) {
    fs::create_dir_all(CGM_RUN_DIR).ok();
    fs::create_dir_all(JOB_LOG_FOLDER).ok();
    fs::set_permissions(JOB_LOG_FOLDER, PermissionsExt::from_mode(0o755)).ok();

    let daemonize = Daemonize::new()
        .pid_file(PID_PATH)
        .stdout(File::create(CGM_OUT_PATH).unwrap())
        .stderr(File::create(CGM_ERR_PATH).unwrap());

    match daemonize.start() {
        Ok(_) => {
            let db = DataBase::open(Path::new(DB_PATH)).unwrap();
            let gpu_pool = GpuManager::new(gpu_indices, threshold);
            let scheduler_kind = format!("{:?}", scheduler);

            thread::spawn({
                let db = Arc::clone(&db);
                let gpu_pool_clone = Arc::clone(&gpu_pool);
                move || {
                    run_executor(db, gpu_pool_clone, interval, scheduler.into());
                }
            });

            println!(
                "{}",
                green!(
                    "Daemon started successfully with PID: {} and scheduler: {}",
                    std::process::id(),
                    scheduler_kind
                )
            );

            let server = Server::new(Arc::clone(&db), Arc::clone(&gpu_pool));
            server.run();
        }
        Err(e) => eprintln!("{}", red!("Failed to start daemon: {}", e)),
    }
}

/// Stop daemon
///
/// Reads PID file, sends SIGTERM signal to terminate process.
///
/// # Returns
///
/// Returns PID of terminated process.
pub fn stop_daemon() -> Result<i32, String> {
    if !Path::new(PID_PATH).exists() {
        return Err("Daemon not running (PID file not found)".to_string());
    }

    let pid_raw: i32 = fs::read_to_string(PID_PATH)
        .map_err(|e| format!("Failed to read PID file: {}", e))?
        .trim()
        .parse()
        .map_err(|_| "PID file content invalid")?;

    let pid = Pid::from_raw(pid_raw);

    let sig = Signal::SIGTERM;
    signal::kill(pid, sig).map_err(|e| format!("Failed to send SIGTERM: {}", e))?;

    let _ = fs::remove_file(PID_PATH);

    Ok(pid_raw)
}

/// Kill process and its entire process group
///
/// Sends SIGTERM to the entire process group, polls for exit,
/// sends SIGKILL if process doesn't exit within 5 seconds.
///
/// # Arguments
///
/// * `pid` - Process ID (the process group leader's pid)
pub fn kill_process(pid: u32) -> Result<(), String> {
    let pid_i32 = pid as i32;
    let pgid = Pid::from_raw(-pid_i32);

    if let Err(e) = signal::kill(pgid, Signal::SIGTERM) {
        return Err(format!(
            "Failed to send SIGTERM to process group {}: {}",
            pid, e
        ));
    }

    for _ in 0..50 {
        thread::sleep(Duration::from_millis(100));
        if signal::kill(Pid::from_raw(pid_i32), None).is_err() {
            return Ok(());
        }
    }

    if let Err(e) = signal::kill(pgid, Signal::SIGKILL) {
        return Err(format!(
            "Failed to send SIGKILL to process group {}: {}",
            pid, e
        ));
    }

    Ok(())
}
