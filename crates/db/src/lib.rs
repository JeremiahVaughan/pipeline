//! SQLite database initialization and migration runner.
//!
//! This crate centralizes setup concerns so that higher layers can stay focused
//! on business logic.

use rusqlite::Connection;
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};

/// Errors that can occur during database initialization.
#[derive(Debug)]
pub enum DbInitError {
    Io(std::io::Error),
    Sqlite(rusqlite::Error),
}

impl Display for DbInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::Sqlite(err) => write!(f, "SQLite error: {err}"),
        }
    }
}

impl std::error::Error for DbInitError {}

impl From<std::io::Error> for DbInitError {
    fn from(value: std::io::Error) -> Self {
        Self::Io(value)
    }
}

impl From<rusqlite::Error> for DbInitError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

/// Initialize a SQLite database at the given path and run migrations.
pub fn initialize_sqlite<P: AsRef<Path>>(db_path: P) -> Result<Connection, DbInitError> {
    let db_path = db_path.as_ref();
    if let Some(parent) = db_path.parent() {
        fs::create_dir_all(parent)?;
    }

    let mut connection = Connection::open(db_path)?;
    connection.pragma_update(None, "foreign_keys", "ON")?;
    connection.pragma_update(None, "journal_mode", "WAL")?;

    apply_migrations(&mut connection)?;
    Ok(connection)
}

fn apply_migrations(connection: &mut Connection) -> Result<(), DbInitError> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (name TEXT PRIMARY KEY);",
        [],
    )?;

    let mut migrations: Vec<PathBuf> = fs::read_dir(migrations_dir())?
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| path.extension().is_some_and(|ext| ext == "sql"))
        .collect();

    migrations.sort();

    for migration in migrations {
        let name = migration
            .file_name()
            .and_then(|name| name.to_str())
            .ok_or_else(|| {
                std::io::Error::new(std::io::ErrorKind::InvalidData, "invalid migration name")
            })?
            .to_owned();

        let applied: bool = connection
            .prepare_cached(
                "SELECT EXISTS(SELECT 1 FROM schema_migrations WHERE name = ?1 LIMIT 1);",
            )?
            .query_row([&name], |row| row.get(0))?;

        if applied {
            continue;
        }

        let sql = fs::read_to_string(&migration)?;
        let tx = connection.transaction()?;
        tx.execute_batch(&sql)?;
        tx.execute("INSERT INTO schema_migrations (name) VALUES (?1);", [&name])?;
        tx.commit()?;
    }

    Ok(())
}

fn migrations_dir() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("migrations")
}

#[cfg(test)]
mod tests {
    use super::initialize_sqlite;
    use rusqlite::Connection;

    #[test]
    fn runs_migrations_for_new_database() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let connection = initialize_sqlite(&db_path).expect("init should succeed");

        assert!(table_exists(&connection, "users"));
    }

    #[test]
    fn skips_already_applied_migrations() {
        let temp_dir = tempfile::tempdir().expect("temp dir");
        let db_path = temp_dir.path().join("test.db");

        let first = initialize_sqlite(&db_path).expect("first init");
        assert!(table_exists(&first, "schema_migrations"));

        // Should not error when re-run and should keep previously created tables.
        let second = initialize_sqlite(&db_path).expect("second init");
        assert!(table_exists(&second, "users"));
    }

    fn table_exists(connection: &Connection, table: &str) -> bool {
        connection
            .prepare_cached("SELECT count(*) FROM sqlite_schema WHERE type='table' AND name=?1;")
            .and_then(|mut stmt| stmt.query_row([table], |row| row.get::<_, i64>(0)))
            .map(|count| count > 0)
            .unwrap_or(false)
    }
}
