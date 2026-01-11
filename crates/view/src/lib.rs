//! View helpers for rendering model data to text.

pub mod landing_page;
pub use landing_page::get_landing_page;
pub mod settings_page;
pub use settings_page::get_settings_page;
pub mod service_page;
pub use service_page::get_service_page;
pub mod not_found;
pub use not_found::get_not_found;

use model::User;

/// Render a user profile into a simple string representation.
pub fn render_user_profile(user: &User) -> String {
    format!(
        "User #{id}\nusername: {username}\nemail: {email}",
        id = user.id(),
        username = user.username(),
        email = user.email()
    )
}

#[cfg(test)]
mod tests {
    use super::render_user_profile;
    use model::User;

    #[test]
    fn renders_profile() {
        let user = User::new(7, "rendered", "rendered@example.com");
        let rendered = render_user_profile(&user);

        assert!(rendered.contains("User #7"));
        assert!(rendered.contains("rendered@example.com"));
    }
}
