//! Example egui application with optional MCP server
//!
//! Run without MCP server:
//!   cargo run -p egui_app
//!
//! Run with MCP server enabled:
//!   cargo run -p egui_app --features a11y_mcp

use eframe::egui;

fn main() -> eframe::Result {
    // Conditionally start the MCP server if feature is enabled
    #[cfg(feature = "a11y_mcp")]
    let _mcp = {
        // Create a Tokio runtime for the MCP server
        let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
        let _runtime_guard = runtime.enter();

        // Start the MCP server before creating the app
        // Listens on /tmp/accessibility_mcp_{PID}.sock
        let handle = accessibility_mcp::start_mcp_server().expect("Failed to start MCP server");

        // Keep the runtime alive
        (runtime, handle)
    };

    // Create and run the egui app
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([400.0, 300.0])
            .with_title("Accessibility MCP Demo"),
        ..Default::default()
    };

    eframe::run_native(
        "Accessibility MCP Demo",
        options,
        Box::new(|_cc| Ok(Box::new(DemoApp::default()))),
    )
}

#[derive(Default)]
struct DemoApp {
    name: String,
    age: u32,
    checkbox: bool,
    slider_value: f32,
}

impl eframe::App for DemoApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // Force AccessKit to initialize immediately (not lazy)
        #[cfg(feature = "a11y_mcp")]
        ctx.enable_accesskit();

        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Accessibility MCP Server Demo");

            ui.separator();

            #[cfg(feature = "a11y_mcp")]
            {
                ui.label("✅ MCP server is ENABLED");
                ui.label("This app exposes its accessibility tree via MCP protocol.");
            }

            #[cfg(not(feature = "a11y_mcp"))]
            {
                ui.label("❌ MCP server is DISABLED");
                ui.label("Run with --features a11y_mcp to enable accessibility inspection.");
            }

            ui.separator();

            ui.horizontal(|ui| {
                ui.label("Name:");
                ui.text_edit_singleline(&mut self.name);
            });

            ui.horizontal(|ui| {
                ui.label("Age:");
                ui.add(egui::DragValue::new(&mut self.age));
            });

            ui.checkbox(&mut self.checkbox, "Enable notifications");

            ui.horizontal(|ui| {
                ui.label("Volume:");
                ui.add(egui::Slider::new(&mut self.slider_value, 0.0..=100.0));
            });

            ui.separator();

            if ui.button("Click Me!").clicked() {
                println!("Button was clicked!");
            }

            ui.separator();

            #[cfg(feature = "a11y_mcp")]
            ui.collapsing("MCP Protocol Info", |ui| {
                let pid = std::process::id();
                ui.label(format!("The MCP server is listening on Unix socket:"));
                ui.monospace(format!("/tmp/accessibility_mcp_{}.sock", pid));
                ui.label("Connect with:");
                ui.monospace(format!("nc -U /tmp/accessibility_mcp_{}.sock", pid));
                ui.label("Then send JSON-RPC requests:");
                ui.monospace(r#"{"protocol_version":"1.0","method":"query_tree"}"#);
            });
        });
    }
}
