//! Job scheduler
//!
//! Read pending jobs from database, allocate GPUs, start subprocess to execute.

mod fifo;
mod greedy;

pub use fifo::FifoScheduler;
pub use greedy::GreedyScheduler;

use std::{
    fs::File, os::unix::process::CommandExt, process::Command, sync::Arc, thread, time::Duration,
};

use nix::unistd::User;

use crate::{
    db::DbPool,
    hardware::GpuPool,
    types::{Job, JobStatus},
};

/// Scheduler trait, implemented by different scheduling algorithms
pub trait Scheduler {
    /// Schedule pending jobs, allocate GPUs and launch subprocesses
    ///
    /// # Arguments
    ///
    /// * `pending_jobs` - List of pending jobs to schedule
    /// * `db` - Database connection pool for updating job status
    /// * `gpu_pool` - GPU manager connection pool for allocating and releasing GPUs
    fn schedule(&mut self, pending_jobs: Vec<Job>, db: DbPool, gpu_pool: GpuPool);
}

/// Execute single job
///
/// Creates log file and redirects stdout/stderr to `/tmp/cgm/logs/job-{id}.log`,
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

    // Get user UID and GID
    let (uid, gid) = User::from_name(&job.username)
        .ok()
        .flatten()
        .map(|u| (u.uid, u.gid))
        .unzip();

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

    // If UID and GID obtained, set subprocess user and group
    if let (Some(uid), Some(gid)) = (uid, gid) {
        cmd.uid(uid.into()).gid(gid.into());
    }

    cmd.spawn()
}

/// Launch job by allocating GPUs, updating database status and starting subprocess in new thread
///
/// # Arguments
///
/// * `job` - Job to launch
/// * `gpus` - GPUs allocated to the job
/// * `db` - Database connection pool for updating job status
/// * `gpu_pool` - GPU manager connection pool for updating GPU allocation and releasing after completion
fn launch_job(job: Job, gpus: Vec<usize>, db: DbPool, gpu_pool: GpuPool) {
    // Allocate GPUs and update database status to Running
    {
        let mut gpu_pool = gpu_pool.lock().unwrap();
        gpu_pool.allocate_batch(job.id, &gpus);
    }
    {
        let db = db.lock().unwrap();
        db.update_status(job.id, JobStatus::Running).ok();
    }

    let db_clone = Arc::clone(&db);
    let gpu_pool_clone = Arc::clone(&gpu_pool);
    let gpus_to_release = gpus.clone();
    let job_id = job.id;

    // Execute job in new thread, update status and release GPUs after completion
    thread::spawn(move || {
        // Execute job, record log and monitor subprocess status
        let final_status = match execute_job(&job, &gpus) {
            Ok(mut child) => {
                // Record subprocess PID for subsequent monitoring and management
                let pid = child.id();
                {
                    let mut gpu_pool = gpu_pool_clone.lock().unwrap();
                    gpu_pool.update_pid_batch(&gpus, pid);
                }

                // Wait for subprocess to end, return final status based on exit code
                child
                    .wait()
                    .map(|s| {
                        if s.success() {
                            JobStatus::Completed
                        } else {
                            JobStatus::Failed
                        }
                    })
                    .unwrap_or(JobStatus::Failed)
            }
            Err(_) => JobStatus::Failed,
        };

        // Update job status in database and release allocated GPUs
        {
            let db = db_clone.lock().unwrap();
            db.update_status(job_id, final_status).ok();
        }
        {
            let mut gpu_pool = gpu_pool_clone.lock().unwrap();
            gpu_pool.release_batch(&gpus_to_release);
        }
    });
}

/// Main scheduler loop
///
/// # Arguments
///
/// * `db` - Database connection pool
/// * `gpu_pool` - GPU manager connection pool
/// * `scheduler` - Scheduler algorithm to use for scheduling pending jobs
/// * `interval` - Sleep interval in seconds between scheduling cycles
pub fn run_executor(
    db: DbPool,
    gpu_pool: GpuPool,
    interval: u32,
    mut scheduler: Box<dyn Scheduler>,
) {
    loop {
        // Refresh GPU state, detect external occupation and error state
        {
            let mut gpu_pool = gpu_pool.lock().unwrap();
            gpu_pool.refresh_state();
        }

        // Get all pending jobs
        let pending_jobs = {
            let db = db.lock().unwrap();
            db.get_jobs_by_status(JobStatus::Pending)
                .unwrap_or_default()
        };

        scheduler.schedule(pending_jobs, Arc::clone(&db), Arc::clone(&gpu_pool));

        // Sleep after each scheduling cycle to avoid too frequent database queries and GPU state refresh
        thread::sleep(Duration::from_secs(interval as u64));
    }
}
