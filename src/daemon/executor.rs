//! Job execution and lifecycle coordination
//!
//! Serialize scheduling and control requests while job threads start and wait.

use std::{
    collections::HashMap,
    fs::File,
    os::unix::process::CommandExt,
    process::Command,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::{self, Receiver},
    },
    thread::{self, JoinHandle},
    time::{Duration, Instant},
};

use nix::{
    errno::Errno,
    sys::signal::{self, Signal},
    unistd::{Pid, User},
};

use crate::{
    db::DbPool,
    hardware::GpuPool,
    types::{Job, JobStatus},
};

use super::{control::Event, scheduler::Scheduler};

/// Grace period between SIGTERM and SIGKILL for job process groups.
const GRACE: Duration = Duration::from_secs(5);

/// Execute single job
///
/// Opens the job's log file in append mode and redirects stdout/stderr to it,
/// starts subprocess with environment variables snapshot at submission time.
///
/// # Arguments
///
/// * `job` - Job to execute
/// * `gpus` - GPUs allocated to the job, passed via `CUDA_VISIBLE_DEVICES` environment variable
///
/// # Returns
///
/// Subprocess handle, can wait via `child.wait()`.
fn execute_job(job: &Job, gpus: &[usize]) -> std::io::Result<std::process::Child> {
    let user = User::from_name(&job.username)
        .map_err(std::io::Error::from)?
        .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "Job user not found"))?;
    // only open log file
    let log_file = File::options()
        .create(true)
        .append(true)
        .open(&job.log_path)?;
    let err_file = log_file.try_clone()?;

    let assigned_gpus = gpus
        .iter()
        .map(|i| i.to_string())
        .collect::<Vec<_>>()
        .join(",");

    let mut cmd = Command::new("bash");
    cmd.arg("-c")
        .arg(job.command.join(" "))
        .current_dir(&job.cwd)
        .env_clear()
        .env("CUDA_VISIBLE_DEVICES", assigned_gpus)
        .envs(&job.envs)
        .stdout(log_file)
        .stderr(err_file)
        .process_group(0);

    cmd.uid(user.uid.as_raw()).gid(user.gid.as_raw());

    // Shutdown signals are blocked in daemon threads, not in submitted jobs.
    // SAFETY: only signal-mask syscalls run between fork and exec.
    unsafe {
        cmd.pre_exec(|| {
            let mut empty = std::mem::zeroed::<nix::libc::sigset_t>();
            nix::libc::sigemptyset(&mut empty);
            if nix::libc::sigprocmask(nix::libc::SIG_SETMASK, &empty, std::ptr::null_mut()) != 0 {
                return Err(std::io::Error::last_os_error());
            }
            Ok(())
        });
    }

    cmd.spawn()
}

/// A running task and its blocking wait thread.
struct Task {
    /// Cancellation was requested before the terminal status update.
    cancelled: Arc<AtomicBool>,
    /// Sole child reaper and finalizer.
    worker: Option<JoinHandle<()>>,
}

impl Task {
    /// Join the wait thread.
    ///
    /// # Returns
    ///
    /// Success after the worker finished, or an error if it panicked.
    fn join(&mut self) -> Result<(), String> {
        match self.worker.take() {
            Some(worker) => worker
                .join()
                .map_err(|_| "Task worker panicked".to_string()),
            None => Ok(()),
        }
    }
}

