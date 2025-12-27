//! Domain models and data access for the MVC application.
//!
//! SQL lives here so controllers remain thin and SQLite-specific concerns stay
//! encapsulated in the model layer.

use rusqlite::{Connection, OptionalExtension, named_params};
use std::sync::{Arc, Mutex};

/// Shared result type for model operations.
pub type ModelResult<T> = Result<T, rusqlite::Error>;

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
///
/// A mutex guards the underlying connection to ensure SQLite receives only one
/// write at a time, which avoids `database is locked` errors.
#[derive(Clone)]
pub struct SqliteUserModel {
    connection: Arc<Mutex<Connection>>,
}

impl SqliteUserModel {
    pub fn new(connection: Arc<Mutex<Connection>>) -> Self {
        Self { connection }
    }

    /// Insert a new user and return the created record.
    pub fn create_user(&self, username: &str, email: &str) -> ModelResult<User> {
        let conn = self.connection.lock().expect("connection poisoned");
        conn.execute(
            "INSERT INTO users (username, email) VALUES (:username, :email) \
             ON CONFLICT(username) DO NOTHING;",
            named_params! {":username": username, ":email": email},
        )?;

        let changes = conn.changes();
        let id = conn.last_insert_rowid() as u64;
        drop(conn);

        if changes == 0 {
            return self
                .find_user_by_username(username)?
                .ok_or_else(|| rusqlite::Error::QueryReturnedNoRows);
        }

        Ok(User::new(id, username, email))
    }

    /// Fetch a user by id.
    pub fn find_user(&self, id: u64) -> ModelResult<Option<User>> {
        let conn = self.connection.lock().expect("connection poisoned");
        conn.prepare_cached("SELECT id, username, email FROM users WHERE id = ?1;")?
            .query_row([id], |row| {
                Ok(User::new(
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .optional()
    }

    /// Fetch a user by username.
    pub fn find_user_by_username(&self, username: &str) -> ModelResult<Option<User>> {
        let conn = self.connection.lock().expect("connection poisoned");
        conn.prepare_cached("SELECT id, username, email FROM users WHERE username = ?1;")?
            .query_row([username], |row| {
                Ok(User::new(
                    row.get::<_, i64>(0)? as u64,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                ))
            })
            .optional()
    }
}

#[cfg(test)]
mod tests {
    use super::{SqliteUserModel, User};
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    #[test]
    fn builds_user() {
        let user = User::new(1, "jill", "jill@example.com");
        assert_eq!(user.id(), 1);
        assert_eq!(user.username(), "jill");
        assert_eq!(user.email(), "jill@example.com");
    }

    #[test]
    fn creates_and_reads_user() {
        let connection = Connection::open_in_memory().expect("memory db");
        connection
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT UNIQUE, email TEXT UNIQUE);",
            )
            .expect("create table");

        let model = SqliteUserModel::new(Arc::new(Mutex::new(connection)));
        let created = model
            .create_user("test-user", "test@example.com")
            .expect("create");
        let fetched = model.find_user(created.id()).expect("find").unwrap();

        assert_eq!(created, fetched);
    }

    #[test]
    fn returns_existing_user_when_username_conflicts() {
        let connection = Connection::open_in_memory().expect("memory db");
        connection
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT UNIQUE, email TEXT UNIQUE);",
            )
            .expect("create table");

        let model = SqliteUserModel::new(Arc::new(Mutex::new(connection)));
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
