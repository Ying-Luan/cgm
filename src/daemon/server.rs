//! Socket communication server
//!
//! Listen on Unix Domain Socket, receive and handle client requests.

use std::{
    fs::{self, File},
    io::Read,
    os::unix::net::{UnixListener, UnixStream},
    path::Path,
    sync::{
        Arc,
        atomic::{AtomicBool, Ordering},
        mpsc::Sender,
    },
    time::Duration,
};

use nix::unistd::{Uid, User};

use crate::{
    daemon::control::{self, Event},
    db::DbPool,
    hardware::GpuPool,
    macros::{green, red},
    runtime::{Runtime, authorize, connection_uid, socket},
    types::{
        DaemonMode, DaemonState, DeleteTarget, GpuSelection, Job, JobInfo, JobPrintInfo,
        JobRequest, JobStatus, Request, RerunContext, Response,
    },
};

/// Server handling client requests via Unix Domain Socket
///
/// Owns handles to shared database and GPU pool, dispatches incoming
/// requests to appropriate handlers.
pub(crate) struct Server {
    /// Database connection pool for job persistence
    queue: DbPool,
    /// GPU pool for resource allocation tracking
    gpu_pool: GpuPool,
    /// Startup-selected identity and paths for status metadata.
    runtime: Runtime,
    /// Pre-bound abstract socket served until shutdown completes.
    listener: UnixListener,
    /// Internal task control channel.
    sender: Sender<Event>,
    /// Whether the executor is stopping.
    stopping: Arc<AtomicBool>,
    /// Whether the executor has stopped.
    stopped: Arc<AtomicBool>,
}

impl Server {
    /// Create a new Server instance
    ///
    /// # Arguments
    ///
    /// * `queue` - Database connection pool for job CRUD operations
    /// * `gpu_pool` - GPU pool for tracking GPU allocation state
    /// * `runtime` - Startup-selected mode, identity, and paths.
    /// * `listener` - The unique abstract socket held until shutdown completes.
    /// * `sender` - Task executor channel used for cancellation and shutdown.
    /// * `stopping` - Whether the executor is stopping.
    /// * `stopped` - Whether the executor has stopped.
    ///
    /// # Returns
    ///
    /// A new Server instance ready to run and handle requests
    pub(super) fn new(
        queue: DbPool,
        gpu_pool: GpuPool,
        runtime: Runtime,
        listener: UnixListener,
        sender: Sender<Event>,
        stopping: Arc<AtomicBool>,
        stopped: Arc<AtomicBool>,
    ) -> Self {
        Self {
            queue,
            gpu_pool,
            runtime,
            listener,
            sender,
            stopping,
            stopped,
        }
    }

    /// Serve the pre-bound listener until the executor finishes shutdown.
    ///
    /// # Returns
    ///
    /// Success after shutdown, or a listener configuration error.
    pub(super) fn run(&self) -> Result<(), String> {
        self.listener
            .set_nonblocking(true)
            .map_err(|error| error.to_string())?;
        println!("{}", green!("Daemon listening on abstract socket"));
        while !self.stopped.load(Ordering::SeqCst) {
            match self.listener.accept() {
                Ok((stream, _)) => self.handle_connection(stream),
                Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                    std::thread::sleep(Duration::from_millis(50));
                }
                Err(error) => eprintln!("{}", red!("{}", error)),
            }
        }

