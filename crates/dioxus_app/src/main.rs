//! Dioxus application with optional MCP server
//!
//! Run without MCP server:
//!   cargo run -p dioxus_app
//!
//! Run with MCP server enabled:
//!   cargo run -p dioxus_app --features a11y_mcp
//!
//! Dioxus ships with Tokio by default, making MCP integration seamless!

use dioxus::prelude::*;

fn main() {
    // Conditionally start the MCP server if feature is enabled
    // Dioxus already has a Tokio runtime, so we don't need to create one!
    #[cfg(feature = "a11y_mcp")]
    let _mcp = {
        let config = accessibility_mcp::Config {
            transport: accessibility_mcp::TransportKind::UnixSocket,
            socket_path: None, // Will use /tmp/accessibility_mcp_<pid>.sock
            ..Default::default()
        };

        accessibility_mcp::start_mcp_server(Some(config)).expect("Failed to start MCP server")
    };

    // Launch the Dioxus app
    launch(app);
}

#[component]
fn app() -> Element {
    let mut name = use_signal(|| String::new());
    let mut age = use_signal(|| 0u32);
    let mut notifications = use_signal(|| false);
    let mut volume = use_signal(|| 50.0f32);
    let mut count = use_signal(|| 0);

    rsx! {
        div {
            style: "padding: 20px; font-family: sans-serif;",

            h1 { "Accessibility MCP Server Demo - Dioxus" }

            hr {}

            // Feature status indicator
            div {
                style: "margin: 10px 0; padding: 10px; background: #f0f0f0; border-radius: 5px;",
                {
                    #[cfg(feature = "a11y_mcp")]
                    rsx! {
                        p { style: "color: green; font-weight: bold;", "✅ MCP server is ENABLED" }
                        p { "This app exposes its accessibility tree via MCP protocol." }
                        p {
                            style: "font-family: monospace; font-size: 12px;",
                            "Socket: /tmp/accessibility_mcp_{std::process::id()}.sock"
                        }
                    }

                    #[cfg(not(feature = "a11y_mcp"))]
                    rsx! {
                        p { style: "color: red; font-weight: bold;", "❌ MCP server is DISABLED" }
                        p { "Run with --features a11y_mcp to enable accessibility inspection." }
                    }
                }
            }

            hr {}

            // Name input
            div {
                style: "margin: 10px 0;",
                label {
                    style: "display: inline-block; width: 100px;",
                    "Name: "
                }
                input {
                    r#type: "text",
                    value: "{name}",
                    oninput: move |evt| name.set(evt.value().clone()),
                    placeholder: "Enter your name",
                }
            }

            // Age input
            div {
                style: "margin: 10px 0;",
                label {
                    style: "display: inline-block; width: 100px;",
                    "Age: "
                }
                input {
                    r#type: "number",
                    value: "{age}",
                    oninput: move |evt| {
                        if let Ok(val) = evt.value().parse::<u32>() {
                            age.set(val);
                        }
                    },
                }
            }

            // Checkbox
            div {
                style: "margin: 10px 0;",
                input {
                    r#type: "checkbox",
                    id: "notifications",
                    checked: notifications(),
                    oninput: move |evt| notifications.set(evt.checked()),
                }
                label {
                    r#for: "notifications",
                    style: "margin-left: 5px;",
                    "Enable notifications"
                }
            }

            // Slider
            div {
                style: "margin: 10px 0;",
                label {
                    style: "display: inline-block; width: 100px;",
                    "Volume: {volume():.0}"
                }
                input {
                    r#type: "range",
                    min: "0",
                    max: "100",
                    value: "{volume}",
                    oninput: move |evt| {
                        if let Ok(val) = evt.value().parse::<f32>() {
                            volume.set(val);
                        }
                    },
                }
            }

            hr {}

            // Button
            div {
                style: "margin: 10px 0;",
                button {
                    onclick: move |_| {
                        count.set(count() + 1);
                        println!("Button clicked! Count: {}", count());
                    },
                    "Click Me! (clicked {count} times)"
                }
            }

            hr {}

            // MCP Protocol Info (only shown when feature is enabled)
            #[cfg(feature = "a11y_mcp")]
            details {
                summary { "MCP Protocol Info" }
                div {
                    style: "margin: 10px; padding: 10px; background: #f9f9f9; border-radius: 5px;",
                    p {
                        "The MCP server is listening on Unix socket:"
                    }
                    p {
                        style: "font-family: monospace; background: #eee; padding: 5px;",
                        "/tmp/accessibility_mcp_{std::process::id()}.sock"
                    }
                    p { "Connect with:" }
                    p {
                        style: "font-family: monospace; background: #eee; padding: 5px;",
                        "nc -U /tmp/accessibility_mcp_{std::process::id()}.sock"
                    }
                    p { "Then send JSON-RPC requests:" }
                    p {
                        style: "font-family: monospace; background: #eee; padding: 5px;",
                        r#"{{"protocol_version":"1.0","method":"query_tree"}}"#
                    }
                }
            }
        }
    }
}
