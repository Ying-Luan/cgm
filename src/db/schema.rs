//! Database schema definition
//!
//! Defines jobs table structure.

use rusqlite::Connection;

/// Create tables. Uses IF NOT EXISTS for idempotency.
///
/// # Arguments
///
/// * `conn` - Open database connection
///
/// # Returns
///
/// Create result, Ok(()) on success, error message on failure
pub fn initialize_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS jobs (
            id            INTEGER PRIMARY KEY,
            status        TEXT    NOT NULL CHECK (status IN ('unknown', 'pending', 'running', 'completed', 'failed', 'cancelled')),
            username      TEXT    NOT NULL,
            command       TEXT    NOT NULL,  -- JSON array, e.g. [\"python\",\"main.py\"]
            gpus          INTEGER NOT NULL,
            envs          TEXT    NOT NULL,  -- JSON object, e.g. {\"KEY\":\"value\"}
            log_path      TEXT    NOT NULL,
            cwd           TEXT    NOT NULL,
            start_time    INTEGER,
            end_time      INTEGER
        );
        ",
    )?;

    Ok(())
}
