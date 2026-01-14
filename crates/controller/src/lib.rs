//! Controller layer coordinating requests between models and views.

use config::AppConfig;
use model::{ModelResult, SqliteUserModel, User};
use std::collections::HashMap;
use view::{get_landing_app, get_landing_app_with_services, get_landing_page, get_settings_app, get_settings_page, get_service_app, get_service_page, get_not_found, get_not_found_app};

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

pub fn get_filtered_landing_app(query: &str, config: &AppConfig) -> String {
    let query = query.trim();
    if query.is_empty() {
        return get_landing_app(config);
    }

    let query_lower = query.to_lowercase();
    let mut matches: Vec<(usize, &String)> = config
        .services
        .keys()
        .filter_map(|name| {
            let score = fuzzy_score(&query_lower, &name.to_lowercase())?;
            Some((score, name))
        })
        .collect();

    matches.sort_by(|(score_a, name_a), (score_b, name_b)| {
        score_a.cmp(score_b).then_with(|| name_a.cmp(name_b))
    });

    get_landing_app_with_services(
        matches.into_iter().map(|(_, name)| name.as_str()),
        Some(query),
    )
}

fn fuzzy_score(needle: &str, haystack: &str) -> Option<usize> {
    let mut score = 0;
    let mut last_match_end = 0;
    let mut hay_iter = haystack.char_indices();

    for needle_ch in needle.chars() {
        let mut found = None;
        while let Some((idx, hay_ch)) = hay_iter.next() {
            if hay_ch == needle_ch {
                found = Some((idx, hay_ch.len_utf8()));
                break;
            }
        }
        match found {
            Some((idx, len)) => {
                score += idx.saturating_sub(last_match_end);
                last_match_end = idx + len;
            }
            None => return None,
        }
    }

    score += haystack.len().saturating_sub(last_match_end);
    Some(score)
}
