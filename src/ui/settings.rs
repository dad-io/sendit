//! Settings panel rendering — system tab bar and per-category inline content.

use egui::RichText;
use tracing::{error, info};

use crate::colors::SendItColors;
use crate::types::*;
use crate::app::SendItApp;

/// Trait for system panel tab bar and inline settings content.
pub trait SettingsUI {
    fn show_system_tab_bar(&mut self, ui: &mut egui::Ui);
    fn show_system_tab_content(&mut self, ui: &mut egui::Ui);
    fn show_settings_errors(&mut self, ui: &mut egui::Ui);
}

impl SettingsUI for SendItApp {
    /// Renders the horizontal row of category tab buttons.
    fn show_system_tab_bar(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            let tabs: &[(SystemTab, &str)] = &[
                (SystemTab::Connection, "connection"),
                (SystemTab::Subscriptions, "subscriptions"),
                (SystemTab::Query, "query"),
                (SystemTab::Queryable, "queryable"),
                (SystemTab::Memory, "memory"),
            ];
            for (tab, label) in tabs {
                let is_active = self.system_tab.as_ref() == Some(tab);
                let drop_zone_color = if self.dark_mode {
                    SendItColors::DARK_CARD_BACKGROUND
                } else {
                    SendItColors::CARD_BACKGROUND
                };
                let fill = if is_active {
                    drop_zone_color
                } else {
                    egui::Color32::from_rgb(110, 110, 115)
                };
                if ui.add(egui::Button::new(RichText::new(*label).strong().size(15.0).color(egui::Color32::WHITE))
                    .fill(fill)
                    .min_size(egui::vec2(0.0, 28.0))).clicked()
                {
                    if is_active {
                        self.system_tab = None;
                    } else {
                        self.system_tab = Some(tab.clone());
                    }
                }
            }

            // Dark/light mode toggle — no content panel, just toggles immediately
            let mode_fill = if self.dark_mode {
                egui::Color32::from_rgb(70, 70, 75)
            } else {
                egui::Color32::from_rgb(225, 225, 225)
            };
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.add(egui::Button::new(
                    RichText::new(if self.dark_mode { "dark" } else { "light" }).strong().size(15.0))
                    .fill(mode_fill)
                    .min_size(egui::vec2(0.0, 28.0))).clicked()
                {
                    self.dark_mode = !self.dark_mode;
                }
            });
        });

        ui.add_space(8.0);
    }

    /// Renders the inline horizontal content for the active tab.
    fn show_system_tab_content(&mut self, ui: &mut egui::Ui) {
        egui::ScrollArea::horizontal()
            .id_salt("system_content_scroll")
            .show(ui, |ui| {
                ui.horizontal(|ui| {
                    match self.system_tab.clone() {
                        Some(SystemTab::Connection)    => self.show_connection_inline(ui),
                        Some(SystemTab::Subscriptions) => self.show_subscriptions_inline(ui),
                        Some(SystemTab::Query)         => self.show_query_inline(ui),
                        Some(SystemTab::Queryable)     => self.show_queryable_inline(ui),
                        Some(SystemTab::Memory)        => self.show_memory_inline(ui),
                        None => {}
                    }
                });
            });
    }

    fn show_settings_errors(&mut self, ui: &mut egui::Ui) {
        ui.horizontal(|ui| {
            if let ConnectionStatus::Error(ref err) = self.connection_status {
                let msg = err.clone();
                ui.colored_label(SendItColors::ERROR, format!("connection error: {}", msg));
                ui.separator();
            }
            if let Some(alert) = self.query_alert.clone() {
                ui.colored_label(SendItColors::WARNING, &alert);
                if ui.small_button("dismiss").clicked() {
                    self.query_alert = None;
                }
            }
        });
    }
}

