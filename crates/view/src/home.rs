use hypertext::{ Raw, maud, prelude::* };
use std::collections::BTreeMap;
use config::ServiceConfig;

static WEBSOCKET_CLIENT: &str = include_str!("../../../static/ws.js"); 
static HOME_CSS: &str = include_str!("../../../static/home.css"); 

pub fn get_home(services: &BTreeMap<String, Vec<ServiceConfig>>) -> Vec<u8> {
    maud! {
        html {
            head {
                meta charset="utf-8";
                title { "Hello!" }
                script type="module" src="/static/custom_htmx.js" defer {}
                style {
                    (Raw::dangerously_create(HOME_CSS))
                }
                link rel="stylesheet" href="/static/animation.css";
            }
            body {
                h1 { "Hello!" }
                p { "Me rust!" }
                img.firetruck src="/static/firetruck.svg" loading="lazy" alt="firetruck" width="96" height="96";
                img.ambulance src="/static/ambulance.svg" loading="lazy" alt="ambulance" width="96" height="96";
                img.police src="/static/police.svg" loading="lazy" alt="police" width="50" height="50";

                form #publish-form {
                    label {
                        "Message:"
                        input #publish-body 
                              type="text" 
                              placeholder="Write a message for subject 'demo'";
                    }
                    button type="submit" {
                        "Publish"
                    }
                }
                h2 { "Messages on 'demo'" }
                ul #messages {
                    @for (name, _) in services {
                        li.item {
                            button {
                                (name)
                            }
                        }
                    }
                }

                // h1 { "Rust WASM demo" }
                // pre #out {}

                script {
                    (Raw::dangerously_create(WEBSOCKET_CLIENT))
                }
                // script type="module" {
                //     r#"
                //     import init, { add, greet } from "/static/wasm_hello.js";

                //     async function main() {
                //         // Loads and initializes the .wasm
                //         await init("/static/wasm_hello_bg.wasm");

                //         const out = document.getElementById("out");
                //         out.textContent =
                //             `add(2, 3) = ${add(2, 3)}\n` +
                //             `${greet("human")}\n`;
                //     }

                //     main();

                //     "#
                // }
            }
        }
    }.render().into_inner().as_bytes().to_vec()
}
