use hypertext::{ Raw, maud, prelude::* };
use config::Service;

static WEBSOCKET_CLIENT: &str = include_str!("../../../static/ws.js"); 

pub fn get_home(services: &[Service]) -> Vec<u8> {
    maud! {
        html {
            head {
                meta charset="utf-8";
                title { "Hello!" }
                script type="module" src="/static/custom_htmx.js" defer {}
            }
            body {
                h1 { "Hello!" }
                p { "Me rust!" }
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
                    @for item in services {
                        li.item {
                            button {
                                (item.name)
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
