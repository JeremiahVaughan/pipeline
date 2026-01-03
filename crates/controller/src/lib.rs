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

#[derive(Debug, PartialEq, Eq)]
pub enum AppEvent {
    Ping,
    Deploy(String),
}

#[derive(Debug, PartialEq, Eq)]
pub enum ParseEventError {
    UnknownKind,
    MissingArg,
    ExtraData,
}

pub fn parse_event(text: &str) -> Result<AppEvent, ParseEventError> {
    let mut it = text.splitn(2, ':');
    let kind = it.next().unwrap_or("");
    let rest = it.next();
    match kind {
        "ping" => {
            if rest.is_some() { Err(ParseEventError::ExtraData) } else { Ok(AppEvent::Ping) }
        }
        "deploy" => match rest {
            Some(service) if !service.is_empty() => Ok(AppEvent::Deploy(service.to_string())),
            _ => Err(ParseEventError::MissingArg),
        },
        _ => Err(ParseEventError::UnknownKind),
    }
}
