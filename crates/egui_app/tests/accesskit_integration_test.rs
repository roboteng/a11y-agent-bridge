//! Integration test to verify AccessKit is properly exposing egui widgets
//!
//! This test ensures that the egui accessibility tree is accessible via the
//! macOS Accessibility API, preventing regressions where AccessKit stops working.

#[cfg(all(test, target_os = "macos", feature = "a11y_mcp"))]
mod accesskit_tests {
    use serde_json::json;
    use serial_test::serial;
    use std::io::{BufRead, BufReader};
    use std::process::{Child, Command, Stdio};
    use std::time::Duration;
    use tokio::time::sleep;

    /// Helper struct to manage the egui app process and cleanup
    struct TestApp {
        process: Child,
        http_url: String,
        client: reqwest::Client,
    }

    impl TestApp {
        async fn start() -> Self {
            // Build the egui_app binary
            let status = Command::new("cargo")
                .args(&["build", "-p", "egui_app", "--features", "a11y_mcp"])
                .status()
                .expect("Failed to build egui_app");

            assert!(status.success(), "Failed to build egui_app");

            // Start the egui_app in the background, capturing stderr to find the port
            let mut process = Command::new("cargo")
                .args(&["run", "-p", "egui_app", "--features", "a11y_mcp"])
                .stderr(Stdio::piped())
                .spawn()
                .expect("Failed to start egui_app");

            // Read stderr to find the HTTP port
            let stderr = process.stderr.take().expect("Failed to capture stderr");
            let reader = BufReader::new(stderr);

            let mut http_url = None;
            for line in reader.lines() {
                if let Ok(line) = line {
                    eprintln!("{}", line); // Print to test output
                    if line.contains("[MCP] listening on") {
                        // Extract URL from "[MCP] listening on http://127.0.0.1:3000"
                        if let Some(start) = line.find("http://") {
                            let url = line[start..].trim().to_string();
                            http_url = Some(url);
                            break;
                        }
                    }
                }
            }

            let http_url = http_url.unwrap_or_else(|| {
                // Fallback: assume default port 3000
                "http://127.0.0.1:3000".to_string()
            });

            // Wait for server to be ready
            let client = reqwest::Client::new();
            let mut retries = 0;
            while retries < 20 {
                sleep(Duration::from_millis(500)).await;

                // Try to connect to verify server is up
                let test_request = json!({
                    "protocol_version": "1.0",
                    "method": "initialize",
                    "protocol_version": "1.0"
                });

                if let Ok(response) = client
                    .post(format!("{}/mcp", http_url))
                    .json(&test_request)
                    .send()
                    .await
                {
                    if response.status().is_success() {
                        break;
                    }
                }

                retries += 1;
            }

            assert!(retries < 20, "Server did not start within timeout");

            Self {
                process,
                http_url,
                client,
            }
        }

        async fn send_request(&self, request: serde_json::Value) -> serde_json::Value {
            let response = self
                .client
                .post(format!("{}/mcp", self.http_url))
                .json(&request)
                .send()
                .await
                .expect("Failed to send HTTP request");

            assert!(
                response.status().is_success(),
                "HTTP request failed with status: {}",
                response.status()
            );

            response
                .json()
                .await
                .expect("Failed to parse JSON response")
        }
    }

