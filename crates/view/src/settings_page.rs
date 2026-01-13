use hypertext::{ Raw, maud, prelude::* };
use config::AppConfig;

static WEBSOCKET_CLIENT: &str = include_str!("../../../static/ws.js"); 

pub fn get_settings_page(config: &'static AppConfig) -> Vec<u8> {
    maud! {
        html {
            head {
                meta charset="utf-8";
                title { "Axe" }
                meta name="app-version" content=(&config.app_version);
                script type="module" src="/static/custom_htmx.js" defer {}
                link rel="stylesheet" href="/static/settings_page.css";
                link rel="stylesheet" href="/static/animation.css";
            }
            body data-page="settings" {
                div #app data-page="settings" data-css="/static/settings_page.css" {
                    h1 { "settings" }
                    img.firetruck src="/static/firetruck.svg" loading="lazy" alt="firetruck" width="96" height="96";
                    img.ambulance src="/static/ambulance.svg" loading="lazy" alt="ambulance" width="96" height="96";
                    img.police src="/static/police.svg" loading="lazy" alt="police" width="50" height="50";

                    h2 { "Services" }
                    ul {
                        @for (name, _) in &config.services {
                            li {
                                (name)
                            }
                        }
                    }

                    h2 { "Nodes" }
                    div #messages {
                        @for (name, node_config) in &config.nodes {
                            div.item {
                                div {
                                    (name)
                                }
                                div {
                                    (node_config.host_name)
                                }
                            }
                        }
                    }

                    h2 { "CI Nodes" }
                    div.ci {
                        @for name in &config.ci.nodes {
                            div.item {
                                (name)
                            }
                        }
                    }

                    h2 { "Environments" }
                    div.env {
                        @for (name, env_config) in &config.environments {
                            div.item {
                                (name) br; br;
                                @for node_name in &env_config.nodes {
                                    div {
                                        "nodes:"
                                    }
                                    div.item {
                                        (node_name)
                                    }
                                }
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
