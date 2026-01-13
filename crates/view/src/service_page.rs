use hypertext::{ Raw, maud, prelude::* };
use config::AppConfig;
use std::collections::HashMap;

static WEBSOCKET_CLIENT: &str = include_str!("../../../static/ws.js"); 

pub fn get_service_page(query_params: HashMap<String, String>, config: &AppConfig) -> Vec<u8> {
    let service_name = match query_params.get("name") {
        Some(sn) => sn,
        None => "unknown", // todo handle error with validation and feedback to user
    };
    maud! {
        html {
            head {
                meta charset="utf-8";
                title { "Axe" }
                meta name="app-version" content=(&config.app_version);
                script type="module" src="/static/custom_htmx.js" defer {}
                link rel="stylesheet" href="/static/animation.css";
                link rel="stylesheet" href="/static/service_page.css";
                script {
                    (Raw::dangerously_create(WEBSOCKET_CLIENT))
                }
            }
            body data-page="service" {
                div #app data-page="service" data-css="/static/service_page.css" {
                    h1 { "Service " (service_name) }
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
                    }
                }

                // h1 { "Rust WASM demo" }
                // pre #out {}

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
