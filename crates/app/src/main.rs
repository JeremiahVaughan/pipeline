use config::{get_config, load_config};
use controller::UserController;
use db::initialize_sqlite;
use model::SqliteUserModel;
use std::error::Error;
use std::path::Path;
use std::sync::{Arc, Mutex};
use view::render_user_profile;

fn main() -> Result<(), Box<dyn Error>> {
    load_config("config/example.toml")?;
    let cfg = get_config()?;

    let connection = initialize_sqlite(Path::new(&cfg.database_path))?;
    let model = SqliteUserModel::new(Arc::new(Mutex::new(connection)));
    let controller = UserController::new(model);

    let seeded = controller.create_user("first-user", "first@example.com")?;
    match controller.get_user(seeded.id())? {
        Some(user) => println!("{}", render_user_profile(&user)),
        None => eprintln!("User not found"),
    };

    Ok(())
}
