// Hide console window on Windows in release builds
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

//! SendIT - A drag-and-drop file transfer tool over Zenoh networks.

mod app;
mod colors;
mod events;
mod transfer;
mod types;
mod ui;
mod zenoh_worker;

use app::SendItApp;
use eframe::egui;
use tracing::info;

/// Application entry point.
fn main() -> eframe::Result<()> {
    #[cfg(target_os = "windows")]
    {
        std::panic::set_hook(Box::new(|panic_info| {
            let msg = format!("SendIT crashed: {}\n", panic_info);
            if let Ok(exe_path) = std::env::current_exe() {
                let log_path = exe_path.with_file_name("crash.log");
                let _ = std::fs::write(&log_path, &msg);
            }
            if let Some(home) = std::env::var_os("USERPROFILE") {
                let log_path = std::path::PathBuf::from(home).join("send-it-crash.log");
                let _ = std::fs::write(log_path, &msg);
            }
        }));
    }

    tracing_subscriber::fmt::init();

    info!("SendIT starting...");

    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_inner_size([1400.0, 900.0])
            .with_min_inner_size([1000.0, 600.0])
            .with_title("SendIT")
            .with_visible(true)
            .with_active(true),
        renderer: eframe::Renderer::Glow,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        ..Default::default()
    };

    eframe::run_native(
        "SendIT",
        options,
        Box::new(|_cc| {
            info!("Creating SendIT instance...");
            Ok(Box::new(SendItApp::new()))
        }),
    )
}
