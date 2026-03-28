//! Status monitor module
//!
//! Provides show_status() function to display daemon status, GPU info and job list.

use comfy_table::{presets::NOTHING, Cell, Table};
use time::{macros::format_description, OffsetDateTime, UtcOffset};

use crate::{
    client,
    daemon::is_daemon_running,
    hardware::get_gpu_info,
    macros::{green, yellow},
};

/// Format Unix timestamp to YYYY-MM-DD HH:MM:SS string
///
/// # Arguments
///
/// * `ts` - Unix timestamp in seconds
///
/// # Returns
///
/// Formatted time string, or "-" if timestamp is invalid
fn format_timestamp(ts: i64) -> String {
    let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
    OffsetDateTime::from_unix_timestamp(ts)
        .ok()
        .map(|dt| dt.to_offset(offset))
        .and_then(|dt| {
            let fmt = format_description!("[year]-[month]-[day] [hour]:[minute]:[second]");
            dt.format(&fmt).ok()
        })
        .unwrap_or_else(|| "-".to_string())
}

/// Calculate duration string from start and end timestamps
///
/// # Arguments
///
/// * `start` - Start time as Unix timestamp in seconds
/// * `end` - End time as Unix timestamp in seconds, or None if still running
///
/// # Returns
///
/// Duration string like "1h 23m", "45m 10s", "30s", "running", or "-" if start time is invalid
fn calculate_duration(start: Option<i64>, end: Option<i64>) -> String {
    match (start, end) {
        (Some(s), Some(e)) => {
            let duration = if e > s { e - s } else { 0 };
            let secs = duration as u64;
            if secs < 60 {
                format!("{}s", secs)
            } else if secs < 3600 {
                format!("{}m {}s", secs / 60, secs % 60)
            } else {
                format!("{}h {}m", secs / 3600, (secs % 3600) / 60)
            }
        }
        (Some(_), None) => "running".to_string(),
        _ => "-".to_string(),
    }
}

/// Display status dashboard
///
/// Prints daemon status, GPU info, GPU state (from daemon), and all job list.
pub fn show_status() {
    // Display daemon status
    println!("--- CGM Status ---");
    println!(
        "Daemon: {}",
        if is_daemon_running() {
            green!("ACTIVE")
        } else {
            yellow!("INACTIVE")
        }
    );

    // Display GPU info
    println!("\n--- GPU Info ---");
    let gpu_infos = get_gpu_info();
    let daemon_status = client::query_gpu_status().unwrap_or_default();

    if gpu_infos.is_empty() {
        println!("{}", yellow!("No NVIDIA GPUs found."));
    } else {
        let mut table = Table::new();
        table.load_preset(NOTHING).set_header(vec![
            "ID",
            "Name",
            "Memory (Used/Total)",
            "Temp",
            "GPU",
            "Mem",
            "State",
            "Job",
        ]);

        for gpu in gpu_infos {
            let daemon_info = daemon_status.iter().find(|d| d.id == gpu.id);
            let state = daemon_info
                .map(|d| format!("{:?}", d.state))
                .unwrap_or_else(|| "?".to_string());
            let job_id = daemon_info
                .and_then(|d| d.job_id)
                .map(|j| j.to_string())
                .unwrap_or_else(|| "-".to_string());

            table.add_row(vec![
                gpu.id.to_string(),
                gpu.name,
                format!(
                    "{:.1} / {:.1} GB",
                    gpu.memory_used as f64 / (1024.0 * 1024.0 * 1024.0),
                    gpu.memory_total as f64 / (1024.0 * 1024.0 * 1024.0)
                ),
                format!("{} °C", gpu.temp),
                format!("{}%", gpu.util_gpu),
                format!("{}%", gpu.util_memory),
                state,
                job_id,
            ]);
        }

        println!("{table}");
    }
}

/// Display job list
///
/// # Arguments
///
/// * `limit` - Limit number of jobs returned, None for all
pub fn show_list(limit: Option<usize>) {
    match client::list_jobs(limit) {
        Ok((jobs, total)) => {
            println!("--- Jobs ({}/{}) ---", jobs.len(), total);
            if jobs.is_empty() {
                println!("{}", yellow!("(no jobs)"));
                return;
            }

            let mut table = Table::new();
            table.load_preset(NOTHING).set_header(vec![
                "ID", "Status", "User", "GPUs", "Start", "Duration", "Command",
            ]);

            for job in jobs {
                let start_str = job
                    .start_time
                    .map(format_timestamp)
                    .unwrap_or_else(|| "-".to_string());
                let duration_str = calculate_duration(job.start_time, job.end_time);
                table.add_row(vec![
                    Cell::new(job.id.to_string()),
                    Cell::new(job.status.as_str()).fg(job.status.color()),
                    Cell::new(job.username),
                    Cell::new(job.gpus.to_string()),
                    Cell::new(start_str),
                    Cell::new(duration_str),
                    Cell::new(job.command.join(" ")),
                ]);
            }

            println!("{table}");
        }
        Err(e) => {
            println!("--- Jobs ---");
            println!("{}", yellow!("(failed to query: {})", e));
        }
    }
}
