//! Database schema validation and migrations.
//!
//! Validates versioned column layouts and upgrades supported databases atomically.

use rusqlite::{Connection, TransactionBehavior};

use crate::constants::SCHEMA_VERSION;

use super::schema::create_tables;

/// Jobs columns in schema order, paired with their introduction version.
const JOB_COLUMNS: &[(&str, u32)] = &[
    ("id", 0),
    ("name", 1),
    ("status", 0),
    ("uid", 0),
    ("username", 0),
    ("command", 0),
    ("allowed_gpus", 0),
    ("allocated_gpus", 0),
    ("required_gpus", 0),
    ("envs", 0),
    ("log_path", 0),
    ("cwd", 0),
    ("remaining_dependencies", 0),
    ("start_time", 0),
    ("end_time", 0),
];

/// Compare the ordered columns with a known schema.
///
/// # Arguments
///
/// * `conn` - Database connection.
/// * `job_columns` - Expected jobs columns in schema order.
///
/// # Returns
///
/// Success if the database columns exactly match the expected columns.
fn check_schema(conn: &Connection, job_columns: &[&str]) -> Result<(), String> {
    for (table, expected_columns) in [
        ("jobs", job_columns),
        ("job_dependencies", &["job_id", "depends_on"][..]),
    ] {
        let mut statement = conn
            .prepare("SELECT name FROM pragma_table_info(?1) ORDER BY cid")
            .map_err(|error| format!("Failed to inspect {table}: {error}"))?;
        let columns: Vec<String> = statement
            .query_map([table], |row| row.get(0))
            .and_then(Iterator::collect)
            .map_err(|error| format!("Failed to inspect {table}: {error}"))?;
        if columns != expected_columns {
            return Err(format!("Database has unexpected {table} columns"));
        }
    }

    Ok(())
}

/// Validate and migrate an existing database directly to the current schema.
///
/// # Arguments
///
/// * `conn` - Database transaction.
/// * `source_version` - Current database schema version.
///
/// # Returns
///
/// Success after the database has been upgraded.
fn migrate_existing(conn: &Connection, source_version: u32) -> Result<(), String> {
    if source_version > SCHEMA_VERSION {
        return Err(format!(
            "Database schema version {source_version} is newer than supported version {SCHEMA_VERSION}"
        ));
    }

    let columns_for = |version| -> Vec<&str> {
        JOB_COLUMNS
            .iter()
            .filter(|(_, since)| *since <= version)
            .map(|(name, _)| *name)
            .collect()
    };
    let source_columns = columns_for(source_version);
    check_schema(conn, &source_columns)?;

    if source_version < SCHEMA_VERSION {
        let source_names = source_columns.join(", ");
        let copy_sql =
            format!("INSERT INTO jobs_new ({source_names}) SELECT {source_names} FROM jobs");
        create_tables(conn, "jobs_new", false)
            .map_err(|error| format!("Failed to create migration table: {error}"))?;
        conn.execute_batch(&copy_sql).map_err(|error| {
            format!("Failed to copy jobs from version {source_version}: {error}")
        })?;
        conn.execute_batch("DROP TABLE jobs; ALTER TABLE jobs_new RENAME TO jobs;")
            .map_err(|error| format!("Failed to replace jobs table: {error}"))?;
        let target_columns = columns_for(SCHEMA_VERSION);
        check_schema(conn, &target_columns)?;
        conn.pragma_update(None, "user_version", SCHEMA_VERSION)
            .map_err(|error| format!("Failed to set database version: {error}"))?;
    }

    Ok(())
}

/// Validate and migrate an existing database in a single transaction.
///
/// # Arguments
///
/// * `conn` - Database connection to prepare.
///
/// # Returns
///
/// Success after the schema is ready. The caller must enable foreign keys.
pub(super) fn prepare_db(conn: &mut Connection) -> Result<(), String> {
    // SQLite only changes foreign key enforcement outside a transaction.
    conn.pragma_update(None, "foreign_keys", false)
        .map_err(|error| format!("Failed to disable foreign keys: {error}"))?;

    let tx = conn
        .transaction_with_behavior(TransactionBehavior::Immediate)
        .map_err(|error| format!("Failed to begin database transaction: {error}"))?;
    let version: u32 = tx
        .pragma_query_value(None, "user_version", |row| row.get(0))
        .map_err(|error| format!("Failed to read database version: {error}"))?;
    migrate_existing(&tx, version)?;
    // Validate dependencies before committing the rebuilt tables.
    let violation: bool = tx
        .query_row(
            "SELECT EXISTS(SELECT 1 FROM pragma_foreign_key_check)",
            [],
            |row| row.get(0),
        )
        .map_err(|error| format!("Failed to check foreign keys: {error}"))?;
    if violation {
        return Err("Database contains foreign key violations".into());
    }
    tx.commit()
        .map_err(|error| format!("Failed to commit database migration: {error}"))
}

#[cfg(test)]
mod tests {
    use std::{fs, process, time::SystemTime};

    use rusqlite::Connection;

    use crate::{constants::SCHEMA_VERSION, db::DataBase};

