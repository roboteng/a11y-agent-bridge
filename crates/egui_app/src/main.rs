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
    let (_runtime, mcp_handle) =
        accessibility_mcp::start_all().expect("Failed to start MCP server");

    #[cfg(feature = "a11y_mcp")]
    let mcp_port = mcp_handle.port;

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
        Box::new(move |_cc| {
            #[cfg(feature = "a11y_mcp")]
            return Ok(Box::new(DemoApp::new(mcp_port)));

            #[cfg(not(feature = "a11y_mcp"))]
            return Ok(Box::new(DemoApp::default()));
        }),
    )
}

struct DemoApp {
    name: String,
    age: u32,
    checkbox: bool,
    slider_value: f32,
    #[cfg(feature = "a11y_mcp")]
    mcp_port: u16,
}

impl Default for DemoApp {
    fn default() -> Self {
        Self {
            name: String::new(),
            age: 0,
            checkbox: false,
            slider_value: 0.0,
            #[cfg(feature = "a11y_mcp")]
            mcp_port: 0,
        }
    }
}

impl DemoApp {
    #[cfg(feature = "a11y_mcp")]
    fn new(mcp_port: u16) -> Self {
        Self {
            name: String::new(),
            age: 0,
            checkbox: false,
            slider_value: 0.0,
            mcp_port,
        }
    }
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
                ui.label("The MCP server is listening on HTTP:");
                ui.monospace(format!("http://127.0.0.1:{}/mcp", self.mcp_port));
                ui.label("Connect with curl:");
                ui.monospace(format!(
                    "curl -X POST http://127.0.0.1:{}/mcp -H 'Content-Type: application/json' -d '{{\"protocol_version\":\"1.0\",\"content\":{{\"request\":{{\"query_tree\":{{}}}}}}}}'",
                    self.mcp_port
                ));
            });
        });
    }
}
