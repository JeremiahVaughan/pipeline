use hypertext::maud;
use hypertext::Renderable;
use hypertext::prelude::*;

pub fn get_not_found() -> Vec<u8> {
    maud! {
        html lang="en" {
            head {
                meta charset="utf-8";
                title { "Hello!" }
            }
            body {
                h1 { "Oops!" }
                p { "couldn't help you with that dave" }
            }
        }
    }.render().into_inner().as_bytes().to_vec()
}
