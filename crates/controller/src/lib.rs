//! Controller layer coordinating requests between models and views.

use model::{ModelResult, SqliteUserModel, User};

/// Coordinates model operations for the view layer.
pub struct UserController {
    model: SqliteUserModel,
}

impl UserController {
    pub fn new(model: SqliteUserModel) -> Self {
        Self { model }
    }

    pub fn create_user(&self, username: &str, email: &str) -> ModelResult<User> {
        self.model.create_user(username, email)
    }

    pub fn get_user(&self, user_id: u64) -> ModelResult<Option<User>> {
        self.model.find_user(user_id)
    }
}

#[cfg(test)]
mod tests {
    use super::UserController;
    use model::SqliteUserModel;
    use rusqlite::Connection;
    use std::sync::{Arc, Mutex};

    #[test]
    fn returns_user_from_model() {
        let connection = Connection::open_in_memory().expect("memory db");
        connection
            .execute_batch(
                "CREATE TABLE users (id INTEGER PRIMARY KEY AUTOINCREMENT, username TEXT UNIQUE, email TEXT UNIQUE);",
            )
            .expect("create table");

        let model = SqliteUserModel::new(Arc::new(Mutex::new(connection)));
        let controller = UserController::new(model);
        let created = controller
            .create_user("controller-user", "controller@example.com")
            .expect("insert user");

        let user = controller
            .get_user(created.id())
            .expect("query user")
            .expect("user should exist");
        assert_eq!(user.username(), "controller-user");
    }
}
