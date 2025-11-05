//! Integration test to verify AccessKit is properly exposing egui widgets
//!
//! This test ensures that the egui accessibility tree is accessible via the
//! macOS Accessibility API, preventing regressions where AccessKit stops working.

#[cfg(all(test, target_os = "macos", feature = "a11y_mcp"))]
mod accesskit_tests {
    use serde_json::json;
    use std::io::{BufRead, BufReader, Write};
    use std::os::unix::net::UnixStream;
    use std::process::{Child, Command};
    use std::thread;
    use std::time::Duration;

    /// Helper struct to manage the egui app process and cleanup
    struct TestApp {
        process: Child,
        socket_path: String,
    }

    impl TestApp {
        fn start() -> Self {
            // Build the egui_app binary
            let status = Command::new("cargo")
                .args(&["build", "-p", "egui_app", "--features", "a11y_mcp"])
                .status()
                .expect("Failed to build egui_app");

            assert!(status.success(), "Failed to build egui_app");

            // Start the egui_app in the background
            let process = Command::new("cargo")
                .args(&["run", "-p", "egui_app", "--features", "a11y_mcp"])
                .spawn()
                .expect("Failed to start egui_app");

            // Wait a bit for the app to start and create the socket
            thread::sleep(Duration::from_secs(5));

            // Find the socket file
            let pid = process.id();
            let socket_path = format!("/tmp/accessibility_mcp_{}.sock", pid);

            // Verify socket exists
            let mut retries = 0;
            while !std::path::Path::new(&socket_path).exists() && retries < 10 {
                thread::sleep(Duration::from_millis(500));
                retries += 1;
            }

            assert!(
                std::path::Path::new(&socket_path).exists(),
                "Socket file not found at {}",
                socket_path
            );

            Self {
                process,
                socket_path,
            }
        }

        fn send_request(&self, request: serde_json::Value) -> serde_json::Value {
            let mut stream =
                UnixStream::connect(&self.socket_path).expect("Failed to connect to Unix socket");

            // Send request
            let request_str = serde_json::to_string(&request).unwrap();
            writeln!(stream, "{}", request_str).expect("Failed to write request");

            // Read response
            let mut reader = BufReader::new(&stream);
            let mut response_line = String::new();
            reader
                .read_line(&mut response_line)
                .expect("Failed to read response");

            serde_json::from_str(&response_line).expect("Failed to parse response")
        }
    }

    impl Drop for TestApp {
        fn drop(&mut self) {
            // Clean up: kill the process
            let _ = self.process.kill();
            let _ = self.process.wait();
        }
    }

    #[test]
    #[ignore] // Run with: cargo test -- --ignored --test-threads=1
    fn test_accesskit_exposes_widgets() {
        let app = TestApp::start();

        // Test 1: Query the accessibility tree
        let query_request = json!({
            "protocol_version": "1.0",
            "method": "query_tree"
        });

        let response = app.send_request(query_request);
        assert_eq!(response["status"], "success", "Query tree failed");

        // Test 2: Find all accessible nodes
        let find_request = json!({
            "protocol_version": "1.0",
            "method": "find_by_name",
            "name": ""
        });

        let response = app.send_request(find_request);
        assert_eq!(response["status"], "success");

        let nodes = response["result"]["nodes"].as_array().unwrap();
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

    #[test]
    #[ignore] // Run with: cargo test -- --ignored --test-threads=1
    fn test_slider_is_accessible_and_interactive() {
        let app = TestApp::start();

        // Find all nodes
        let find_request = json!({
            "protocol_version": "1.0",
            "method": "find_by_name",
            "name": ""
        });

        let response = app.send_request(find_request);
        assert_eq!(response["status"], "success");

        // Find the window
        let nodes = response["result"]["nodes"].as_array().unwrap();
        let window_node = nodes
            .iter()
            .find(|n| n["role"] == "AXWindow")
            .expect("Window not found");

        let window_id = window_node["id"].as_str().unwrap();

        // Get window's children
        let get_node_request = json!({
            "protocol_version": "1.0",
            "method": "get_node",
            "node_id": window_id
        });

        let response = app.send_request(get_node_request);
        let window_children = response["result"]["node"]["children"].as_array().unwrap();

        // Find the main content group (first child is usually the content)
        let group_id = window_children[0].as_str().unwrap();

        let get_group_request = json!({
            "protocol_version": "1.0",
            "method": "get_node",
            "node_id": group_id
        });

        let response = app.send_request(get_group_request);
        let group_children = response["result"]["node"]["children"].as_array().unwrap();

        // Find the slider among group children
        let mut slider_id = None;
        for child_id in group_children {
            let child_request = json!({
                "protocol_version": "1.0",
                "method": "get_node",
                "node_id": child_id.as_str().unwrap()
            });

            let child_response = app.send_request(child_request);
            if child_response["result"]["node"]["role"] == "AXSlider" {
                slider_id = Some(child_id.as_str().unwrap().to_string());
                break;
            }
        }

        let slider_id =
            slider_id.expect("AXSlider not found - AccessKit not exposing egui slider!");

        // Verify slider has increment/decrement actions
        let get_slider_request = json!({
            "protocol_version": "1.0",
            "method": "get_node",
            "node_id": slider_id
        });

        let response = app.send_request(get_slider_request);
        let actions = response["result"]["node"]["actions"].as_array().unwrap();

        let has_increment = actions.iter().any(|a| a["type"] == "increment");
        let has_decrement = actions.iter().any(|a| a["type"] == "decrement");

        assert!(has_increment, "Slider missing increment action");
        assert!(has_decrement, "Slider missing decrement action");

        // Test 5: Try to increment the slider
        let increment_request = json!({
            "protocol_version": "1.0",
            "method": "perform_action",
            "node_id": slider_id,
            "action": {"type": "increment"}
        });

        let response = app.send_request(increment_request);
        assert_eq!(response["status"], "success", "Increment action failed");
        assert_eq!(
            response["result"]["success"], true,
            "Increment action reported failure"
        );

        // Test 6: Try to decrement the slider
        let decrement_request = json!({
            "protocol_version": "1.0",
            "method": "perform_action",
            "node_id": slider_id,
            "action": {"type": "decrement"}
        });

        let response = app.send_request(decrement_request);
        assert_eq!(response["status"], "success", "Decrement action failed");
        assert_eq!(
            response["result"]["success"], true,
            "Decrement action reported failure"
        );

        println!("✅ Slider is accessible and interactive via MCP protocol");
    }

    #[test]
    #[ignore] // Run with: cargo test -- --ignored --test-threads=1
    fn test_accesskit_lazy_init_is_disabled() {
        // This test ensures that AccessKit is initialized immediately,
        // not lazily. If AccessKit were lazy, we wouldn't see any widgets
        // until a "real" accessibility client (like VoiceOver) connected.

        let app = TestApp::start();

        // Immediately after startup, we should be able to find widgets
        // without needing VoiceOver or other accessibility clients running

        let find_request = json!({
            "protocol_version": "1.0",
            "method": "find_by_name",
            "name": ""
        });

        let response = app.send_request(find_request);
        assert_eq!(response["status"], "success");

        let nodes = response["result"]["nodes"].as_array().unwrap();

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
