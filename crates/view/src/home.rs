use hypertext::{ Raw, maud, prelude::* };

pub fn get_home() -> Vec<u8> {
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
                ul #messages {}

                h1 { "Rust WASM demo" }
                pre #out {}

                script {
                    (Raw::dangerously_create(r##" 
                        (function () {
                          const host = location.hostname || "localhost";
                          const protocol = location.protocol === "https:" ? "wss" : "ws";
                          const url = `${protocol}://${host}:8787`;

                          function randomInt(min, max) {
                                return Math.floor(Math.random() * (max - min + 1)) + min;
                          }

                          const heartbeatMs = randomInt(35000, 45000);
                          const baseReconnectMs = 500;
                          const maxReconnectMs = randomInt(7000, 10000);

                          let ws;
                          let reconnectTimer;
                          let heartbeatTimer;
                          let attempts = 0;
                          let last_server_contact = Date.now();
                          let last_contact_timeout = 120000;

                          const log = (...args) => console.log("[ws-demo]", ...args);

                          function connect() {
                            clearTimeout(reconnectTimer);
                            ws = new WebSocket(url);

                            ws.addEventListener("open", () => {
                              last_server_contact = Date.now();
                              log("connected", url);
                              attempts = 0;
                              startHeartbeat();
                            });

                            ws.addEventListener("message", (event) => {
                                last_server_contact = Date.now();
                                if (event.data !== "pong") {
                                    const ul = document.querySelector('#messages');
                                    const li = document.createElement('li');
                                    li.textContent = event.data;
                                    ul.appendChild(li);
                                }
                            });

                            ws.addEventListener("close", (event) => {
                              log("closed", event.code, event.reason || "clean close");
                              stopHeartbeat();
                              scheduleReconnect();
                            });

                            ws.addEventListener("error", (event) => {
                              log("socket error", event);
                              ws.close();
                            });
                          }

                          function send(payload) {
                            if (ws && ws.readyState === WebSocket.OPEN) {
                              ws.send(typeof payload === "string" ? payload : JSON.stringify(payload));
                            } else {
                              log("send skipped; socket not open");
                            }
                          }

                          function startHeartbeat() {
                            stopHeartbeat();
                            heartbeatTimer = setInterval(() => {
                              if (last_server_contact < (Date.now() - last_contact_timeout)) {
                                  scheduleReconnect()
                                  // reloading page is too agressive because it will destroy any UI
                                  // state like scroll position or unsent form fields. But still need
                                  // to fetch any data we might be missing
                                  // TODO fetch any data we might have missed
                              }
                              if (ws && ws.readyState === WebSocket.OPEN) {
                                  ws.send("ping");
                              }
                            }, heartbeatMs);
                          }

                          function stopHeartbeat() {
                            clearInterval(heartbeatTimer);
                          }

                          function scheduleReconnect() {
                            clearTimeout(reconnectTimer);
                            attempts += 1;
                            const delay = Math.min(baseReconnectMs * 2 ** attempts, maxReconnectMs);
                            log(`reconnecting in ${delay}ms`);
                            reconnectTimer = setTimeout(connect, delay);
                          }

                          function disconnect() {
                            stopHeartbeat();
                            clearTimeout(reconnectTimer);
                            if (ws && ws.readyState !== WebSocket.CLOSED) {
                              ws.close();
                            }
                          }

                          connect();

                          window.demoWebSocket = { send, disconnect };

                          const form = document.querySelector("#publish-form");
                          const input = document.querySelector("#publish-body");
                          if (form && input) {
                            form.addEventListener("submit", (event) => {
                              event.preventDefault();
                              const value = input.value.trim();
                              if (!value) return;
                              send(value);
                              input.value = "";
                            });
                          }
                        })();
                    "##))
                }
                script type="module" {
                    r#"
                    import init, { add, greet } from "/static/wasm_hello.js";

                    async function main() {
                        // Loads and initializes the .wasm
                        await init("/static/wasm_hello_bg.wasm");

                        const out = document.getElementById("out");
                        out.textContent =
                            `add(2, 3) = ${add(2, 3)}\n` +
                            `${greet("human")}\n`;
                    }

                    main();

                    "#
                }
            }
        }
    }.render().into_inner().as_bytes().to_vec()
}
