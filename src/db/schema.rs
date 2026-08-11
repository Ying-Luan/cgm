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
pub(crate) fn initialize_db(conn: &Connection) -> rusqlite::Result<()> {
    conn.execute_batch(
        "
        CREATE TABLE IF NOT EXISTS jobs (
            id                     INTEGER PRIMARY KEY,
            status                 TEXT    NOT NULL CHECK (status IN ('unknown', 'blocked', 'pending', 'running', 'completed', 'failed', 'cancelled')),
            username               TEXT    NOT NULL,
            command                TEXT    NOT NULL,  -- JSON array, e.g. [\"python\",\"main.py\"]
            allowed_gpus           TEXT    NOT NULL,  -- JSON array
            required_gpus          INTEGER NOT NULL,
            envs                   TEXT    NOT NULL,  -- JSON object, e.g. {\"KEY\":\"value\"}
            log_path               TEXT    NOT NULL,
            cwd                    TEXT    NOT NULL,
            remaining_dependencies INTEGER NOT NULL DEFAULT 0,
            start_time    INTEGER,
            end_time      INTEGER
        );

        CREATE TABLE IF NOT EXISTS job_dependencies (
            job_id     INTEGER NOT NULL,
            depends_on INTEGER NOT NULL,
            PRIMARY KEY (job_id, depends_on),
            FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE,
            FOREIGN KEY (depends_on) REFERENCES jobs(id) ON DELETE NO ACTION,
            CHECK (job_id != depends_on)
        );
        ",
    )?;

    Ok(())
}
