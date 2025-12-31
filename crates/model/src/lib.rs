//! Domain models and data access for the MVC application.
//!
//! SQL lives here so controllers remain thin and SQLite-specific concerns stay
//! encapsulated in the model layer.

use r2d2_sqlite::{rusqlite};
use r2d2_sqlite::rusqlite::{OptionalExtension, named_params};
use std::fmt::{self, Display, Formatter};
use db;

/// Errors that can occur during model operations.
#[derive(Debug)]
pub enum ModelError {
    Sqlite(rusqlite::Error),
    Pool(r2d2::Error),
}

impl Display for ModelError {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Sqlite(err) => write!(f, "SQLite error: {err}"),
            Self::Pool(err) => write!(f, "SQLite pool error: {err}"),
        }
    }
}

impl std::error::Error for ModelError {}

impl From<rusqlite::Error> for ModelError {
    fn from(value: rusqlite::Error) -> Self {
        Self::Sqlite(value)
    }
}

impl From<r2d2::Error> for ModelError {
    fn from(value: r2d2::Error) -> Self {
        Self::Pool(value)
    }
}

/// Shared result type for model operations.
pub type ModelResult<T> = Result<T, ModelError>;

/// A simple representation of a user account.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct User {
    id: u64,
    username: String,
    email: String,
}

impl User {
    /// Create a new user.
    pub fn new(id: u64, username: impl Into<String>, email: impl Into<String>) -> Self {
        Self {
            id,
            username: username.into(),
            email: email.into(),
        }
    }

    /// Unique identifier for the user.
    pub fn id(&self) -> u64 {
        self.id
    }

    /// Username chosen by the user.
    pub fn username(&self) -> &str {
        &self.username
    }

    /// Email address associated with the account.
    pub fn email(&self) -> &str {
        &self.email
    }
}

/// SQLite-backed user model.
#[derive(Clone)]
pub struct SqliteUserModel {
}

impl SqliteUserModel {
    pub fn new() -> Self {
        Self {}
    }

    /// Insert a new user and return the created record.
    pub fn create_user(&self, username: &str, email: &str) -> ModelResult<User> {
        let conn = db::pool().get()?;
        conn.execute(
            "INSERT INTO users (username, email) VALUES (:username, :email) \
             ON CONFLICT(username) DO NOTHING;",
            named_params! {":username": username, ":email": email},
        )?;

        let changes = conn.changes();
        let id = conn.last_insert_rowid() as u64;

        if changes == 0 {
            return self
                .find_user_by_username(username)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows.into());
        }

        Ok(User::new(id, username, email))
    }

    /// Fetch a user by id.
    pub fn find_user(&self, id: u64) -> ModelResult<Option<User>> {
        let conn = db::pool().get()?;
        conn.prepare_cached("SELECT id, username, email FROM users WHERE id = ?1;")?
            .query_row([id], |row| {
                Ok(User::new(
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .optional()
            .map_err(Into::into)
    }

    /// Fetch a user by username.
    pub fn find_user_by_username(&self, username: &str) -> ModelResult<Option<User>> {
        let conn = db::pool().get()?;
        conn.prepare_cached("SELECT id, username, email FROM users WHERE username = ?1;")?
            .query_row([username], |row| {
                Ok(User::new(
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .optional()
            .map_err(Into::into)
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteUserModel, User};
    use r2d2::Pool;
    use r2d2_sqlite::SqliteConnectionManager;

    #[test]
    fn builds_user() {
        let user = User::new(1, "jill", "jill@example.com");
        assert_eq!(user.id(), 1);
        assert_eq!(user.username(), "jill");
        assert_eq!(user.email(), "jill@example.com");
    }

    #[test]
    fn creates_and_reads_user() {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).expect("pool");
        let connection = pool.get().expect("conn");
        connection
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT UNIQUE, email TEXT UNIQUE);",
            )
            .expect("create table");

        drop(connection);
        let model = SqliteUserModel::new();
        let created = model
            .create_user("test-user", "test@example.com")
            .expect("create");
        let fetched = model.find_user(created.id()).expect("find").unwrap();

        assert_eq!(created, fetched);
    }

    #[test]
    fn returns_existing_user_when_username_conflicts() {
        let manager = SqliteConnectionManager::memory();
        let pool = Pool::builder().max_size(1).build(manager).expect("pool");
        let connection = pool.get().expect("conn");
        connection
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT UNIQUE, email TEXT UNIQUE);",
            )
            .expect("create table");

        drop(connection);
        let model = SqliteUserModel::new();
        let first = model
            .create_user("dupe", "first@example.com")
            .expect("create");
        let second = model
            .create_user("dupe", "second@example.com")
            .expect("create duplicate username returns existing");

        assert_eq!(first.id(), second.id());
        assert_eq!(first.username(), second.username());
    }
}
