//! Example egui application with MCP server
//!
//! This demonstrates how to integrate the accessibility MCP server
//! into a native application.
//!
//! Run with: cargo run --example egui_app

use accessibility_mcp::start_mcp_server;
use eframe::egui;

fn main() -> eframe::Result {
    // Start the MCP server before creating the app
    // The server runs in a background thread and exposes accessibility info via stdio
    let _mcp = start_mcp_server(None).expect("Failed to start MCP server");

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
                ui.label("The MCP server is listening on stdio.");
                ui.label("Send JSON-RPC requests to query the accessibility tree:");
                ui.monospace(r#"{"protocol_version":"1.0","method":"query_tree"}"#);
            });
        });
    }
}
