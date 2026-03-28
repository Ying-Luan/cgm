//! Socket client module
//!
//! Communicate with daemon via Unix Domain Socket, submit jobs and query GPU status.

use std::{io::Write, os::unix::net::UnixStream};

use crate::{
    constants::SOCKET_PATH,
    os::get_current_username,
    types::{DeleteTarget, GpuInfo, JobPrintInfo, JobRequest, Request, Response},
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

    let mut stream =
        UnixStream::connect(SOCKET_PATH).map_err(|_| "Failed to connect to daemon".to_string())?;
    stream
        .write_all(json.as_bytes())
        .map_err(|e| format!("Failed to send request: {}", e))?;
    stream.shutdown(std::net::Shutdown::Write).ok();

    serde_json::from_reader(&stream).map_err(|e| format!("Failed to receive response: {}", e))
}

/// Check number of running jobs
///
/// # Returns
///
/// * `usize` - Number of running jobs
/// * `String` - Error message
pub fn check_stop() -> Result<usize, String> {
    match send_request(&Request::StopCheck)? {
        Response::StopCheck { running_count } => Ok(running_count),
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
/// * `(usize, String)` - Job ID and log path on success
/// * `String` - Error message on failure
pub fn submit_job(request: JobRequest) -> Result<(usize, String), String> {
    match send_request(&Request::Submit { job: request })? {
        Response::Submit { id, log_path } => Ok((id, log_path)),
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
pub fn cancel_job(id: usize, force: bool) -> Result<(), String> {
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
pub fn delete_job(target: DeleteTarget) -> Result<usize, String> {
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
pub fn query_gpu_status() -> Result<Vec<GpuInfo>, String> {
    match send_request(&Request::Status)? {
        Response::Status { gpus } => Ok(gpus),
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
pub fn list_jobs(limit: Option<usize>) -> Result<(Vec<JobPrintInfo>, usize), String> {
    match send_request(&Request::List {
        username: get_current_username(),
        limit,
    })? {
        Response::List { jobs, total } => Ok((jobs, total)),
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
pub fn get_log_path(id: usize) -> Result<String, String> {
    match send_request(&Request::Log {
        id,
        username: get_current_username(),
    })? {
        Response::Log { log_path } => Ok(log_path),
        Response::Error { message } => Err(message),
        _ => Err("Unexpected response from daemon".to_string()),
    }
}
