use hypertext::{ Raw, maud, prelude::* };
use config::AppConfig;

static WEBSOCKET_CLIENT: &str = include_str!("../../../static/ws.js"); 

pub fn get_landing_app(config: &AppConfig) -> String {
    get_landing_app_with_services(config.services.keys().map(String::as_str), None)
}

pub fn get_landing_app_with_services<'a, I>(services: I, search_value: Option<&str>) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let search_value = search_value.unwrap_or("");
    let services: Vec<&'a str> = services.into_iter().collect();
    maud! {
        div #app data-page="landing" data-css="/static/landing_page.css" data-js="/static/landing_page.js" {
            p { "Services" }
            img.firetruck src="/static/firetruck.svg" loading="lazy" alt="firetruck" width="96" height="96";
            img.ambulance src="/static/ambulance.svg" loading="lazy" alt="ambulance" width="96" height="96";
            img.police src="/static/police.svg" loading="lazy" alt="police" width="50" height="50";

            form #publish-form {
                input #search
                      type="text" 
                      value=(search_value)
                      placeholder="search...";
            }
            ul #messages {
                @for name in &services {
                    li.item {
                        a.item-link href=(format!("/service?name={}", name)) {
                            (name)
                        }
                    }
                }
            }
        }
    }
    .render()
    .into_inner()
}

pub fn get_landing_services_oob<'a, I>(services: I) -> String
where
    I: IntoIterator<Item = &'a str>,
{
    let services: Vec<&'a str> = services.into_iter().collect();
    maud! {
        ul #messages hx-swap-oob="true" {
            @for name in &services {
                li.item {
                    a.item-link href=(format!("/service?name={}", name)) {
                        (name)
                    }
                }
            }
        }
    }
    .render()
    .into_inner()
}

pub fn get_landing_page(config: &AppConfig) -> Vec<u8> {
    let app_html = get_landing_app(config);
    maud! {
        html {
            head {
                meta charset="utf-8";
                title { "Axe" }
                meta name="app-version" content=(&config.app_version);
                script type="module" src="/static/custom_htmx.js" defer {}
                link rel="stylesheet" href="/static/landing_page.css";
                script {
                    (Raw::dangerously_create(WEBSOCKET_CLIENT))
                }
                link rel="stylesheet" href="/static/animation.css";
            }
            body data-page="landing" {
                (Raw::dangerously_create(&app_html))

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
