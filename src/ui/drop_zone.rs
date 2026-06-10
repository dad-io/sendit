//! Drop zone rendering — central panel for drag-and-drop file transfer.

use egui::{Color32, RichText, Stroke};
use tracing::{error, info};

use crate::app::SendItApp;
use crate::colors::SendItColors;
use crate::types::*;

/// State of the drop zone UI.
#[derive(Debug, Clone, PartialEq, Default)]
pub enum DropZoneState {
    /// Ready to accept files
    #[default]
    Idle,
    /// File is being dragged over the zone
    Hover,
    /// File is being sent (with cancel option)
    Sending {
        file_name: String,
        file_size: usize,
        topic_key: String,
    },
    /// File sent successfully
    Success {
        file_name: String,
        completed_at: std::time::Instant,
    },
    /// Send failed
    Error {
        message: String,
        occurred_at: std::time::Instant,
    },
}

/// Trait for drop zone rendering.
pub trait DropZoneUI {
    fn show_drop_zone(&mut self, ctx: &egui::Context, ui: &mut egui::Ui);
}

impl DropZoneUI for SendItApp {
    fn show_drop_zone(&mut self, ctx: &egui::Context, ui: &mut egui::Ui) {
        // Connection failure overlay
        if matches!(
            self.connection_status,
            ConnectionStatus::Error(_) | ConnectionStatus::Disconnected
        ) {
            self.show_connection_overlay(ui);
            return;
        }

        // Connecting spinner
        if matches!(
            self.connection_status,
            ConnectionStatus::ConnectingPublishing | ConnectionStatus::ConnectingMonitor
        ) {
            ui.vertical_centered(|ui| {
                ui.add_space(ui.available_height() / 3.0);
                ui.spinner();
                ui.label(
                    RichText::new("Connecting...")
                        .size(HEADING_MEDIUM_SIZE)
                        .color(self.text_secondary_color()),
                );
            });
            return;
        }

        // Check for file hover/drop
        let hovering = ctx.input(|i| !i.raw.hovered_files.is_empty());
        let dropped_files = ctx.input(|i| i.raw.dropped_files.clone());

        // Auto-transition from success/error back to idle after 3 seconds
        match &self.drop_zone_state {
            DropZoneState::Success { completed_at, .. } => {
                if completed_at.elapsed() > std::time::Duration::from_secs(3) {
                    self.drop_zone_state = DropZoneState::Idle;
                }
            }
            DropZoneState::Error { occurred_at, .. } => {
                if occurred_at.elapsed() > std::time::Duration::from_secs(5) {
                    self.drop_zone_state = DropZoneState::Idle;
                }
            }
            _ => {}
        }

        // Handle hover state
        if hovering
            && matches!(
                self.drop_zone_state,
                DropZoneState::Idle | DropZoneState::Hover
            )
        {
            self.drop_zone_state = DropZoneState::Hover;
        } else if !hovering && matches!(self.drop_zone_state, DropZoneState::Hover) {
            self.drop_zone_state = DropZoneState::Idle;
        }

        // Handle file drop
        if !dropped_files.is_empty()
            && !matches!(self.drop_zone_state, DropZoneState::Sending { .. })
        {
            if dropped_files.len() > 1 {
                self.drop_zone_state = DropZoneState::Error {
                    message: "Only one file at a time. Please drop a single file.".to_string(),
                    occurred_at: std::time::Instant::now(),
                };
            } else if let Some(dropped_file) = dropped_files.first() {
                self.handle_file_drop(dropped_file);
            }
        }

        // Render based on state
        match self.drop_zone_state.clone() {
            DropZoneState::Idle => self.render_idle(ui),
            DropZoneState::Hover => self.render_hover(ui),
            DropZoneState::Sending {
                file_name,
                file_size,
                topic_key,
            } => {
                self.render_sending(ui, &file_name, file_size, &topic_key);
            }
            DropZoneState::Success { file_name, .. } => self.render_success(ui, &file_name),
            DropZoneState::Error { message, .. } => self.render_error(ui, &message),
        }
    }
}

