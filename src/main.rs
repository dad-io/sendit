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

/// Detach from the launching terminal on macOS.
/// The original process exits immediately (returning shell control), and a
/// respawned child carries on with the GUI. Set SENDIT_LAUNCHED=1 to skip
/// detach (useful when you need to see log output in the terminal).
#[cfg(target_os = "macos")]
fn detach_from_terminal() {
    if std::env::var("SENDIT_LAUNCHED").is_err() {
        if let Ok(exe) = std::env::current_exe() {
            let _ = std::process::Command::new(exe)
                .args(std::env::args_os().skip(1))
                .env("SENDIT_LAUNCHED", "1")
                .stdin(std::process::Stdio::null())
                .stdout(std::process::Stdio::null())
                .stderr(std::process::Stdio::null())
                .spawn();
        }
        std::process::exit(0);
    }
}

/// Application entry point.
fn main() -> eframe::Result<()> {
    #[cfg(target_os = "macos")]
    detach_from_terminal();

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
            .with_inner_size([800.0, 600.0])
            .with_min_inner_size([550.0, 400.0])
            .with_title("sendit")
            .with_visible(true)
            .with_active(true),
        renderer: eframe::Renderer::Glow,
        hardware_acceleration: eframe::HardwareAcceleration::Preferred,
        ..Default::default()
    };

    eframe::run_native(
        "sendit",
        options,
        Box::new(|_cc| {
            info!("Creating SendIT instance...");
            Ok(Box::new(SendItApp::new()))
        }),
    )
}