    use super::{JOB_COLUMNS, check_schema, prepare_db};

    /// Schema shipped in cgm v0.2.0, before database versioning.
    const V0_SCHEMA: &str = "
CREATE TABLE jobs (
    id INTEGER PRIMARY KEY,
    status TEXT NOT NULL CHECK (status IN ('unknown', 'blocked', 'pending', 'running', 'completed', 'failed', 'cancelled')),
    uid INTEGER NOT NULL,
    username TEXT NOT NULL,
    command TEXT NOT NULL,
    allowed_gpus TEXT NOT NULL,
    allocated_gpus TEXT,
    required_gpus INTEGER NOT NULL,
    envs TEXT NOT NULL,
    log_path TEXT NOT NULL,
    cwd TEXT NOT NULL,
    remaining_dependencies INTEGER NOT NULL DEFAULT 0,
    start_time INTEGER,
    end_time INTEGER
);
CREATE TABLE job_dependencies (
    job_id INTEGER NOT NULL,
    depends_on INTEGER NOT NULL,
    PRIMARY KEY (job_id, depends_on),
    FOREIGN KEY (job_id) REFERENCES jobs(id) ON DELETE CASCADE,
    FOREIGN KEY (depends_on) REFERENCES jobs(id) ON DELETE NO ACTION,
    CHECK (job_id != depends_on)
);";

    #[test]
    fn creates_current_schema() {
        let unique = SystemTime::now()
            .duration_since(SystemTime::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        let path = std::env::temp_dir().join(format!("cgm-migrate-{}-{unique}.db", process::id()));
        let db = DataBase::open(&path).unwrap();
        drop(db);
        let mut conn = Connection::open(&path).unwrap();
        let columns: Vec<&str> = JOB_COLUMNS
            .iter()
            .filter(|(_, since)| *since <= SCHEMA_VERSION)
            .map(|(name, _)| *name)
            .collect();
        check_schema(&conn, &columns).unwrap();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, SCHEMA_VERSION);
        prepare_db(&mut conn).unwrap();
        drop(conn);
        fs::remove_file(path).unwrap();
    }

    #[test]
    fn migrates_v020_and_preserves_jobs_and_dependencies() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V0_SCHEMA).unwrap();
        let second: String = conn
            .query_row(
                "SELECT name FROM pragma_table_info('jobs') WHERE cid = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(second, "status");
        conn.execute_batch(
            "INSERT INTO jobs (id, status, uid, username, command, allowed_gpus, required_gpus, envs, log_path, cwd)
             VALUES (1, 'completed', 1000, 'alice', '[\"one\"]', '\"All\"', 1, '{}', '/tmp/one.log', '/tmp'),
                    (2, 'blocked', 1000, 'alice', '[\"two\"]', '\"All\"', 1, '{}', '/tmp/two.log', '/tmp');
             INSERT INTO job_dependencies VALUES (2, 1);",
        ).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        prepare_db(&mut conn).unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        let second: String = conn
            .query_row(
                "SELECT name FROM pragma_table_info('jobs') WHERE cid = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(second, "name");
        let columns: Vec<&str> = JOB_COLUMNS
            .iter()
            .filter(|(_, since)| *since <= SCHEMA_VERSION)
            .map(|(name, _)| *name)
            .collect();
        check_schema(&conn, &columns).unwrap();
        let row: (Option<String>, u32, String) = conn
            .query_row(
                "SELECT name, uid, log_path FROM jobs WHERE id = 1",
                [],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .unwrap();
        assert_eq!(row, (None, 1000, "/tmp/one.log".into()));
        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM job_dependencies WHERE job_id = 2 AND depends_on = 1",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
        prepare_db(&mut conn).unwrap();
    }

    #[test]
    fn rolls_back_failed_migration() {
        let mut conn = Connection::open_in_memory().unwrap();
        conn.execute_batch(V0_SCHEMA).unwrap();
        conn.pragma_update(None, "ignore_check_constraints", true)
            .unwrap();
        conn.execute_batch(
            "INSERT INTO jobs (id, status, uid, username, command, allowed_gpus, required_gpus, envs, log_path, cwd)
            VALUES (1, 'invalid', 1, 'u', '[]', '\"All\"', 0, '{}', '/tmp/log', '/tmp')"
        ).unwrap();
        conn.pragma_update(None, "ignore_check_constraints", false)
            .unwrap();
        conn.pragma_update(None, "foreign_keys", true).unwrap();
        assert!(prepare_db(&mut conn).is_err());
        let columns: Vec<&str> = JOB_COLUMNS
            .iter()
            .filter(|(_, since)| *since <= 0)
            .map(|(name, _)| *name)
            .collect();
        check_schema(&conn, &columns).unwrap();
        let version: u32 = conn
            .pragma_query_value(None, "user_version", |row| row.get(0))
            .unwrap();
        assert_eq!(version, 0);
        let count: u32 = conn
            .query_row(
                "SELECT COUNT(*) FROM jobs WHERE status = 'invalid'",
                [],
                |row| row.get(0),
            )
            .unwrap();
        assert_eq!(count, 1);
    }
}
