//! Settings panel rendering — collapsible section at top of left panel.

use egui::RichText;
use tracing::{error, info};

use crate::colors::SendItColors;
use crate::types::*;
use crate::app::SendItApp;

/// Trait for settings panel rendering.
pub trait SettingsUI {
    fn show_settings_panel(&mut self, ui: &mut egui::Ui);
}

impl SettingsUI for SendItApp {
    /// Renders the collapsible settings panel at the top of the left panel.
    fn show_settings_panel(&mut self, ui: &mut egui::Ui) {
        // Gear icon toggle
        ui.horizontal(|ui| {
            if ui.button("⚙").on_hover_text("Settings").clicked() {
                self.settings_open = !self.settings_open;
            }

            // Connection status dot
            let status_color = self.connection_status.color();
            ui.label(RichText::new("●").color(status_color));
            ui.label(
                RichText::new(self.connection_status.text())
                    .color(self.text_secondary_color())
                    .size(TEXT_SMALL_SIZE),
            );
        });

        if !self.settings_open {
            return;
        }

        ui.separator();

        egui::ScrollArea::vertical()
            .id_salt("settings_scroll")
            .max_height(ui.available_height() * 0.6) // Don't take more than 60% of left panel
            .show(ui, |ui| {
                // === Connection Settings ===
                ui.collapsing("Connection", |ui| {
                    self.show_connection_settings(ui);
                });

                // === Subscribe Management ===
                ui.collapsing("Subscriptions", |ui| {
                    self.show_subscribe_settings(ui);
                });

                // === Query ===
                ui.collapsing("Query", |ui| {
                    self.show_query_settings(ui);
                });

                // === Queryable ===
                ui.collapsing("Queryable", |ui| {
                    self.show_queryable_settings(ui);
                });

                // === Memory & Performance ===
                ui.collapsing("Memory & Performance", |ui| {
                    self.show_memory_settings(ui);
                });
            });

        ui.separator();
    }
}

