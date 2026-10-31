//! Socket client module
//!
//! Communicate with daemon via Unix Domain Socket, submit jobs and query GPU status.

use std::{io::Write, time::Duration};

use crate::{
    os::get_current_username,
    runtime::{connection_uid, socket},
    types::{
        DaemonInfo, DeleteTarget, GpuInfo, GpuSelection, JobInfo, JobPrintInfo, JobRequest,
        Request, RerunContext, Response,
    },
};

/// Send request to daemon and receive response
///
/// # Arguments
///
/// * `request` - Request struct
///
/// # Returns
///
/// * `Response` - Daemon response
/// * `String` - Error message
fn send_request(request: &Request) -> Result<Response, String> {
    let json = serde_json::to_string(request)
        .map_err(|e| format!("Failed to serialize request: {}", e))?;

    // Root may manage a personal daemon with Stop.
    // All other business requests still require a trusted daemon owner.
    let mut stream = if matches!(request, Request::Status)
        || (matches!(request, Request::Stop { .. }) && nix::unistd::Uid::effective().is_root())
    {
        socket::connect_status()
    } else {
        socket::connect()
    }
    .map_err(|error| format!("Failed to connect to daemon: {}", error))?;
    let owner = connection_uid(&stream).map_err(|error| error.to_string())?;
    stream
        .set_read_timeout(Some(Duration::from_secs(
            if matches!(request, Request::Stop { .. } | Request::Cancel { .. }) {
                35
            } else {
                10
            },
        )))
        .map_err(|error| error.to_string())?;
    stream
        .set_write_timeout(Some(Duration::from_secs(10)))
        .map_err(|error| error.to_string())?;
    stream
        .write_all(json.as_bytes())
        .map_err(|error| format!("Failed to send request: {}", error))?;
    stream.shutdown(std::net::Shutdown::Write).ok();

    let response: Response = serde_json::from_reader(&stream)
        .map_err(|error| format!("Failed to receive response: {}", error))?;
    if let Response::Status { daemon, .. } = &response
        && daemon.owner_uid != owner.as_raw()
    {
        return Err("Daemon status owner does not match socket credentials".into());
    }

    Ok(response)
}

/// Request authorized shutdown and wait for the executor's cleanup acknowledgement.
///
/// # Arguments
///
/// * `force` - Terminate running tasks and wait for cleanup before stopping.
///
/// # Returns
///
/// Success after cleanup and release of the daemon socket, or an error.
pub(crate) fn stop(force: bool) -> Result<(), String> {
    match send_request(&Request::Stop { force })? {
        Response::Stop => {
            let deadline = std::time::Instant::now() + Duration::from_secs(5);
            loop {
                match socket::bind() {
                    Ok(_) => return Ok(()),
                    Err(error) if error.kind() == std::io::ErrorKind::AddrInUse => {}
                    Err(error) => return Err(format!("Failed to confirm daemon exit: {error}")),
                }
                if std::time::Instant::now() >= deadline {
                    return Err("Shutdown completed but daemon socket is still occupied".into());
                }
                std::thread::sleep(Duration::from_millis(20));
            }
        }
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Submit job to daemon
///
/// # Arguments
///
/// * `request` - Job request struct
///
/// # Returns
///
/// * `JobInfo` - Persisted details of the submitted job on success
/// * `String` - Error message on failure
pub(crate) fn submit_job(request: JobRequest) -> Result<JobInfo, String> {
    match send_request(&Request::Submit { job: request })? {
        Response::Submit { job } => Ok(*job),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Rerun an existing job as a new job
///
/// # Arguments
///
/// * `id` - Source job ID
/// * `context` - Current terminal values overriding the saved context
/// * `allowed_gpus` - Optional allowed GPU ID override
/// * `dependencies` - Optional dependency override
/// * `required_gpus` - Optional required GPU count override
/// * `log_path` - Optional log path override
///
/// # Returns
///
/// * `JobInfo` - Persisted details of the new job on success
/// * `String` - Error message on failure
pub(crate) fn rerun_job(
    id: usize,
    context: RerunContext,
    allowed_gpus: Option<GpuSelection>,
    dependencies: Option<Vec<usize>>,
    required_gpus: Option<usize>,
    log_path: Option<String>,
) -> Result<JobInfo, String> {
    match send_request(&Request::Rerun {
        id,
        context,
        allowed_gpus,
        dependencies,
        required_gpus,
        log_path,
        username: get_current_username(),
    })? {
        Response::Rerun { job } => Ok(*job),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Cancel job
///
/// # Arguments
///
/// * `id` - Job ID
/// * `force` - Force termination of running job
///
/// # Returns
///
/// * `()` - Success
/// * `String` - Error message
pub(crate) fn cancel_job(id: usize, force: bool) -> Result<(), String> {
    match send_request(&Request::Cancel {
        id,
        username: get_current_username(),
        force,
    })? {
        Response::Cancel => Ok(()),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Delete job
///
/// # Arguments
///
/// * `target` - Delete target (single, by status, or all)
///
/// # Returns
///
/// * `usize` - Number of jobs deleted
/// * `String` - Error message
pub(crate) fn delete_job(target: DeleteTarget) -> Result<usize, String> {
    match send_request(&Request::Delete { target })? {
        Response::Delete { count } => Ok(count),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Query GPU status
///
/// # Returns
///
/// * `Vec<GpuInfo>` - GPU info list
/// * `String` - Error message
pub(crate) fn query_gpu_status() -> Result<(Vec<GpuInfo>, DaemonInfo), String> {
    match send_request(&Request::Status)? {
        Response::Status { gpus, daemon } => Ok((gpus, daemon)),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// List all jobs
///
/// # Arguments
///
/// * `limit` - Limit number of jobs returned, None for all
///
/// # Returns
///
/// * `(Vec<JobPrintInfo>, usize)` - Job info list and total count
/// * `String` - Error message
pub(crate) fn list_jobs(limit: Option<usize>) -> Result<(Vec<JobPrintInfo>, usize), String> {
    match send_request(&Request::List {
        username: get_current_username(),
        limit,
    })? {
        Response::List { jobs, total } => Ok((jobs, total)),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Query complete job details with the current user's permissions.
///
/// # Arguments
///
/// * `id` - Job ID to query.
///
/// # Returns
///
/// The saved job details, including environment variables and directory.
pub(crate) fn get_job_info(id: usize) -> Result<JobInfo, String> {
    match send_request(&Request::Info {
        id,
        username: get_current_username(),
    })? {
        Response::Info { job } => Ok(*job),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response".to_string()),
    }
}

/// Get job log path
///
/// # Arguments
///
/// * `id` - Job ID
///
/// # Returns
///
/// * `String` - Log file path
/// * `String` - Error message
pub(crate) fn get_log_path(id: usize) -> Result<String, String> {
    match send_request(&Request::Log {
        id,
        username: get_current_username(),
    })? {
        Response::Log { log_path } => Ok(log_path),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}