/// Terminate process groups, escalating to SIGKILL after the grace period.
///
/// # Arguments
///
/// * `pids` - Process group leader IDs.
/// * `grace` - Time allowed after SIGTERM before group SIGKILL.
///
/// # Returns
///
/// Success after signalling, or an error on signal failure.
fn terminate(pids: &[u32], grace: Duration) -> Result<(), String> {
    let groups = pids
        .iter()
        .map(|pid| Pid::from_raw(-(*pid as i32)))
        .collect::<Vec<_>>();
    for &group in &groups {
        match signal::kill(group, Signal::SIGTERM) {
            Ok(()) | Err(Errno::ESRCH) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    let deadline = Instant::now() + grace;
    while Instant::now() < deadline {
        let mut live = false;
        for &group in &groups {
            match signal::kill(group, None) {
                Ok(()) => live = true,
                Err(Errno::ESRCH) => {}
                Err(error) => return Err(error.to_string()),
            }
        }
        if !live {
            return Ok(());
        }
        thread::sleep(Duration::from_millis(20));
    }
    for &group in &groups {
        match signal::kill(group, Signal::SIGKILL) {
            Ok(()) | Err(Errno::ESRCH) => {}
            Err(error) => return Err(error.to_string()),
        }
    }
    Ok(())
}

/// Allocate GPUs, start a job, and register its wait thread.
///
/// # Arguments
///
/// * `job` - Job to launch.
/// * `gpus` - GPUs allocated to the job.
/// * `db` - Database connection pool for updating job status.
/// * `gpu_pool` - GPU manager connection pool for updating GPU allocation and releasing after completion.
/// * `tasks` - Registered running tasks.
fn launch_job(
    job: Job,
    gpus: Vec<usize>,
    db: &DbPool,
    gpu_pool: &GpuPool,
    tasks: &mut HashMap<usize, Task>,
) {
    // Allocate GPUs and update database status to Running
    gpu_pool.lock().unwrap().allocate_batch(job.id, &gpus);
    let marked = db
        .lock()
        .unwrap()
        .update_status(job.id, JobStatus::Running, Some(&gpus));
    if let Err(error) = marked {
        eprintln!("Failed to mark job {} running: {error}", job.id);
        gpu_pool.lock().unwrap().release_batch(&gpus);
        return;
    }

    let id = job.id;
    let cancelled = Arc::new(AtomicBool::new(false));
    let worker_cancelled = Arc::clone(&cancelled);
    let worker_gpus = gpus.clone();
    let worker_db = Arc::clone(db);
    let worker_gpu_pool = Arc::clone(gpu_pool);
    let worker = thread::Builder::new()
        .name(format!("cgm-job-{id}"))
        .spawn(move || {
            let success = match execute_job(&job, &worker_gpus) {
                Ok(mut child) => {
                    // Record subprocess PID for subsequent monitoring and management
                    worker_gpu_pool
                        .lock()
                        .unwrap()
                        .update_pid_batch(&worker_gpus, child.id());

                    // Wait for subprocess to end, return final status based on exit code
                    child.wait().is_ok_and(|status| status.success())
                }
                Err(error) => {
                    eprintln!("Failed to start job {id}: {error}");
                    false
                }
            };
            let status = if worker_cancelled.load(Ordering::SeqCst) {
                JobStatus::Cancelled
            } else if success {
                JobStatus::Completed
            } else {
                JobStatus::Failed
            };
            if let Err(error) = worker_db.lock().unwrap().update_status(id, status, None) {
                eprintln!("Failed to persist job {id} status: {error}");
            }
            worker_gpu_pool.lock().unwrap().release_batch(&worker_gpus);
        });
    match worker {
        Ok(worker) => {
            tasks.insert(
                id,
                Task {
                    cancelled,
                    worker: Some(worker),
                },
            );
        }
        Err(error) => {
            eprintln!("Failed to create job {id} worker thread: {error}");
            if let Err(error) = db
                .lock()
                .unwrap()
                .update_status(id, JobStatus::Failed, None)
            {
                eprintln!("Failed to persist job {id} startup failure: {error}");
            }
            gpu_pool.lock().unwrap().release_batch(&gpus);
        }
    }
}

/// Coordinates job scheduling, execution, and control.
pub(super) struct Executor {
    /// Shared database.
    db: DbPool,
    /// GPU manager.
    gpu_pool: GpuPool,
    /// Scheduler strategy.
    scheduler: Box<dyn Scheduler>,
    /// Sole receiver of control events.
    events: Receiver<Event>,
    /// Whether the executor is stopping.
    stopping: Arc<AtomicBool>,
    /// Whether the executor has stopped.
    stopped: Arc<AtomicBool>,
    /// Running tasks keyed by job ID.
    tasks: HashMap<usize, Task>,
    /// Scheduling interval.
    interval: Duration,
}

impl Executor {
    /// Create an executor with no running tasks.
    ///
    /// # Arguments
    ///
    /// * `db` - Shared database.
    /// * `gpu_pool` - GPU manager.
    /// * `interval` - Seconds between scheduling cycles.
    /// * `scheduler` - Scheduler algorithm.
    /// * `events` - Sole receiver of control events.
    /// * `stopping` - Whether the executor is stopping.
    /// * `stopped` - Whether the executor has stopped.
    ///
    /// # Returns
    ///
    /// A new executor.
    pub(super) fn new(
        db: DbPool,
        gpu_pool: GpuPool,
        interval: u32,
        scheduler: Box<dyn Scheduler>,
        events: Receiver<Event>,
        stopping: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
    ) -> Self {
        Self {
            db,
            gpu_pool,
            scheduler,
            events,
            stopping,
            stopped,
            tasks: HashMap::new(),
            interval: Duration::from_secs(u64::from(interval)),
        }
    }

    /// Process lifecycle events and schedule tasks until shutdown.
    pub(super) fn run(mut self) {
        let mut next_schedule = Instant::now();
        loop {
            self.prune_finished();

            let event = if self.stopping.load(Ordering::SeqCst) {
                match self.events.recv() {
                    Ok(event) => Some(event),
                    Err(_) => return,
                }
            } else {
                match self
                    .events
                    .recv_timeout(next_schedule.saturating_duration_since(Instant::now()))
                {
                    Ok(event) => Some(event),
                    Err(mpsc::RecvTimeoutError::Timeout) => None,
                    Err(mpsc::RecvTimeoutError::Disconnected) => return,
                }
            };
            if let Some(event) = event {
                match event {
                    Event::Cancel {
                        id,
                        username,
                        reply,
                    } => {
                        let result = self.cancel(id, &username).and_then(|()| {
                            self.tasks
                                .get_mut(&id)
                                .ok_or_else(|| {
                                    "Running task has no process registration".to_string()
                                })?
                                .join()
                        });
                        if result.is_ok() {
                            self.tasks.remove(&id);
                        }
                        let _ = reply.send(result);
                    }
                    Event::Stop { force, reply } => match self.stop(force) {
                        Ok(()) => {
                            self.stopped.store(true, Ordering::SeqCst);
                            let _ = reply.send(Ok(()));
                            return;
                        }
                        Err(error) => {
                            let _ = reply.send(Err(error));
                        }
                    },
                }
            }
            if !self.stopping.load(Ordering::SeqCst) && Instant::now() >= next_schedule {
                self.schedule();
                next_schedule = Instant::now() + self.interval;
            }
        }
    }

    /// Schedule pending jobs once.
    fn schedule(&mut self) {
        self.gpu_pool.lock().unwrap().refresh_state();
        let pending = self
            .db
            .lock()
            .unwrap()
            .get_jobs_by_status(JobStatus::Pending)
            .unwrap_or_default();
        let mut launch =
            |job, gpus| launch_job(job, gpus, &self.db, &self.gpu_pool, &mut self.tasks);
        self.scheduler
            .schedule(Arc::clone(&self.gpu_pool), &mut launch, pending);
    }

    /// Cancel a running task after rechecking its state, owner, and process.
    ///
    /// # Arguments
    ///
    /// * `id` - Task ID.
    /// * `username` - Authenticated requester.
    ///
    /// # Returns
    ///
    /// Success after the process group was terminated, or an error.
    fn cancel(&self, id: usize, username: &str) -> Result<(), String> {
        let job = self
            .db
            .lock()
            .unwrap()
            .get_job_by_id(id)
            .map_err(|error| error.to_string())?
            .ok_or_else(|| format!("Job {id} not found"))?;
        if username != "root" && username != job.username {
            return Err("Permission denied".to_string());
        }
        if job.status != JobStatus::Running {
            return Err("Job has ended, cannot cancel".to_string());
        }
        let task = self
            .tasks
            .get(&id)
            .ok_or_else(|| "Running task has no process registration".to_string())?;
        let pid = self
            .gpu_pool
            .lock()
            .unwrap()
            .get_first_pid_by_job_id(id)
            .ok_or_else(|| "Job is still starting; retry cancellation later".to_string())?;
        task.cancelled.store(true, Ordering::SeqCst);
        terminate(&[pid], GRACE)
    }

    /// Stop scheduling and optionally terminate all running tasks.
    ///
    /// # Arguments
    ///
    /// * `force` - Terminate running tasks instead of refusing shutdown.
    ///
    /// # Returns
    ///
    /// Success after all tasks finalized, or a refusal/error.
    fn stop(&mut self, force: bool) -> Result<(), String> {
        self.prune_finished();
        if !force {
            if !self.tasks.is_empty() {
                return Err("Running or finishing jobs exist; use --force".to_string());
            }
            self.stopping.store(true, Ordering::SeqCst);
        } else {
            let mut pids = Vec::new();
            for id in self.tasks.keys() {
                let pid = self
                    .gpu_pool
                    .lock()
                    .unwrap()
                    .get_first_pid_by_job_id(*id)
                    .ok_or_else(|| "A job is still starting; retry stop later".to_string())?;
                pids.push(pid);
            }
            for task in self.tasks.values() {
                task.cancelled.store(true, Ordering::SeqCst);
            }
            self.stopping.store(true, Ordering::SeqCst);
            terminate(&pids, GRACE)?;
            for task in self.tasks.values_mut() {
                task.join()?;
            }
            self.tasks.clear();
        }

        Ok(())
    }

    /// Remove tasks whose wait thread has already finished.
    fn prune_finished(&mut self) {
        self.tasks.retain(|_, task| {
            !task
                .worker
                .as_ref()
                .is_some_and(|worker| worker.is_finished())
        });
    }
}
