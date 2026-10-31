//! Daemon start and stop
//!
//! Background startup, readiness acknowledgement, and coordinated signal shutdown.

use std::{
    fs::File,
    io::{BufRead, BufReader, Write},
    os::unix::{
        fs::OpenOptionsExt,
        net::{UnixListener, UnixStream},
    },
    path::Path,
    sync::{Arc, atomic::AtomicBool, mpsc},
    thread,
    time::Duration,
};

use daemonize::{Daemonize, Outcome};
use nix::{
    sys::signal::{self, SigSet, Signal},
    unistd::Pid,
};

use crate::{
    db::DataBase, hardware::GpuManager, macros::green, runtime::Runtime, types::SchedulerKind,
};

use super::{
    control,
    executor::Executor,
    scheduler::{FifoScheduler, GreedyScheduler, Scheduler},
    server::Server,
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
/// * `listener` - Socket bound before initialization, retained through shutdown.
/// * `runtime` - Startup mode, identity, and state paths.
/// * `gpu_indices` - GPU index list managed by CGM
/// * `interval` - Scheduling interval in seconds
/// * `scheduler` - Scheduler strategy
/// * `threshold` - GPU memory threshold (%)
///
/// # Returns
///
/// Success in the parent only after initialization and readiness acknowledgement.
pub(crate) fn start_daemon(
    listener: UnixListener,
    runtime: Runtime,
    gpu_indices: Vec<usize>,
    interval: u32,
    scheduler: SchedulerKind,
    threshold: u32,
) -> Result<(), String> {
    let paths = &runtime.paths;
    let open_log = |path: &Path| {
        File::options()
            .create(true)
            .append(true)
            .mode(0o600)
            .custom_flags(nix::libc::O_NOFOLLOW)
            .open(path)
    };
    let stdout =
        open_log(&paths.stdout).map_err(|e| format!("Failed to open daemon stdout: {e}"))?;
    let stderr =
        open_log(&paths.stderr).map_err(|e| format!("Failed to open daemon stderr: {e}"))?;
    let (parent, mut child) = UnixStream::pair().map_err(|e| e.to_string())?;
    parent
        .set_read_timeout(Some(Duration::from_secs(30)))
        .map_err(|error| error.to_string())?;
    let daemonize = Daemonize::new().umask(0o077).stdout(stdout).stderr(stderr);

    match daemonize.execute() {
        Outcome::Parent(result) => {
            drop(child);
            drop(listener);
            let status = result.map_err(|error| format!("Failed to daemonize: {error}"))?;
            if status.first_child_exit_code != 0 {
                return Err(format!(
                    "Daemonization failed: {}",
                    status.first_child_exit_code
                ));
            }
            let mut reader = BufReader::new(parent);
            let mut line = String::new();
            reader
                .read_line(&mut line)
                .map_err(|error| format!("Daemon initialization failed: {error}"))?;
            let pid: i32 = line
                .trim()
                .parse()
                .map_err(|_| "Daemon exited before initialization".to_string())?;
            if let Err((error, timed_out)) = read_ready(&mut reader) {
                if timed_out {
                    let _ = signal::kill(Pid::from_raw(pid), Signal::SIGKILL);
                }
                return Err(format!("Daemon initialization failed: {error}"));
            }
            writeln!(reader.get_mut(), "ACK")
                .map_err(|error| format!("Daemon readiness acknowledgement failed: {error}"))?;

            Ok(())
        }
        Outcome::Child(result) => {
            drop(parent);
            if let Err(error) = result {
                let _ = writeln!(child, "ERROR {error}");
                std::process::exit(1);
            }
            if writeln!(child, "{}", std::process::id()).is_err() {
                std::process::exit(1);
            }
            let initialized = std::panic::catch_unwind(|| {
                let db = DataBase::open(&paths.db).map_err(|error| error.to_string())?;
                db.lock()
                    .unwrap()
                    .recover_running()
                    .map_err(|error| format!("Failed to recover running jobs: {error}"))?;
                let gpu_pool = GpuManager::new(gpu_indices, threshold);

                Ok::<_, String>((db, gpu_pool))
            });
            let (db, gpu_pool) = match initialized {
                Ok(Ok(value)) => value,
                Ok(Err(error)) => {
                    let _ = writeln!(child, "ERROR {error}");
                    std::process::exit(1);
                }
                Err(_) => {
                    let _ = writeln!(child, "ERROR initialization panicked");
                    std::process::exit(1);
                }
            };
            let scheduler_kind = format!("{:?}", scheduler);
            let mut signals = SigSet::empty();
            signals.add(Signal::SIGTERM);
            signals.add(Signal::SIGINT);
            if let Err(error) = signals.thread_block() {
                let _ = writeln!(child, "ERROR Failed to block shutdown signals: {error}");
                std::process::exit(1);
            }
            let (sender, events) = mpsc::channel();
            let stopping = Arc::new(AtomicBool::new(false));
            let stopped = Arc::new(AtomicBool::new(false));

            let server = Server::new(
                Arc::clone(&db),
                Arc::clone(&gpu_pool),
                runtime,
                listener,
                sender.clone(),
                Arc::clone(&stopping),
                Arc::clone(&stopped),
            );
            let (begin, ready_to_run) = mpsc::channel();
            let executor_stopping = Arc::clone(&stopping);
            let executor_stopped = Arc::clone(&stopped);
            let executor =
                match thread::Builder::new()
                    .name("cgm-executor".into())
                    .spawn(move || {
                        if ready_to_run.recv().is_ok() {
                            Executor::new(
                                db,
                                gpu_pool,
                                interval,
                                scheduler.into(),
                                events,
                                executor_stopping,
                                executor_stopped,
                            )
                            .run();
                        }
                    }) {
                    Ok(executor) => executor,
                    Err(error) => {
                        let _ = writeln!(child, "ERROR Failed to start executor: {error}");
                        std::process::exit(1);
                    }
                };
            if writeln!(child, "READY").is_err() {
                std::process::exit(1);
            }
            let _ = child.set_read_timeout(Some(Duration::from_secs(30)));
            let mut acknowledgement = String::new();
            if BufReader::new(&child)
                .read_line(&mut acknowledgement)
                .is_err()
                || acknowledgement.trim() != "ACK"
            {
                std::process::exit(1);
            }
            drop(child);
            if begin.send(()).is_err() {
                std::process::exit(1);
            }
            let signal_sender = sender.clone();
            let signal_stopped = Arc::clone(&stopped);
            thread::spawn(move || {
                while signals.wait().is_ok() {
                    match control::stop(&signal_sender, &signal_stopped, true) {
                        Ok(()) => break,
                        Err(error) => eprintln!("Signal shutdown failed: {error}"),
                    }
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
            if let Err(error) = server.run() {
                eprintln!("Daemon server failed: {error}");
                if let Err(error) = control::stop(&sender, &stopped, true) {
                    eprintln!("Daemon cleanup failed: {error}");
                }
            }
            if executor.join().is_err() {
                eprintln!("Task executor panicked");
                std::process::exit(1);
            }
            std::process::exit(0);
        }
    }
}

/// Read the child's initialization result without treating EOF as readiness.
///
/// # Arguments
///
/// * `reader` - Parent side of the startup handshake.
///
/// # Returns
///
/// Success on READY, otherwise the failure reason and whether reading failed.
fn read_ready(reader: &mut impl BufRead) -> Result<(), (String, bool)> {
    let mut line = String::new();
    match reader.read_line(&mut line) {
        Ok(0) => Err(("child exited before READY".into(), false)),
        Ok(_) if line.trim() == "READY" => Ok(()),
        Ok(_) => Err((line.trim().to_string(), false)),
        Err(error) => Err((error.to_string(), true)),
    }
}

#[cfg(test)]
mod tests {
    use super::read_ready;
    use std::io::Cursor;

    #[test]
    fn readiness_requires_explicit_success() {
        assert!(read_ready(&mut Cursor::new("READY\n")).is_ok());
        assert!(read_ready(&mut Cursor::new("ERROR database\n")).is_err());
        assert!(read_ready(&mut Cursor::new("")).is_err());
    }
}
