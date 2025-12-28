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