impl SendItApp {
    fn show_connection_inline(&mut self, ui: &mut egui::Ui) {
        ui.label("transport:");
        egui::ComboBox::from_id_salt("inline_transport")
            .width(55.0)
            .selected_text(&self.connect_transport)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.connect_transport, "tcp".to_string(),  "tcp");
                ui.selectable_value(&mut self.connect_transport, "udp".to_string(),  "udp");
                ui.selectable_value(&mut self.connect_transport, "quic".to_string(), "quic");
                ui.selectable_value(&mut self.connect_transport, "ws".to_string(),   "ws");
                ui.selectable_value(&mut self.connect_transport, "tls".to_string(),  "tls");
            });

        ui.separator();
        ui.label("address:");
        ui.add(egui::TextEdit::singleline(&mut self.connect_address).desired_width(110.0));

        ui.separator();
        ui.label("port:");
        ui.add(egui::TextEdit::singleline(&mut self.connect_port).desired_width(45.0));

        ui.separator();
        ui.label("mode:");
        egui::ComboBox::from_id_salt("inline_mode")
            .width(55.0)
            .selected_text(&self.connection_mode)
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut self.connection_mode, "client".to_string(), "Client");
                ui.selectable_value(&mut self.connection_mode, "peer".to_string(),   "Peer");
            });

        if self.connection_mode == "peer" {
            ui.separator();
            ui.label("listen port:");
            ui.add(egui::TextEdit::singleline(&mut self.listen_port).desired_width(45.0));
        }

        ui.separator();

        if matches!(self.connection_status, ConnectionStatus::Disconnected | ConnectionStatus::Error(_)) {
            if ui.button("connect").clicked() {
                if let Some(sender) = &self.command_sender {
                    self.connection_status = ConnectionStatus::ConnectingPublishing;
                    let locators = if self.connect_address.is_empty() {
                        String::new()
                    } else {
                        format!("{}/{}:{}", self.connect_transport, self.connect_address, self.connect_port)
                    };
                    info!("Connect - mode: {}, locators: {}, listen_port: {}", self.connection_mode, locators, self.listen_port);
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
        } else if ui.button("disconnect").clicked() {
            self.connection_status = ConnectionStatus::Disconnected;
            self.subscriptions.clear();
            if let Some(sender) = &self.command_sender {
                let _ = sender.send(ZenohCommand::Disconnect);
            }
        }
    }

    fn show_subscriptions_inline(&mut self, ui: &mut egui::Ui) {
        ui.label("key:");
        ui.add(egui::TextEdit::singleline(&mut self.subscribe_key).desired_width(140.0));

        ui.separator();

        let can_subscribe = matches!(
            self.connection_status,
            ConnectionStatus::Connected | ConnectionStatus::WaitingForPeers
        ) && !self.subscribe_key.is_empty();

        if ui.add_enabled(can_subscribe, egui::Button::new("subscribe")).clicked() {
            if let Some(sender) = &self.command_sender {
                let _ = sender.send(ZenohCommand::Subscribe {
                    key_expr: self.subscribe_key.clone(),
                    reliability: self.subscribe_reliability.clone(),
                    mode: self.subscribe_mode.clone(),
                });
            }
        }

        if !self.subscriptions.is_empty() {
            ui.separator();
            ui.label("active:");
            for subscription in self.subscriptions.clone() {
                ui.label(RichText::new(&subscription.key_expr).size(TEXT_SMALL_SIZE));
                if ui.small_button("×").clicked() {
                    if let Some(sender) = &self.command_sender {
                        let _ = sender.send(ZenohCommand::Unsubscribe {
                            subscription_id: subscription.id,
                        });
                    }
                }
            }
        }
    }

    fn show_query_inline(&mut self, ui: &mut egui::Ui) {
        ui.label("selector:");
        ui.add(egui::TextEdit::singleline(&mut self.query_selector).desired_width(130.0));

        ui.separator();
        ui.label("value:");
        ui.add(egui::TextEdit::singleline(&mut self.query_value).desired_width(100.0));

        ui.separator();
        ui.label("timeout (ms):");
        ui.add(egui::TextEdit::singleline(&mut self.query_timeout).desired_width(55.0));

        ui.separator();

        let can_query = matches!(
            self.connection_status,
            ConnectionStatus::Connected | ConnectionStatus::WaitingForPeers
        ) && !self.query_selector.is_empty();

        if ui.add_enabled(can_query, egui::Button::new("query")).clicked() {
            if let Some(sender) = &self.command_sender {
                let timeout = self.query_timeout.parse().unwrap_or(10000);
                let _ = sender.send(ZenohCommand::Query {
                    selector: self.query_selector.clone(),
                    value: self.query_value.clone(),
                    timeout_ms: timeout,
                });
                self.query_alert = Some(format!("Query sent for '{}'. Waiting for responses...", self.query_selector));
            }
        }

    }

    fn show_queryable_inline(&mut self, ui: &mut egui::Ui) {
        ui.label("pattern:");
        ui.add(egui::TextEdit::singleline(&mut self.queryable_pattern).desired_width(120.0));

        ui.separator();

        let was_enabled = self.queryable_enabled;
        ui.checkbox(&mut self.queryable_enabled, "enable queryable");

        let status_color = if self.queryable_enabled {
            if self.dark_mode { SendItColors::DARK_SUCCESS } else { SendItColors::SUCCESS }
        } else {
            self.text_tertiary_color()
        };
        ui.label(RichText::new(if self.queryable_enabled { "active" } else { "inactive" })
            .color(status_color)
            .size(TEXT_SMALL_SIZE));

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
    }

    fn show_memory_inline(&mut self, ui: &mut egui::Ui) {
        ui.label("memory (mb):");
        let mut limit_str = self.max_memory_mb.to_string();
        if ui.add(egui::TextEdit::singleline(&mut limit_str).desired_width(50.0)).changed() {
            if let Ok(v) = limit_str.parse::<usize>() {
                self.max_memory_mb = v.clamp(10, 1000);
            }
        }

        ui.separator();
        ui.label("message limit:");
        let mut count_str = self.max_messages.to_string();
        if ui.add(egui::TextEdit::singleline(&mut count_str).desired_width(70.0)).changed() {
            if let Ok(v) = count_str.parse::<usize>() {
                self.max_messages = v.clamp(100, 50000);
            }
        }

        ui.separator();
        ui.label("rate (msg/s):");
        let mut rate_str = self.rate_limiter.max_messages_per_second.to_string();
        if ui.add(egui::TextEdit::singleline(&mut rate_str).desired_width(55.0)).changed() {
            if let Ok(v) = rate_str.parse::<usize>() {
                self.rate_limiter.max_messages_per_second = v.clamp(10, 10000);
            }
        }

        ui.separator();
        ui.checkbox(&mut self.dedup_enabled, "dedup");
        if self.messages_deduped > 0 {
            ui.label(RichText::new(format!("({} deduped)", self.messages_deduped))
                .color(self.text_secondary_color())
                .size(TEXT_SMALL_SIZE));
        }
    }
}
