//! Status monitor module
//!
//! Provides show_status() function to display daemon status, GPU info and job list.

use comfy_table::{Cell, Table, presets::NOTHING};
use time::{OffsetDateTime, UtcOffset, macros::format_description};

use crate::{
    client,
    daemon::is_daemon_running,
    hardware::get_gpu_info,
    macros::{green, yellow},
};

/// Maximum command length shown in the compact job list.
const COMMAND_PREVIEW_LENGTH: usize = 32;

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

/// Truncate a command for compact table output.
///
/// # Arguments
///
/// * `command` - The command string to truncate.
///
/// # Returns
///
/// The original command if it fits within the preview limit; otherwise, a
/// shortened command ending with `...`.
fn truncate_command(mut command: String) -> String {
    if command.chars().count() <= COMMAND_PREVIEW_LENGTH {
        return command;
    }

    let prefix_length = COMMAND_PREVIEW_LENGTH.saturating_sub(3);
    let prefix_end = command
        .char_indices()
        .nth(prefix_length)
        // Safe: the length check guarantees this boundary exists.
        .expect("long command must contain the preview prefix")
        .0;
    command.truncate(prefix_end);
    // No reallocation: truncation preserves enough capacity for the suffix.
    command.push_str("...");
    command
}

/// Display status dashboard
///
/// Prints daemon status, GPU info, GPU state (from daemon), and all job list.
pub(crate) fn show_status() {
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
/// * `long_format` - Show detailed field names and full commands
pub(crate) fn show_list(limit: Option<usize>, long_format: bool) {
    match client::list_jobs(limit) {
        Ok((jobs, total)) => {
            println!("--- Jobs ({}/{}) ---", jobs.len(), total);
            if jobs.is_empty() {
                println!("{}", yellow!("(no jobs)"));
                return;
            }

            let mut table = Table::new();
            let headers = if long_format {
                vec![
                    "ID",
                    "Status",
                    "User",
                    "Required GPUs",
                    "Allowed GPUs",
                    "Depends On",
                    "Remaining",
                    "Start",
                    "Duration",
                    "Command",
                ]
            } else {
                vec![
                    "ID",
                    "Status",
                    "User",
                    "Req GPU",
                    "Allow GPU",
                    "Deps",
                    "Remain",
                    "Start",
                    "Duration",
                    "Cmd",
                ]
            };
            table.load_preset(NOTHING).set_header(headers);

            for job in jobs {
                let allowed_gpus = job.allowed_gpus.to_string();
                let dependencies = if job.dependencies.is_empty() {
                    "-".to_string()
                } else {
                    job.dependencies
                        .iter()
                        .map(usize::to_string)
                        .collect::<Vec<_>>()
                        .join(",")
                };
                let remaining = if job.dependencies.is_empty() {
                    "-".to_string()
                } else {
                    format!("{}/{}", job.remaining_dependencies, job.dependencies.len())
                };
                let start_str = job
                    .start_time
                    .map(format_timestamp)
                    .unwrap_or_else(|| "-".to_string());
                let duration_str = calculate_duration(job.start_time, job.end_time);
                let command = job.command.join(" ");
                let command = if long_format {
                    command
                } else {
                    truncate_command(command)
                };
                table.add_row(vec![
                    Cell::new(job.id.to_string()),
                    Cell::new(job.status.as_str()).fg(job.status.color()),
                    Cell::new(job.username),
                    Cell::new(job.required_gpus.to_string()),
                    Cell::new(allowed_gpus),
                    Cell::new(dependencies),
                    Cell::new(remaining),
                    Cell::new(start_str),
                    Cell::new(duration_str),
                    Cell::new(command),
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

#[cfg(test)]
mod tests {
    use super::{COMMAND_PREVIEW_LENGTH, truncate_command};

    #[test]
    fn keeps_short_commands_unchanged() {
        assert_eq!(
            truncate_command("python train.py".to_string()),
            "python train.py"
        );
    }

    #[test]
    fn truncates_long_commands_with_an_ellipsis() {
        let command = "x".repeat(COMMAND_PREVIEW_LENGTH + 1);
        let truncated = truncate_command(command);

        assert_eq!(truncated.chars().count(), COMMAND_PREVIEW_LENGTH);
        assert!(truncated.ends_with("..."));
    }
}