        Ok(())
    }

    /// Process a single client connection
    ///
    /// Reads JSON request from client, dispatches to handler, writes response.
    ///
    /// # Arguments
    ///
    /// * `stream` - Unix stream connected to client
    fn handle_connection(&self, mut stream: UnixStream) {
        let _ = stream.set_read_timeout(Some(Duration::from_secs(2)));
        let _ = stream.set_write_timeout(Some(Duration::from_secs(2)));
        let caller = match connection_uid(&stream) {
            Ok(uid) => uid,
            Err(error) => {
                let _ = serde_json::to_writer(
                    &stream,
                    &Response::Error {
                        message: format!("Failed to obtain caller credentials: {}", error),
                    },
                );
                return;
            }
        };

        // Read client request
        let mut buffer = String::new();
        if let Err(error) = stream.read_to_string(&mut buffer) {
            eprintln!("{}", red!("Failed to read request: {}", error));
            return;
        }

        // Parse and handle request
        let response = match serde_json::from_str::<Request>(&buffer) {
            Ok(req) => {
                println!("{}", green!("Received request: {:?}", req));
                match authorize(req, caller, Uid::effective()) {
                    Ok((req, username)) => {
                        let foreign_status = matches!(req, Request::Status)
                            && !Uid::effective().is_root()
                            && caller != Uid::effective()
                            && !caller.is_root();
                        let mut response = self.dispatch(req, &username);
                        if foreign_status && let Response::Status { gpus, .. } = &mut response {
                            gpus.clear();
                        }
                        response
                    }
                    Err(message) => Response::Error { message },
                }
            }
            Err(error) => Response::Error {
                message: format!("Failed to parse request: {}", error),
            },
        };

        // Write response back to client
        if let Err(error) = serde_json::to_writer(&stream, &response) {
            eprintln!("{}", red!("Failed to write response: {}", error));
        }
    }

    /// Route request to appropriate handler based on request type
    ///
    /// # Arguments
    ///
    /// * `req` - Parsed Request enum variant
    ///
    /// # Returns
    ///
    /// Response generated by the corresponding handler function
    fn dispatch(&self, req: Request, username: &str) -> Response {
        if self.stopping.load(Ordering::SeqCst)
            && matches!(req, Request::Submit { .. } | Request::Rerun { .. })
        {
            return Response::Error {
                message: "Daemon is stopping; new jobs are not accepted".into(),
            };
        }
        match req {
            Request::Stop { force } => self.handle_stop(force),
            Request::Submit { job } => self.handle_submit(job),
            Request::Rerun {
                id,
                context,
                allowed_gpus,
                dependencies,
                required_gpus,
                log_path,
                username: _,
            } => self.handle_rerun(
                id,
                context,
                allowed_gpus,
                dependencies,
                required_gpus,
                log_path,
                username,
            ),
            Request::Cancel {
                id,
                username: _,
                force,
            } => self.handle_cancel(id, username, force),
            Request::Delete { target } => self.handle_delete(target),
            Request::Status => self.handle_status(),
            Request::List { limit, .. } => self.handle_list(username, limit),
            Request::Info { id, .. } => self.handle_info(id, username),
            Request::Log { id, .. } => self.handle_log(id, username),
        }
    }

    /// Ask the executor to stop.
    ///
    /// # Arguments
    ///
    /// * `force` - Terminate running tasks and wait for cleanup before stopping.
    ///
    /// # Returns
    ///
    /// A stop acknowledgement only after task finalization, or a refusal/error.
    fn handle_stop(&self, force: bool) -> Response {
        match control::stop(&self.sender, &self.stopped, force) {
            Ok(()) => Response::Stop,
            Err(message) => Response::Error { message },
        }
    }

    /// Handle job submission request
    ///
    /// Assigns a new job ID and returns the persisted details.
    ///
    /// # Arguments
    ///
    /// * `req` - Job request containing command, GPU count, environment, etc.
    ///
    /// # Returns
    ///
    /// Response containing the new job details, or an error message on failure.
    fn handle_submit(&self, req: JobRequest) -> Response {
        match self.create_job(req) {
            Ok(job) => Response::Submit { job: Box::new(job) },
            Err(message) => Response::Error { message },
        }
    }

    /// Handle job rerun request
    ///
    /// Creates a new job from an existing job, preserving the source job.
    ///
    /// # Arguments
    ///
    /// * `id` - Source job ID
    /// * `context` - Current terminal values overriding the saved context
    /// * `allowed_gpus` - Optional allowed GPU ID override
    /// * `dependencies` - Optional dependency override
    /// * `required_gpus` - Optional GPU count override
    /// * `log_path` - Optional log path override
    /// * `username` - User requesting the rerun
    ///
    /// # Returns
    ///
    /// Response containing the new job details, or an error message.
    fn handle_rerun(
        &self,
        id: usize,
        context: RerunContext,
        allowed_gpus: Option<GpuSelection>,
        dependencies: Option<Vec<usize>>,
        required_gpus: Option<usize>,
        log_path: Option<String>,
        username: &str,
    ) -> Response {
        let job = match self.get_job_with_permission(id, username) {
            Ok(job) => job,
            Err(message) => return Response::Error { message },
        };

        if context.envs.is_some() && username == "root" && job.username != "root" {
            return Response::Error {
                message: "Cannot use root's current environment for another user's job".to_string(),
            };
        }

        let request = JobRequest {
            username: job.username,
            allowed_gpus: allowed_gpus.unwrap_or(job.allowed_gpus),
            command: job.command,
            dependencies: dependencies.unwrap_or_default(),
            required_gpus: required_gpus.unwrap_or(job.required_gpus),
            envs: context.envs.unwrap_or(job.envs),
            log_path,
            cwd: context.cwd.unwrap_or(job.cwd),
        };

        match self.create_job(request) {
            Ok(job) => Response::Rerun { job: Box::new(job) },
            Err(message) => Response::Error { message },
        }
    }

    /// Create and persist a new job
    ///
    /// # Arguments
    ///
    /// * `req` - Job request containing command, GPU count, environment, etc.
    ///
    /// # Returns
    ///
    /// Persisted job details, or an error message on failure.
    fn create_job(&self, req: JobRequest) -> Result<JobInfo, String> {
        if matches!(self.runtime.mode, DaemonMode::User) && req.username != self.runtime.owner_name
        {
            return Err("Job user does not match daemon owner".to_string());
        }
        let q = self.queue.lock().unwrap();
        let dependencies = req.dependencies.clone();
        let mut job = Job::from_request(
            q.get_max_id().unwrap_or(0) + 1,
            req,
            &self.runtime.paths.job_logs,
        );

        if let Some(allowed_gpus) = job.allowed_gpus.ids()
            && (!self.gpu_pool.lock().unwrap().manages_gpu_ids(allowed_gpus)
                || job.required_gpus > allowed_gpus.len())
        {
            return Err("Invalid allowed GPU selection for required GPU count".to_string());
        }

        q.insert_job_with_dependencies(&dependencies, &mut job)?;
        if let Err(message) = self.prepare_log_file(&job.log_path, &job.username) {
            q.delete_jobs(&[job.id]).ok();
            return Err(message);
        }

        drop(q);
        self.get_job_info(job.id, &job.username).map_err(|error| {
            format!(
                "Job {} was created, but reading its saved details failed: {}",
                job.id, error
            )
        })
    }

    /// Prepare log file for a new job
    ///
    /// Creates log file and parent directories if needed, sets ownership to job user.
    ///
    /// # Arguments
    ///
    /// * `log_file` - Path to the log file to prepare
    /// * `username` - Username to set as owner of the log file
    ///
    /// # Returns
    ///
    /// `Ok(())` if log file is prepared successfully, `Err` with error message on failure
    fn prepare_log_file(&self, log_file: &str, username: &str) -> Result<(), String> {
        let user = User::from_name(username)
            .map_err(|error| format!("Failed to query user '{}': {}", username, error))?
            .ok_or_else(|| format!("User not found: {}", username))?;
        let log = Path::new(log_file);
        let parent = log
            .parent()
            .ok_or_else(|| format!("Invalid log path: {}", log_file))?;

        fs::create_dir_all(parent)
            .map_err(|error| format!("Failed to create log dir: {}", error))?;
        let file = File::options()
            .create(true)
            .append(true)
            .open(log)
            .map_err(|error| format!("Failed to prepare log file: {}", error))?;
        nix::unistd::fchown(&file, Some(user.uid), Some(user.gid))
            .map_err(|error| error.to_string())?;

        Ok(())
    }

    /// Handle job cancellation request
    ///
    /// Checks permission and job status. Blocked and pending jobs are directly cancelled.
    ///
    /// Running jobs require `--force` flag and will be forcibly terminated.
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID to cancel
    /// * `username` - User requesting cancellation
    /// * `force` - Whether to forcibly terminate running jobs
    ///
    /// # Returns
    ///
    /// Response indicating success or failure of cancellation request, or error message on failure.
    fn handle_cancel(&self, id: usize, username: &str, force: bool) -> Response {
        match self.get_job_with_permission(id, username) {
            Ok(job) => match job.status {
                JobStatus::Blocked | JobStatus::Pending => {
                    match self
                        .queue
                        .lock()
                        .unwrap()
                        .update_status(id, JobStatus::Cancelled, None)
                    {
                        Ok(()) => Response::Cancel,
                        Err(error) => Response::Error {
                            message: error.to_string(),
                        },
                    }
                }
                JobStatus::Running if force => match control::cancel(&self.sender, id, username) {
                    Ok(()) => Response::Cancel,
                    Err(message) => Response::Error { message },
                },
                JobStatus::Running => Response::Error {
                    message: "Job is running, requires --force".to_string(),
                },
                _ => Response::Error {
                    message: "Job has ended, cannot cancel".to_string(),
                },
            },
            Err(message) => Response::Error { message },
        }
    }

    /// Handle job deletion request
    ///
    /// Only completed, failed, or cancelled jobs can be deleted.
    /// Pending and running jobs cannot be deleted.
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID to delete
    ///
    /// # Returns
    ///
    /// Response indicating number of jobs deleted or error message on failure
    fn handle_delete(&self, target: DeleteTarget) -> Response {
        let result = match target {
            DeleteTarget::Single(id) => self.delete_single(id),
            DeleteTarget::ByStatuses(statuses) => self.delete_by_statuses(statuses),
            DeleteTarget::AllTerminated => self.delete_all_terminated(),
        };

        let (ids, log_paths) = match result {
            Ok(result) => result,
            Err(e) => return Response::Error { message: e },
        };

        // delete database records
        match self
            .queue
            .lock()
            .unwrap()
            .delete_jobs(&ids)
            .map_err(|e| e.to_string())
        {
            Ok(count) => {
                // delete log files
                for path in &log_paths {
                    let _ = fs::remove_file(path);
                }
                Response::Delete { count }
            }
            Err(e) => Response::Error { message: e },
        }
    }

    /// Delete a single job by ID after checking its status
    ///
    /// Only allows deletion of jobs that are completed, failed, or cancelled.
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID to delete
    ///
    /// # Returns
    ///
    /// `Ok((vec![id], vec![log_path]))` if the job can be deleted
    fn delete_single(&self, id: usize) -> Result<(Vec<usize>, Vec<String>), String> {
        let q = self.queue.lock().unwrap();
        match q.get_job_by_id(id) {
            Ok(Some(job)) => {
                if !matches!(
                    job.status,
                    JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
                ) {
                    return Err(format!(
                        "Job {} is {}, cannot delete",
                        id,
                        job.status.as_str()
                    ));
                }

                Ok((vec![id], vec![job.log_path]))
            }
            Ok(None) => Err(format!("Failed to find job {}", id)),
            Err(e) => Err(format!("Failed to query job: {}", e)),
        }
    }

    /// Delete jobs by their statuses
    ///
    /// Only allows deletion of jobs that are completed, failed, or cancelled.
    ///
    /// # Arguments
    ///
    /// * `statuses` - List of job statuses to delete
    ///
    /// # Returns
    ///
    /// `Ok((ids, log_paths))` if jobs can be deleted
    fn delete_by_statuses(
        &self,
        statuses: Vec<JobStatus>,
    ) -> Result<(Vec<usize>, Vec<String>), String> {
        let allowed = |s: &JobStatus| {
            matches!(
                s,
                JobStatus::Completed | JobStatus::Failed | JobStatus::Cancelled
            )
        };
        if !statuses.iter().all(allowed) {
            return Err("Only completed, failed, cancelled statuses are allowed".to_string());
        }

        let q = self.queue.lock().unwrap();
        q.get_jobs_by_statuses(&statuses)
            .map(|jobs| {
                let ids: Vec<usize> = jobs.iter().map(|j| j.id).collect();
                let paths: Vec<String> = jobs.iter().map(|j| j.log_path.clone()).collect();
                (ids, paths)
            })
            .map_err(|e| format!("Failed to query jobs: {}", e))
    }

    /// Delete all jobs that are in terminated states (completed, failed, cancelled)
    ///
    /// # Returns
    ///
    /// `Ok((ids, log_paths))` if jobs can be deleted
    fn delete_all_terminated(&self) -> Result<(Vec<usize>, Vec<String>), String> {
        let q = self.queue.lock().unwrap();
        let statuses = vec![
            JobStatus::Completed,
            JobStatus::Failed,
            JobStatus::Cancelled,
        ];
        q.get_jobs_by_statuses(&statuses)
            .map(|jobs| {
                let ids: Vec<usize> = jobs.iter().map(|j| j.id).collect();
                let paths: Vec<String> = jobs.iter().map(|j| j.log_path.clone()).collect();
                (ids, paths)
            })
            .map_err(|e| format!("Failed to query jobs: {}", e))
    }

    /// Handle GPU status query request
    ///
    /// # Returns
    ///
    /// Response containing list of all GPUs with their current status
    fn handle_status(&self) -> Response {
        let gpus = self.gpu_pool.lock().unwrap().get_all_status();
        Response::Status {
            gpus,
            daemon: self.runtime.info(if self.stopping.load(Ordering::SeqCst) {
                DaemonState::Stopping
            } else {
                DaemonState::Running
            }),
        }
    }

    /// Handle job list request
    ///
    /// # Arguments
    ///
    /// * `username` - Filter jobs by this user (root sees all)
    /// * `limit` - Limit number of jobs returned, None for all
    ///
    /// # Returns
    ///
    /// Jobs filtered by username. Root user sees all jobs.
    fn handle_list(&self, username: &str, limit: Option<usize>) -> Response {
        let (dependencies, total, jobs) = {
            let q = self.queue.lock().unwrap();

            let total = q.get_job_count().unwrap_or(0) as usize;
            let jobs = if username == "root" {
                q.get_all_jobs(limit).unwrap_or_default()
            } else {
                q.get_jobs_by_username(username, limit).unwrap_or_default()
            };

            let job_ids = jobs.iter().map(|job| job.id).collect::<Vec<_>>();
            let dependencies = q.get_dependencies_by_job_ids(&job_ids).unwrap_or_default();

            (dependencies, total, jobs)
        };

        let job_infos: Vec<JobPrintInfo> = jobs
            .into_iter()
            .map(|j| JobPrintInfo {
                id: j.id,
                status: j.status,
                username: j.username,
                command: j.command,
                allowed_gpus: j.allowed_gpus,
                allocated_gpus: j.allocated_gpus,
                dependencies: dependencies.get(&j.id).cloned().unwrap_or_default(),
                required_gpus: j.required_gpus,
                remaining_dependencies: j.remaining_dependencies,
                start_time: j.start_time,
                end_time: j.end_time,
            })
            .collect();

        Response::List {
            jobs: job_infos,
            total,
        }
    }

    /// Query complete job details after checking ownership.
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID to query.
    /// * `username` - Requesting user; root may query any job.
    ///
    /// # Returns
    ///
    /// Complete details, or an error for lookup, permission, or database failure.
    fn handle_info(&self, id: usize, username: &str) -> Response {
        match self.get_job_info(id, username) {
            Ok(job) => Response::Info { job: Box::new(job) },
            Err(message) => Response::Error { message },
        }
    }

    /// Read saved job details and dependencies after checking ownership.
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID to query.
    /// * `username` - Requesting user; root may query any job.
    ///
    /// # Returns
    ///
    /// Saved job details with prerequisite IDs in ascending order.
    fn get_job_info(&self, id: usize, username: &str) -> Result<JobInfo, String> {
        let job = self.get_job_with_permission(id, username)?;
        let mut dependencies = self
            .queue
            .lock()
            .unwrap()
            .get_dependencies_by_job_ids(&[id])
            .map_err(|error| format!("Database error: {error}"))?;
        Ok(JobInfo::from_job(
            job,
            dependencies.remove(&id).unwrap_or_default(),
        ))
    }

    /// Handle job log path request
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID
    /// * `username` - User requesting log path
    ///
    /// # Returns
    ///
    /// Log file path for specified job after permission check.
    fn handle_log(&self, id: usize, username: &str) -> Response {
        match self.get_job_with_permission(id, username) {
            Ok(job) => Response::Log {
                log_path: job.log_path,
            },
            Err(message) => Response::Error { message },
        }
    }

    /// Fetch job and verify user permission
    ///
    /// # Arguments
    ///
    /// * `id` - Job ID to fetch
    /// * `username` - User requesting the job
    ///
    /// # Returns
    ///
    /// Job if found and user has permission (root or job owner).
    fn get_job_with_permission(&self, id: usize, username: &str) -> Result<Job, String> {
        match self.queue.lock().unwrap().get_job_by_id(id) {
            Ok(Some(job)) => {
                if username != "root" && username != job.username {
                    Err(format!(
                        "Permission denied: job {} belongs to user '{}'",
                        id, job.username
                    ))
                } else {
                    Ok(job)
                }
            }
            Ok(None) => Err(format!("Job {} not found", id)),
            Err(e) => Err(format!("Database error: {}", e)),
        }
    }
}

/// Check if daemon is running
///
/// Determines by attempting to connect to socket.
///
/// # Returns
///
/// True if daemon is running (socket connection successful), false otherwise.
pub(crate) fn is_daemon_running() -> bool {
    socket::occupied()
}