impl SendItApp {
    fn show_connection_overlay(&self, ui: &mut egui::Ui) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);

            ui.label(
                RichText::new("Connection Failed")
                    .size(HEADING_LARGE_SIZE)
                    .color(SendItColors::ERROR),
            );
            ui.add_space(8.0);

            let detail = match &self.connection_status {
                ConnectionStatus::Error(err) => err.clone(),
                ConnectionStatus::Disconnected => "Not connected".to_string(),
                _ => String::new(),
            };
            ui.label(
                RichText::new(&detail)
                    .color(self.text_secondary_color())
                    .size(TEXT_SMALL_SIZE),
            );
            ui.add_space(16.0);
            ui.label(
                RichText::new("Open settings (gear icon) to configure and reconnect")
                    .color(self.text_tertiary_color())
                    .italics(),
            );
        });
    }

    fn render_idle(&self, ui: &mut egui::Ui) {
        let available = ui.available_rect_before_wrap();
        let margin = 24.0;
        let inner_rect = available.shrink(margin);

        // Dashed border
        let stroke = Stroke::new(
            2.0,
            if self.dark_mode {
                Color32::from_gray(80)
            } else {
                Color32::from_gray(180)
            },
        );
        let painter = ui.painter();
        let dash_len = 10.0;
        let gap_len = 6.0;
        draw_dashed_rect(painter, inner_rect, stroke, dash_len, gap_len);

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(inner_rect.height() / 3.0);
                ui.label(
                    RichText::new("Drop a file here to send")
                        .size(HEADING_MEDIUM_SIZE)
                        .color(self.text_secondary_color()),
                );
                ui.add_space(8.0);
                ui.label(
                    RichText::new("Files are sent automatically on drop")
                        .size(TEXT_SMALL_SIZE)
                        .color(self.text_tertiary_color())
                        .italics(),
                );
            });
        });
    }

    fn render_hover(&self, ui: &mut egui::Ui) {
        let available = ui.available_rect_before_wrap();
        let margin = 24.0;
        let inner_rect = available.shrink(margin);

        // Green highlight border
        let stroke = Stroke::new(3.0, SendItColors::SUCCESS);
        let painter = ui.painter();
        painter.rect_stroke(inner_rect, 8.0, stroke);

        // Light green fill
        let fill = if self.dark_mode {
            Color32::from_rgba_unmultiplied(0, 200, 0, 15)
        } else {
            Color32::from_rgba_unmultiplied(0, 200, 0, 20)
        };
        painter.rect_filled(inner_rect, 8.0, fill);

        ui.allocate_new_ui(egui::UiBuilder::new().max_rect(inner_rect), |ui| {
            ui.vertical_centered(|ui| {
                ui.add_space(inner_rect.height() / 3.0);
                ui.label(
                    RichText::new("Release to send")
                        .size(HEADING_LARGE_SIZE)
                        .color(SendItColors::SUCCESS),
                );
            });
        });
    }

    fn render_sending(
        &mut self,
        ui: &mut egui::Ui,
        file_name: &str,
        file_size: usize,
        topic_key: &str,
    ) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);

            ui.label(
                RichText::new("Sending...")
                    .size(HEADING_LARGE_SIZE)
                    .color(self.text_color()),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(file_name)
                    .size(HEADING_MEDIUM_SIZE)
                    .color(self.text_secondary_color()),
            );
            ui.label(
                RichText::new(format!(
                    "{} → {}",
                    crate::transfer::format_size(file_size),
                    topic_key
                ))
                .size(TEXT_SMALL_SIZE)
                .color(self.text_tertiary_color()),
            );
            ui.add_space(16.0);
            ui.spinner();
            ui.add_space(16.0);

            if ui
                .button(RichText::new("Cancel").color(SendItColors::ERROR))
                .clicked()
            {
                info!("User cancelled file transfer for {}", file_name);
                self.drop_zone_state = DropZoneState::Idle;
                // Note: for chunked transfers, cancellation after publish is fire-and-forget
                // The worker has already received the command; we just reset UI state
            }
        });
    }

    fn render_success(&self, ui: &mut egui::Ui, file_name: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.label(
                RichText::new("Sent!")
                    .size(HEADING_LARGE_SIZE)
                    .color(if self.dark_mode {
                        SendItColors::DARK_SUCCESS
                    } else {
                        SendItColors::SUCCESS
                    }),
            );
            ui.add_space(8.0);
            ui.label(
                RichText::new(file_name)
                    .size(HEADING_MEDIUM_SIZE)
                    .color(self.text_secondary_color()),
            );
        });
    }

    fn render_error(&self, ui: &mut egui::Ui, message: &str) {
        ui.vertical_centered(|ui| {
            ui.add_space(ui.available_height() / 3.0);
            ui.label(
                RichText::new("Error")
                    .size(HEADING_LARGE_SIZE)
                    .color(SendItColors::ERROR),
            );
            ui.add_space(8.0);
            ui.label(RichText::new(message).color(self.text_secondary_color()));
        });
    }

    fn handle_file_drop(&mut self, dropped_file: &egui::DroppedFile) {
        // Get file path
        let path = match &dropped_file.path {
            Some(p) => p.clone(),
            None => {
                self.drop_zone_state = DropZoneState::Error {
                    message: "Could not determine file path".to_string(),
                    occurred_at: std::time::Instant::now(),
                };
                return;
            }
        };

        let file_name = path
            .file_name()
            .map(|n| n.to_string_lossy().to_string())
            .unwrap_or_else(|| "unknown".to_string());

        // Derive topic key: parent_dir/filename
        let topic_key = derive_topic_key(&path);

        // Read file bytes
        let bytes = match std::fs::read(&path) {
            Ok(b) => b,
            Err(e) => {
                self.drop_zone_state = DropZoneState::Error {
                    message: format!("Failed to read file: {}", e),
                    occurred_at: std::time::Instant::now(),
                };
                return;
            }
        };

        let file_size = bytes.len();
        info!(
            "File dropped: {} ({} bytes) → topic: {}",
            file_name, file_size, topic_key
        );

        // Set sending state
        self.drop_zone_state = DropZoneState::Sending {
            file_name: file_name.clone(),
            file_size,
            topic_key: topic_key.clone(),
        };

        // Send publish command
        if let Some(sender) = &self.command_sender {
            match sender.send(ZenohCommand::Publish {
                key: topic_key,
                payload: bytes,
                encoding: "application/octet-stream".to_string(),
                from_import: true,
                filename: Some(file_name.clone()),
            }) {
                Ok(_) => {
                    info!("Publish command sent for dropped file: {}", file_name);
                    self.show_tree = true;
                    // Transition to success after send (for non-chunked, this is immediate)
                    self.drop_zone_state = DropZoneState::Success {
                        file_name,
                        completed_at: std::time::Instant::now(),
                    };
                }
                Err(e) => {
                    error!("Failed to send Publish command: {:?}", e);
                    self.drop_zone_state = DropZoneState::Error {
                        message: format!("Failed to queue file for sending: {}", e),
                        occurred_at: std::time::Instant::now(),
                    };
                }
            }
        } else {
            self.drop_zone_state = DropZoneState::Error {
                message: "No connection to worker thread".to_string(),
                occurred_at: std::time::Instant::now(),
            };
        }
    }
}

