use controller::UserController;
use http::handle_http_connection;
use model::SqliteUserModel;
use view::render_user_profile;
use app::{ThreadPool};
use config::get_config;
use std::{
    thread,
    net::TcpListener,
};
use ws::handle_websocket_connection;

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

    // websocket threads
    thread::spawn(move || {
        let listener = TcpListener::bind("127.0.0.1:8787").unwrap();
        let max_users = get_config().max_users;
        let pool = ThreadPool::new(max_users); 
        for stream in listener.incoming() {
            match stream {
                Ok(s) => {
                    pool.execute(|| {
                        handle_websocket_connection(s);
                    });
                }
                Err(e) => eprintln!("websocket connection from browser failed. {e}"),
            }
        }
    });

    // http threads
    let listener = TcpListener::bind("127.0.0.1:7878").unwrap();
    let pool = ThreadPool::new(4); // for serving http files needed to get the websocket setup
    for stream in listener.incoming() {
        match stream {
            Ok(s) => {
                pool.execute(|| {
                    handle_http_connection(s);
                });
            }
            Err(e) => eprintln!("connection from browser client failed. {e}"),
        }
    }
}




