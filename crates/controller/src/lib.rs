//! Controller layer coordinating requests between models and views.

use config::AppConfig;
use model::{ModelResult, SqliteUserModel, User};
use std::collections::HashMap;
use view::{get_landing_app, get_landing_page, get_settings_app, get_settings_page, get_service_app, get_service_page, get_not_found, get_not_found_app};

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
    SearchServices(String),
    Navigate(String),
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
        "search_services" => match rest {
            Some(service) if !service.is_empty() => Ok(AppEvent::SearchServices(service.to_string())),
            _ => Err(ParseEventError::MissingArg),
        },
        "navigate" => match rest {
            Some(path) if !path.is_empty() => Ok(AppEvent::Navigate(path.to_string())),
            _ => Err(ParseEventError::MissingArg),
        },
        _ => Err(ParseEventError::UnknownKind),
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum UiMode {
    FullPage,
    Patch,
}

#[derive(Debug, PartialEq, Eq)]
pub enum UiResult {
    FullHtml(Vec<u8>),
    Patch(String),
    NotFound(Vec<u8>),
    Redirect(String),
}

pub fn handle_nav(path: &str, query_params: HashMap<String, String>, config: &AppConfig, mode: UiMode) -> UiResult {
    match path {
        "/" => match mode {
            UiMode::FullPage => UiResult::FullHtml(get_landing_page(config)),
            UiMode::Patch => UiResult::Patch(get_landing_app(config)),
        },
        "/settings" => match mode {
            UiMode::FullPage => UiResult::FullHtml(get_settings_page(config)),
            UiMode::Patch => UiResult::Patch(get_settings_app(config)),
        },
        "/service" => match mode {
            UiMode::FullPage => UiResult::FullHtml(get_service_page(query_params, config)),
            UiMode::Patch => UiResult::Patch(get_service_app(query_params)),
        },
        _ => match mode {
            UiMode::FullPage => UiResult::NotFound(get_not_found()),
            UiMode::Patch => UiResult::Patch(get_not_found_app()),
        },
    }
}

pub fn parse_query_params(query: &str) -> HashMap<String, String> {
    query
        .split('&')
        .filter(|pair| !pair.is_empty())
        .filter_map(|pair| {
            let mut parts = pair.splitn(2, '=');
            let key = parts.next().unwrap_or_default();
            if key.is_empty() {
                return None;
            }
            let value = parts.next().unwrap_or_default();
            Some((key.to_string(), value.to_string()))
        })
        .collect()
}
