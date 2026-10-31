//! Internal control requests for the executor.

use std::{
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Sender},
    },
    time::Duration,
};

/// Internal requests from the server and signal watcher.
pub(super) enum Event {
    /// Cancel a running task after rechecking its current state and owner.
    Cancel {
        /// Task ID.
        id: usize,
        /// Authenticated requester.
        username: String,
        /// Receives the result after termination.
        reply: Sender<Result<(), String>>,
    },
    /// Stop scheduling and optionally cancel all active tasks.
    Stop {
        /// Allow termination of active tasks.
        force: bool,
        /// Receives the result after all tasks finish.
        reply: Sender<Result<(), String>>,
    },
}

/// Request cancellation and wait for task finalization.
///
/// # Arguments
///
/// * `sender` - Executor event sender.
/// * `id` - Task ID.
/// * `username` - Authenticated requesting username.
///
/// # Returns
///
/// Success after cancellation, or an executor/task error.
pub(super) fn cancel(sender: &Sender<Event>, id: usize, username: &str) -> Result<(), String> {
    let (reply, result) = mpsc::channel::<Result<(), String>>();
    sender
        .send(Event::Cancel {
            id,
            username: username.to_string(),
            reply,
        })
        .map_err(|_| "Task executor is unavailable".to_string())?;
    result
        .recv_timeout(Duration::from_secs(30))
        .map_err(|error| format!("Cancellation acknowledgement failed: {error}"))?
}

/// Request shutdown and wait until no worker remains.
///
/// # Arguments
///
/// * `sender` - Executor event sender.
/// * `stopped` - Whether all tasks have finished and the executor has exited.
/// * `force` - Cancel running tasks instead of refusing shutdown.
///
/// # Returns
///
/// Success after cleanup, or an executor/task error.
pub(super) fn stop(
    sender: &Sender<Event>,
    stopped: &AtomicBool,
    force: bool,
) -> Result<(), String> {
    if stopped.load(Ordering::SeqCst) {
        return Ok(());
    }
    let (reply, result) = mpsc::channel::<Result<(), String>>();
    if sender.send(Event::Stop { force, reply }).is_err() {
        return if stopped.load(Ordering::SeqCst) {
            Ok(())
        } else {
            Err("Task executor is unavailable".into())
        };
    }
    match result.recv_timeout(Duration::from_secs(30)) {
        Ok(result) => result,
        Err(_) if stopped.load(Ordering::SeqCst) => Ok(()),
        Err(error) => Err(format!("Shutdown acknowledgement failed: {error}")),
    }
}
