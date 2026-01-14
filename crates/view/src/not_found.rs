use hypertext::{ Raw, maud, prelude::* };
use hypertext::Renderable;

pub fn get_not_found_app() -> String {
    maud! {
        div #app data-page="not-found" {
            h1 { "Oops!" }
            p { "couldn't help you with that dave" }
        }
    }
    .render()
    .into_inner()
}

pub fn get_not_found() -> Vec<u8> {
    let app_html = get_not_found_app();
    maud! {
        html lang="en" {
            head {
                meta charset="utf-8";
                title { "Hello!" }
            }
            body data-page="not-found" {
                (Raw::dangerously_create(&app_html))
            }
        }
    }.render().into_inner().as_bytes().to_vec()
}
