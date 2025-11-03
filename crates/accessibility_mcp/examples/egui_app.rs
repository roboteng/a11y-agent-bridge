//! Example egui application with MCP server
//!
//! This demonstrates how to integrate the accessibility MCP server
//! into a native application.
//!
//! Run with: cargo run --example egui_app

use accessibility_mcp::{start_mcp_server, Config, TransportKind};
use eframe::egui;

fn main() -> eframe::Result {
    // Create a Tokio runtime for the MCP server
    let runtime = tokio::runtime::Runtime::new().expect("Failed to create Tokio runtime");
    let _runtime_guard = runtime.enter();

    // Start the MCP server before creating the app
    // Use Unix socket for GUI apps so we can communicate while it's running
    let config = Config {
        transport: TransportKind::UnixSocket,
        socket_path: None, // Will use /tmp/accessibility_mcp_<pid>.sock
        ..Default::default()
    };
    let _mcp = start_mcp_server(Some(config)).expect("Failed to start MCP server");

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
        egui::CentralPanel::default().show(ctx, |ui| {
            ui.heading("Accessibility MCP Server Demo");

            ui.separator();

            ui.label("This app has an MCP server running that exposes its accessibility tree.");
            ui.label("You can query it via stdin/stdout using the MCP protocol.");

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

            ui.collapsing("MCP Protocol Info", |ui| {
                let pid = std::process::id();
                ui.label(format!("The MCP server is listening on Unix socket:"));
                ui.monospace(format!("/tmp/accessibility_mcp_{}.sock", pid));
                ui.label("Connect with: nc -U /tmp/accessibility_mcp_<pid>.sock");
                ui.label("Then send JSON-RPC requests:");
                ui.monospace(r#"{"protocol_version":"1.0","method":"query_tree"}"#);
            });
        });
    }
}
