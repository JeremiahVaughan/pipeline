use controller::UserController;
use model::SqliteUserModel;
use view::render_user_profile;

fn main() {
    let _ = db::pool();
    let model = SqliteUserModel::new();
    let controller = UserController::new(model);

    let seeded = controller.create_user("first-user", "first@example.com")
        .unwrap_or_else(|e| panic!("error, when creating user. Error: {e}"));
    match controller.get_user(seeded.id())
        .unwrap_or_else(|e| panic!("error, when fetching user. Error: {e}")) {
        Some(user) => println!("{}", render_user_profile(&user)),
        None => eprintln!("User not found"),
    };

}