    impl Drop for TestApp {
        fn drop(&mut self) {
            // Clean up: kill the process
            let _ = self.process.kill();
            let _ = self.process.wait();

            // Give the system time to release resources
            std::thread::sleep(Duration::from_millis(500));
        }
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    #[serial]
    async fn test_accesskit_exposes_widgets() {
        let app = TestApp::start().await;

        // Test 1: Query the accessibility tree
        let query_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "query_tree": {}
                }
            }
        });

        let response = app.send_request(query_request).await;
        assert_eq!(
            response["content"]["response"]["success"]["result"]["tree"]
                .as_object()
                .is_some(),
            true,
            "Query tree failed"
        );

        // Test 2: Find all accessible nodes
        let find_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "find_by_name": {
                        "name": ""
                    }
                }
            }
        });

        let response = app.send_request(find_request).await;
        let nodes = response["content"]["response"]["success"]["result"]["nodes"]
            .as_array()
            .unwrap();

        assert!(
            nodes.len() >= 5,
            "Expected at least 5 accessible nodes (app, window, buttons, checkbox), found {}",
            nodes.len()
        );

        // Test 3: Verify we can find the window
        let window_node = nodes
            .iter()
            .find(|n| n["role"] == "AXWindow")
            .expect("AXWindow not found - AccessKit not exposing egui window!");

        assert_eq!(
            window_node["name"], "Accessibility MCP Demo",
            "Window name doesn't match"
        );

        // Test 4: Verify we can find the checkbox
        let checkbox_node = nodes
            .iter()
            .find(|n| n["role"] == "AXCheckBox")
            .expect("AXCheckBox not found - AccessKit not exposing egui checkbox!");

        assert_eq!(
            checkbox_node["name"], "Enable notifications",
            "Checkbox name doesn't match"
        );

        println!(
            "✅ AccessKit is working: found {} accessible nodes",
            nodes.len()
        );
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    #[serial]
    async fn test_slider_is_accessible_and_interactive() {
        let app = TestApp::start().await;

        // Find all nodes
        let find_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "find_by_name": {
                        "name": ""
                    }
                }
            }
        });

        let response = app.send_request(find_request).await;

        // Find the window
        let nodes = response["content"]["response"]["success"]["result"]["nodes"]
            .as_array()
            .unwrap();
        let window_node = nodes
            .iter()
            .find(|n| n["role"] == "AXWindow")
            .expect("Window not found");

        let window_id = window_node["id"].as_str().unwrap();

        // Get window's children
        let get_node_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "get_node": {
                        "node_id": window_id
                    }
                }
            }
        });

        let response = app.send_request(get_node_request).await;
        let window_children =
            response["content"]["response"]["success"]["result"]["node"]["children"]
                .as_array()
                .unwrap();

        // Find the main content group (first child is usually the content)
        let group_id = window_children[0].as_str().unwrap();

        let get_group_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "get_node": {
                        "node_id": group_id
                    }
                }
            }
        });

        let response = app.send_request(get_group_request).await;
        let group_children =
            response["content"]["response"]["success"]["result"]["node"]["children"]
                .as_array()
                .unwrap();

        // Find the slider among group children
        let mut slider_id = None;
        for child_id in group_children {
            let child_request = json!({
                "protocol_version": "1.0",
                "content": {
                    "request": {
                        "get_node": {
                            "node_id": child_id.as_str().unwrap()
                        }
                    }
                }
            });

            let child_response = app.send_request(child_request).await;
            if child_response["content"]["response"]["success"]["result"]["node"]["role"]
                == "AXSlider"
            {
                slider_id = Some(child_id.as_str().unwrap().to_string());
                break;
            }
        }

        let slider_id =
            slider_id.expect("AXSlider not found - AccessKit not exposing egui slider!");

        // Verify slider has increment/decrement actions
        let get_slider_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "get_node": {
                        "node_id": slider_id
                    }
                }
            }
        });

        let response = app.send_request(get_slider_request).await;
        let actions = response["content"]["response"]["success"]["result"]["node"]["actions"]
            .as_array()
            .unwrap();

        let has_increment = actions.iter().any(|a| a["type"] == "increment");
        let has_decrement = actions.iter().any(|a| a["type"] == "decrement");

        assert!(has_increment, "Slider missing increment action");
        assert!(has_decrement, "Slider missing decrement action");

        // Test 5: Try to increment the slider
        let increment_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "perform_action": {
                        "node_id": slider_id,
                        "action": {"type": "increment"}
                    }
                }
            }
        });

        let response = app.send_request(increment_request).await;
        assert!(
            response["content"]["response"]["success"]["result"]["action_result"]["success"]
                .as_bool()
                .unwrap(),
            "Increment action failed"
        );

        // Test 6: Try to decrement the slider
        let decrement_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "perform_action": {
                        "node_id": slider_id,
                        "action": {"type": "decrement"}
                    }
                }
            }
        });

        let response = app.send_request(decrement_request).await;
        assert!(
            response["content"]["response"]["success"]["result"]["action_result"]["success"]
                .as_bool()
                .unwrap(),
            "Decrement action failed"
        );

        println!("✅ Slider is accessible and interactive via MCP protocol");
    }

    #[tokio::test]
    #[ignore] // Run with: cargo test -- --ignored
    #[serial]
    async fn test_accesskit_lazy_init_is_disabled() {
        // This test ensures that AccessKit is initialized immediately,
        // not lazily. If AccessKit were lazy, we wouldn't see any widgets
        // until a "real" accessibility client (like VoiceOver) connected.

        let app = TestApp::start().await;

        // Give AccessKit a moment to build the initial tree
        // (it's not truly "immediate", but should be within a couple seconds)
        sleep(Duration::from_secs(2)).await;

        // Immediately after startup, we should be able to find widgets
        // without needing VoiceOver or other accessibility clients running

        let find_request = json!({
            "protocol_version": "1.0",
            "content": {
                "request": {
                    "find_by_name": {
                        "name": ""
                    }
                }
            }
        });

        let response = app.send_request(find_request).await;
        let nodes = response["content"]["response"]["success"]["result"]["nodes"]
            .as_array()
            .unwrap();

        // If AccessKit is still lazy, we'd only see 1 node (the application)
        // With enable_accesskit() called, we should see 5+ nodes
        assert!(
            nodes.len() > 1,
            "Only found {} node(s). AccessKit appears to still be using lazy initialization! \
             Expected 5+ nodes (app, window, buttons, checkbox, etc.). \
             This means ctx.enable_accesskit() is not being called or not working.",
            nodes.len()
        );

        // Verify we can see UI elements, not just the application
        let has_window = nodes.iter().any(|n| n["role"] == "AXWindow");
        let has_ui_elements = nodes.iter().any(|n| {
            let role = n["role"].as_str().unwrap_or("");
            role == "AXButton" || role == "AXCheckBox" || role == "AXSlider"
        });

        assert!(
            has_window,
            "No AXWindow found - AccessKit not exposing egui window"
        );
        assert!(
            has_ui_elements,
            "No UI elements (buttons, checkboxes, sliders) found - AccessKit lazy init still active"
        );

        println!(
            "✅ AccessKit is initialized immediately (found {} nodes)",
            nodes.len()
        );
    }
}
