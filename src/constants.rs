//! Constants definitions
//!
//! Defines daemon communication paths, color codes and so on.

/// Macro to generate cgm related paths
macro_rules! cgm_dir {
    () => {
        "/tmp/cgm"
    };
}

/// CGM run directory
pub const CGM_RUN_DIR: &str = cgm_dir!();
/// PID file path
pub const PID_PATH: &str = concat!(cgm_dir!(), "/cgm.pid");
/// Socket path for daemon communication
pub const SOCKET_PATH: &str = concat!(cgm_dir!(), "/cgm.sock");
/// Database path
pub const DB_PATH: &str = concat!(cgm_dir!(), "/cgm.db");
/// Job log folder path
pub const JOB_LOG_FOLDER: &str = concat!(cgm_dir!(), "/logs");
/// Standard output log path
pub const CGM_OUT_PATH: &str = concat!(cgm_dir!(), "/cgm.out");
/// Standard error log path
pub const CGM_ERR_PATH: &str = concat!(cgm_dir!(), "/cgm.err");