impl SendItApp {
    fn show_connection_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Transport:");
            egui::ComboBox::from_id_salt("settings_transport")
                .width(60.0)
                .selected_text(&self.connect_transport)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.connect_transport, "tcp".to_string(), "tcp");
                    ui.selectable_value(&mut self.connect_transport, "udp".to_string(), "udp");
                    ui.selectable_value(&mut self.connect_transport, "quic".to_string(), "quic");
                    ui.selectable_value(&mut self.connect_transport, "ws".to_string(), "ws");
                    ui.selectable_value(&mut self.connect_transport, "tls".to_string(), "tls");
                });
        });

        ui.horizontal(|ui| {
            ui.label("Address:");
            ui.add(
                egui::TextEdit::singleline(&mut self.connect_address).desired_width(120.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Port:");
            ui.add(
                egui::TextEdit::singleline(&mut self.connect_port).desired_width(50.0),
            );
        });

        ui.horizontal(|ui| {
            ui.label("Mode:");
            egui::ComboBox::from_id_salt("settings_mode")
                .selected_text(&self.connection_mode)
                .show_ui(ui, |ui| {
                    ui.selectable_value(&mut self.connection_mode, "client".to_string(), "Client");
                    ui.selectable_value(&mut self.connection_mode, "peer".to_string(), "Peer");
                });
        });

        if self.connection_mode == "peer" {
            ui.horizontal(|ui| {
                ui.label("Listen Port:");
                ui.add(
                    egui::TextEdit::singleline(&mut self.listen_port).desired_width(60.0),
                );
            });
            ui.label(
                RichText::new("Use different listen ports for peers on same machine")
                    .size(TEXT_SMALL_SIZE)
                    .color(self.text_secondary_color()),
            );
        } else {
            ui.label(
                RichText::new("Client mode: connects to Zenoh router")
                    .size(TEXT_SMALL_SIZE)
                    .color(self.text_secondary_color()),
            );
        }

        // Locator preview
        let locator_preview = if self.connect_address.is_empty() {
            "(multicast discovery)".to_string()
        } else {
            format!(
                "{}/{}:{}",
                self.connect_transport, self.connect_address, self.connect_port
            )
        };
        ui.label(
            RichText::new(format!("→ {}", locator_preview))
                .size(TEXT_SMALL_SIZE - 1.0)
                .italics()
                .color(self.text_tertiary_color()),
        );

        if let ConnectionStatus::Error(ref err) = self.connection_status {
            ui.colored_label(SendItColors::ERROR, format!("Error: {}", err));
        }

        // Connect/Disconnect buttons
        if matches!(
            self.connection_status,
            ConnectionStatus::Disconnected | ConnectionStatus::Error(_)
        ) {
            if ui.button("Connect").clicked() {
                if let Some(sender) = &self.command_sender {
                    self.connection_status = ConnectionStatus::ConnectingPublishing;

                    let locators = if self.connect_address.is_empty() {
                        String::new()
                    } else {
                        format!(
                            "{}/{}:{}",
                            self.connect_transport, self.connect_address, self.connect_port
                        )
                    };

                    info!(
                        "Settings: Connect - mode: {}, locators: {}, listen_port: {}",
                        self.connection_mode, locators, self.listen_port
                    );
                    match sender.send(ZenohCommand::Connect {
                        locators,
                        listen_port: self.listen_port.clone(),
                        mode: self.connection_mode.clone(),
                        config_json: self.config_json.clone(),
                    }) {
                        Ok(_) => info!("Connect command sent"),
                        Err(e) => error!("Failed to send Connect command: {:?}", e),
                    }
                }
            }
        } else {
            if ui.button("Disconnect").clicked() {
                self.connection_status = ConnectionStatus::Disconnected;
                self.subscriptions.clear();
                if let Some(sender) = &self.command_sender {
                    let _ = sender.send(ZenohCommand::Disconnect);
                }
            }
        }
    }

    fn show_subscribe_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Key:");
            ui.text_edit_singleline(&mut self.subscribe_key);
        });
        let button = egui::Button::new("Subscribe");
        if ui
            .add_enabled(
                matches!(self.connection_status, ConnectionStatus::Connected | ConnectionStatus::WaitingForPeers)
                    && !self.subscribe_key.is_empty(),
                button,
            )
            .clicked()
        {
            if let Some(sender) = &self.command_sender {
                let _ = sender.send(ZenohCommand::Subscribe {
                    key_expr: self.subscribe_key.clone(),
                    reliability: self.subscribe_reliability.clone(),
                    mode: self.subscribe_mode.clone(),
                });
            }
        }

        // Active subscriptions
        if !self.subscriptions.is_empty() {
            ui.label(RichText::new("Active:").size(SUBSCRIPTION_TEXT_SIZE));
            for subscription in &self.subscriptions {
                ui.horizontal(|ui| {
                    ui.label(
                        RichText::new(&subscription.key_expr).size(SUBSCRIPTION_TEXT_SIZE),
                    );
                    if ui.small_button("✖").clicked() {
                        if let Some(sender) = &self.command_sender {
                            let _ = sender.send(ZenohCommand::Unsubscribe {
                                subscription_id: subscription.id.clone(),
                            });
                        }
                    }
                });
            }
        }
    }

    fn show_query_settings(&mut self, ui: &mut egui::Ui) {
        ui.label(
            RichText::new("Queries require queryables running on the network to respond.")
                .color(self.text_secondary_color())
                .size(TEXT_SMALL_SIZE),
        );

        ui.horizontal(|ui| {
            ui.label("Selector:");
            ui.text_edit_singleline(&mut self.query_selector);
        });
        ui.horizontal(|ui| {
            ui.label("Value:");
            ui.text_edit_singleline(&mut self.query_value);
        });
        ui.horizontal(|ui| {
            ui.label("Timeout (ms):");
            ui.text_edit_singleline(&mut self.query_timeout);
        });

        let button = egui::Button::new("Query");
        if ui
            .add_enabled(
                matches!(self.connection_status, ConnectionStatus::Connected | ConnectionStatus::WaitingForPeers)
                    && !self.query_selector.is_empty(),
                button,
            )
            .clicked()
        {
            if let Some(sender) = &self.command_sender {
                let timeout = self.query_timeout.parse().unwrap_or(10000);
                let _ = sender.send(ZenohCommand::Query {
                    selector: self.query_selector.clone(),
                    value: self.query_value.clone(),
                    timeout_ms: timeout,
                });
                self.query_alert = Some(format!(
                    "Query sent for '{}'. Waiting for responses...",
                    self.query_selector
                ));
            }
        }

        // Show query alert
        if let Some(alert) = &self.query_alert.clone() {
            ui.colored_label(SendItColors::WARNING, alert);
            if ui.small_button("Dismiss").clicked() {
                self.query_alert = None;
            }
        }
    }

    fn show_queryable_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Key Pattern:");
            ui.text_edit_singleline(&mut self.queryable_pattern);
        });

        ui.horizontal(|ui| {
            let was_enabled = self.queryable_enabled;
            ui.checkbox(&mut self.queryable_enabled, "Enable Queryable");

            if self.queryable_enabled {
                ui.label(
                    RichText::new("Active")
                        .color(if self.dark_mode {
                            SendItColors::DARK_SUCCESS
                        } else {
                            SendItColors::SUCCESS
                        })
                        .size(TEXT_SMALL_SIZE),
                );
            } else {
                ui.label(
                    RichText::new("Inactive")
                        .color(self.text_tertiary_color())
                        .size(TEXT_SMALL_SIZE),
                );
            }

            if was_enabled != self.queryable_enabled {
                if let Some(sender) = &self.command_sender {
                    if self.queryable_enabled {
                        let _ = sender.send(ZenohCommand::EnableQueryable {
                            key_expr: self.queryable_pattern.clone(),
                        });
                    } else {
                        let _ = sender.send(ZenohCommand::DisableQueryable);
                    }
                }
            }
        });

        ui.label(
            RichText::new("Responds to queries for keys you've published")
                .size(TEXT_SMALL_SIZE)
                .color(self.text_tertiary_color())
                .italics(),
        );
    }

    fn show_memory_settings(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            ui.label("Memory Limit (MB):");
            let mut limit_str = self.max_memory_mb.to_string();
            if ui
                .add(egui::TextEdit::singleline(&mut limit_str).desired_width(50.0))
                .changed()
            {
                if let Ok(new_limit) = limit_str.parse::<usize>() {
                    self.max_memory_mb = new_limit.max(10).min(1000);
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Message Limit:");
            let mut count_str = self.max_messages.to_string();
            if ui
                .add(egui::TextEdit::singleline(&mut count_str).desired_width(60.0))
                .changed()
            {
                if let Ok(new_limit) = count_str.parse::<usize>() {
                    self.max_messages = new_limit.max(100).min(50000);
                }
            }
        });

        ui.horizontal(|ui| {
            ui.label("Rate Limit (msg/s):");
            let mut rate_str = self.rate_limiter.max_messages_per_second.to_string();
            if ui
                .add(egui::TextEdit::singleline(&mut rate_str).desired_width(50.0))
                .changed()
            {
                if let Ok(new_rate) = rate_str.parse::<usize>() {
                    self.rate_limiter.max_messages_per_second = new_rate.max(10).min(10000);
                }
            }
        });

        ui.horizontal(|ui| {
            ui.checkbox(&mut self.dedup_enabled, "Dedup");
            if self.messages_deduped > 0 {
                ui.label(
                    RichText::new(format!("({} deduped)", self.messages_deduped))
                        .color(self.text_secondary_color())
                        .size(TEXT_SMALL_SIZE),
                );
            }
        });
    }
}