/// Derive topic key from file path: one parent level + filename.
/// e.g. /Users/sam/Documents/photo.jpg → Documents/photo.jpg
/// e.g. /tmp/data.bin → tmp/data.bin
fn derive_topic_key(path: &std::path::Path) -> String {
    let file_name = path
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "unknown".to_string());

    let parent = path
        .parent()
        .and_then(|p| p.file_name().map(|n| n.to_string_lossy().to_string()));

    match parent {
        Some(dir) => format!("{}/{}", dir, file_name),
        None => file_name,
    }
}

/// Draw a dashed rectangle outline.
fn draw_dashed_rect(
    painter: &egui::Painter,
    rect: egui::Rect,
    stroke: Stroke,
    dash_len: f32,
    gap_len: f32,
) {
    let corners = [
        (rect.left_top(), rect.right_top()),
        (rect.right_top(), rect.right_bottom()),
        (rect.right_bottom(), rect.left_bottom()),
        (rect.left_bottom(), rect.left_top()),
    ];

    for (start, end) in &corners {
        let dir = (*end - *start).normalized();
        let total_len = (*end - *start).length();
        let mut pos = 0.0;
        while pos < total_len {
            let seg_start = *start + dir * pos;
            let seg_end = *start + dir * (pos + dash_len).min(total_len);
            painter.line_segment([seg_start, seg_end], stroke);
            pos += dash_len + gap_len;
        }
    }
}
