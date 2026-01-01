//! SQLite database initialization and migration runner.
//!
//! This crate centralizes setup concerns so that higher layers can stay focused
//! on business logic.

use r2d2::Pool;
use r2d2_sqlite::SqliteConnectionManager;
use r2d2_sqlite::rusqlite::{Connection, self};
use std::fmt::{self, Display, Formatter};
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;
use config::get_config;

/// Errors that can occur during database initialization.
#[derive(Debug)]
pub enum DbInitError {
    Io(std::io::Error),
    IoWithPath { path: PathBuf, source: std::io::Error },
    Sqlite(rusqlite::Error),
    Pool(r2d2::Error),
}

impl Display for DbInitError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Io(err) => write!(f, "I/O error: {err}"),
            Self::IoWithPath { path, source } => {
                write!(f, "I/O error at {path:?}: {source}")
            }
            Self::Sqlite(err) => write!(f, "SQLite error: {err}"),
            Self::Pool(err) => write!(f, "SQLite pool error: {err}"),
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

impl From<r2d2::Error> for DbInitError {
    fn from(value: r2d2::Error) -> Self {
        Self::Pool(value)
    }
}

/// Shared SQLite connection pool.
pub type DbPool = Pool<SqliteConnectionManager>;

static DB_POOL: OnceLock<DbPool> = OnceLock::new();

pub fn pool() -> &'static DbPool {
    DB_POOL.get_or_init(|| {
        let db_path = Path::new(&get_config().database_path);
        if let Some(parent) = db_path.parent() {
            fs::create_dir_all(parent)
                .unwrap_or_else(|e| panic!("error, when creating db dir. Error: {e}"));
        }

        let mut connection = Connection::open(db_path)
            .unwrap_or_else(|e| panic!("error, when opening db connection. Error: {e}"));
        set_pragmas(&mut connection);
        apply_migrations(&mut connection)
            .unwrap_or_else(|e| panic!("error, when applying migrations. Error: {e}"));

        let manager = SqliteConnectionManager::file(db_path).with_init(|conn| {
            set_pragmas(conn);
            Ok(())
        });

        let max_users = u32::try_from(get_config().max_users)
            .expect("max_users should fit into u32 for the connection pool");
        Pool::builder()
            .max_size(max_users)
            .build(manager)
            .unwrap_or_else(|e| panic!("error, when creating db connection pool. Error: {e}"))
    })
}

fn set_pragmas(connection: &mut Connection) {
    connection.pragma_update(None, "foreign_keys", "ON")
        .unwrap_or_else(|e| panic!("error, when setting foreign key pragma. Error: {e}"));
    connection.pragma_update(None, "journal_mode", "WAL")
        .unwrap_or_else(|e| panic!("error, when setting journal mode pragma. Error: {e}"));
}

fn apply_migrations(connection: &mut Connection) -> Result<(), DbInitError> {
    connection.execute(
        "CREATE TABLE IF NOT EXISTS schema_migrations (name TEXT PRIMARY KEY);",
        [],
    )?;

    let migrations_dir = Path::new(&get_config().migrations_dir);
    let mut migrations: Vec<PathBuf> = fs::read_dir(&migrations_dir)
        .map_err(|err| DbInitError::IoWithPath {
            path: migrations_dir.to_path_buf(),
            source: err,
        })?
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
