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

/// Remove tasks whose wait thread has already finished.
///
/// # Arguments
///
/// * `tasks` - Registered running tasks.
fn prune_finished(tasks: &mut HashMap<usize, Task>) {
    tasks.retain(|_, task| {
        !task
            .worker
            .as_ref()
            .is_some_and(|worker| worker.is_finished())
    });
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

/// Schedule and coordinate tasks through a single internal control inbox.
///
/// # Arguments
///
/// * `db` - Shared database.
/// * `gpu_pool` - Existing GPU allocation API.
/// * `interval` - Seconds between scheduling cycles.
/// * `scheduler` - Scheduler algorithm to use for scheduling pending jobs.
/// * `events` - Sole receiver of control events.
/// * `stopping` - Whether the executor is stopping
/// * `stopped` - Whether the executor has stopped.
pub(super) fn run_executor(
    db: DbPool,
    gpu_pool: GpuPool,
    interval: u32,
    mut scheduler: Box<dyn Scheduler>,
    events: Receiver<Event>,
    stopping: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
) {
    let schedule_db = Arc::clone(&db);
    let schedule_gpu_pool = Arc::clone(&gpu_pool);
    let schedule = move |tasks: &mut HashMap<usize, Task>| {
        schedule_gpu_pool.lock().unwrap().refresh_state();
        let pending = schedule_db
            .lock()
            .unwrap()
            .get_jobs_by_status(JobStatus::Pending)
            .unwrap_or_default();
        let mut launch = |job, gpus| {
            launch_job(job, gpus, &schedule_db, &schedule_gpu_pool, tasks);
        };
        scheduler.schedule(Arc::clone(&schedule_gpu_pool), &mut launch, pending);
    };
    run_loop(db, gpu_pool, interval, events, stopping, stopped, schedule);
}

/// Process lifecycle events and schedule tasks.
///
/// # Arguments
///
/// * `db` - Task database used for current state and authorization checks.
/// * `gpu_pool` - GPU manager for tracking task allocations.
/// * `interval` - Seconds between calls to the synchronous scheduling callback.
/// * `events` - Lifecycle inbox for control requests.
/// * `stopping` - Whether the executor is stopping
/// * `stopped` - Whether the executor has stopped.
/// * `schedule` - Reserves resources and registers spawned tasks before returning.
fn run_loop(
    db: DbPool,
    gpu_pool: GpuPool,
    interval: u32,
    events: Receiver<Event>,
    stopping: Arc<AtomicBool>,
    stopped: Arc<AtomicBool>,
    mut schedule: impl FnMut(&mut HashMap<usize, Task>),
) {
    let grace = Duration::from_secs(5);
    let mut tasks: HashMap<usize, Task> = HashMap::new();
    let mut next_schedule = Instant::now();
    loop {
        prune_finished(&mut tasks);

        let event = if stopping.load(Ordering::SeqCst) {
            match events.recv() {
                Ok(event) => Some(event),
                Err(_) => return,
            }
        } else {
            match events.recv_timeout(next_schedule.saturating_duration_since(Instant::now())) {
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
                    let result = cancel_task(&db, &gpu_pool, &tasks, id, &username, grace)
                        .and_then(|()| {
                            tasks
                                .get_mut(&id)
                                .ok_or_else(|| {
                                    "Running task has no process registration".to_string()
                                })?
                                .join()
                        });
                    if result.is_ok() {
                        tasks.remove(&id);
                    }
                    let _ = reply.send(result);
                }
                Event::Stop { force, reply } => {
                    match stop_tasks(&gpu_pool, &mut tasks, &stopping, force, grace) {
                        Ok(()) => {
                            stopped.store(true, Ordering::SeqCst);
                            let _ = reply.send(Ok(()));
                            return;
                        }
                        Err(error) => {
                            let _ = reply.send(Err(error));
                        }
                    }
                }
            }
        }
        if !stopping.load(Ordering::SeqCst) && Instant::now() >= next_schedule {
            schedule(&mut tasks);
            next_schedule = Instant::now() + Duration::from_secs(u64::from(interval));
        }
    }
}

/// Cancel a running task after rechecking its state, owner, and process.
///
/// # Arguments
///
/// * `db` - Task database for current state and authorization checks.
/// * `gpu_pool` - GPU manager holding the registered process ID.
/// * `tasks` - Registered running tasks.
/// * `id` - Task ID.
/// * `username` - Authenticated requester.
/// * `grace` - SIGTERM grace period before escalating to SIGKILL.
///
/// # Returns
///
/// Success after the process group was terminated, or an error.
fn cancel_task(
    db: &DbPool,
    gpu_pool: &GpuPool,
    tasks: &HashMap<usize, Task>,
    id: usize,
    username: &str,
    grace: Duration,
) -> Result<(), String> {
    let job = db
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
    let task = tasks
        .get(&id)
        .ok_or_else(|| "Running task has no process registration".to_string())?;
    let pid = gpu_pool
        .lock()
        .unwrap()
        .get_first_pid_by_job_id(id)
        .ok_or_else(|| "Job is still starting; retry cancellation later".to_string())?;
    task.cancelled.store(true, Ordering::SeqCst);
    terminate(&[pid], grace)
}

/// Stop scheduling and optionally terminate all running tasks.
///
/// # Arguments
///
/// * `gpu_pool` - GPU manager holding registered process IDs.
/// * `tasks` - Registered running tasks.
/// * `stopping` - Whether scheduling has been disabled.
/// * `force` - Terminate running tasks instead of refusing shutdown.
/// * `grace` - SIGTERM grace period before escalating to SIGKILL.
///
/// # Returns
///
/// Success after all tasks finalized, or a refusal/error.
fn stop_tasks(
    gpu_pool: &GpuPool,
    tasks: &mut HashMap<usize, Task>,
    stopping: &AtomicBool,
    force: bool,
    grace: Duration,
) -> Result<(), String> {
    prune_finished(tasks);
    if !force {
        if !tasks.is_empty() {
            return Err("Running or finishing jobs exist; use --force".to_string());
        }
        stopping.store(true, Ordering::SeqCst);
    } else {
        let mut pids = Vec::new();
        for id in tasks.keys() {
            let pid = gpu_pool
                .lock()
                .unwrap()
                .get_first_pid_by_job_id(*id)
                .ok_or_else(|| "A job is still starting; retry stop later".to_string())?;
            pids.push(pid);
        }
        for task in tasks.values() {
            task.cancelled.store(true, Ordering::SeqCst);
        }
        stopping.store(true, Ordering::SeqCst);
        terminate(&pids, grace)?;
        for task in tasks.values_mut() {
            task.join()?;
        }
        tasks.clear();
    }

    Ok(())
}
