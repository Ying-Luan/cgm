//! Database schema definition
//!
//! Defines jobs table structure.

use rusqlite::Connection;

use crate::constants::SCHEMA_VERSION;

/// Create the jobs table and optionally the dependency table.
///
/// # Arguments
///
/// * `conn` - Database connection.
/// * `jobs_table_name` - Name of the jobs table to create.
/// * `with_dependencies` - Also create the dependency table.
///
/// # Returns
///
/// The SQLite result of creating the table.
pub(super) fn create_tables(
    conn: &Connection,
    jobs_table_name: &str,
    with_dependencies: bool,
) -> rusqlite::Result<()> {
    conn.execute_batch(&format!(
        "
        CREATE TABLE \"{jobs_table_name}\" (
            id                     INTEGER PRIMARY KEY,
            name                   TEXT,
            status                 TEXT    NOT NULL CHECK (status IN ('unknown', 'blocked', 'pending', 'running', 'completed', 'failed', 'cancelled')),
            uid                    INTEGER NOT NULL,
            username               TEXT    NOT NULL,
            command                TEXT    NOT NULL,  -- JSON array, e.g. [\"python\",\"main.py\"]
            allowed_gpus           TEXT    NOT NULL,  -- Serialized GpuSelection
            allocated_gpus         TEXT,              -- JSON array of allocated GPU IDs
            required_gpus          INTEGER NOT NULL,
            envs                   TEXT    NOT NULL,  -- JSON object, e.g. {{\"KEY\":\"value\"}}
            log_path               TEXT    NOT NULL,
            cwd                    TEXT    NOT NULL,
            remaining_dependencies INTEGER NOT NULL DEFAULT 0,
            start_time    INTEGER,
            end_time      INTEGER
        );
        "
    ))?;
    if with_dependencies {
        conn.execute_batch(
            "
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
    }
    Ok(())
}

/// Create tables and set the schema version.
///
/// # Arguments
///
/// * `conn` - Open database connection
///
/// # Returns
///
/// Create result, Ok(()) on success, error message on failure
pub(crate) fn initialize_db(conn: &mut Connection) -> rusqlite::Result<()> {
    let tx = conn.transaction()?;
    create_tables(&tx, "jobs", true)?;
    tx.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    tx.commit()
}
