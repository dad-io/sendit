//! Application struct, construction, theme helpers, and eframe::App implementation.

use eframe::egui;
use egui::{Color32, Margin, RichText};
use std::collections::{HashMap, VecDeque};
use std::sync::mpsc::{self, Receiver, Sender};
use std::sync::{Arc, RwLock};
use std::time::{Duration, Instant};
use tracing::info;

use crate::colors::SendItColors;
use crate::types::*;
use crate::ui::drop_zone::DropZoneUI;
use crate::ui::settings::SettingsUI;
use crate::ui::topic_tree::TopicTreeUI;
use crate::zenoh_worker;

/// Main application state, contains all UI state, configuration, and communication channels.
#[allow(dead_code)]
pub struct SendItApp {
    pub(crate) detail_view: DetailView,
    pub(crate) connection_status: ConnectionStatus,
    pub(crate) discovered_peers: usize,
    pub(crate) discovered_routers: usize,
    pub(crate) selected_topic: Option<String>,
    pub(crate) connect_transport: String,
    pub(crate) connect_address: String,
    pub(crate) connect_port: String,
    pub(crate) listen_port: String,
    pub(crate) connection_mode: String,
    pub(crate) config_json: String,
    pub(crate) subscribe_key: String,
    pub(crate) subscribe_reliability: String,
    pub(crate) subscribe_mode: String,
    pub(crate) publish_key: String,
    pub(crate) publish_payload: String,
    pub(crate) publish_payload_bytes: Option<Vec<u8>>,
    pub(crate) publish_payload_filename: Option<String>,
    pub(crate) publish_payload_expanded: bool,
    pub(crate) import_memory_bytes: usize,
    pub(crate) publish_encoding: String,
    pub(crate) query_selector: String,
    pub(crate) query_value: String,
    pub(crate) query_timeout: String,
    pub(crate) messages: VecDeque<ZenohMessage>,
    pub(crate) subscriptions: Vec<Subscription>,
    pub(crate) browse_tree: Arc<RwLock<ZenohNode>>,
    pub(crate) command_sender: Option<Sender<ZenohCommand>>,
    pub(crate) tree_filter: String,
    pub(crate) event_receiver: Option<Receiver<ZenohEvent>>,
    pub(crate) dark_mode: bool,
    pub(crate) max_messages: usize,
    pub(crate) max_memory_mb: usize,
    pub(crate) current_memory_bytes: usize,
    pub(crate) message_filter: String,
    pub(crate) auto_scroll: bool,
    pub(crate) query_alert: Option<String>,
    pub(crate) messages_dropped: usize,
    pub(crate) rate_limiter: RateLimiter,
    pub(crate) rate_limit_drops: usize,
    pub(crate) memory_warning_shown: bool,
    pub(crate) last_health_check: Instant,
    pub(crate) worker_healthy: bool,
    pub(crate) message_hashes: HashMap<u64, Instant>,
    pub(crate) dedup_ttl: Duration,
    pub(crate) dedup_enabled: bool,
    pub(crate) messages_deduped: usize,
    #[allow(dead_code)]
    pub(crate) local_kvstore: Arc<RwLock<HashMap<String, (String, String)>>>,
    pub(crate) queryable_enabled: bool,
    pub(crate) queryable_pattern: String,
    pub(crate) paused_keys: std::collections::HashSet<String>,
    pub(crate) json_parse_cache: std::collections::HashMap<u64, Option<String>>,
    pub(crate) expanded_payloads: std::collections::HashSet<String>,
    pub(crate) payload_store: Arc<RwLock<HashMap<String, (Vec<u8>, chrono::DateTime<chrono::Utc>)>>>,
    pub(crate) settings_open: bool,
    pub(crate) drop_zone_state: crate::ui::drop_zone::DropZoneState,
    pub(crate) show_tree: bool,
    pub(crate) tree_auto_opened: bool,
    pub(crate) system_tab: Option<SystemTab>,
}

impl Default for SendItApp {
    fn default() -> Self {
        Self::new()
    }
}

impl SendItApp {
    /// Creates a new instance of the SendIT application.
    /// Sets up communication channels and spawns the Zenoh worker thread.
    pub fn new() -> Self {
        // Create channels for worker/buffer/ui
        let (command_sender, command_receiver) = mpsc::channel();
        let (worker_event_sender, buffer_receiver) = mpsc::channel();
        let (ui_sender, event_receiver) = mpsc::channel();

        // Create shared key-value store for queryable
        let local_kvstore = Arc::new(RwLock::new(HashMap::new()));
        let kvstore_clone = local_kvstore.clone();

        // Start message buffer thread
        std::thread::spawn(move || {
            zenoh_worker::message_buffer_thread(buffer_receiver, ui_sender);
        });

        // Start the Zenoh worker in a separate async task
        std::thread::spawn(move || {
            let rt = tokio::runtime::Runtime::new().unwrap();
            rt.block_on(async {
                zenoh_worker::zenoh_worker(command_receiver, worker_event_sender, kvstore_clone)
                    .await;
            });
        });

        info!("SendItApp initialized with worker and buffer threads");

        // Auto-connect with default peer mode on launch
        info!("Auto-connecting with default peer mode, port 7447");
        let _ = command_sender.send(ZenohCommand::Connect {
            locators: String::new(), // empty = multicast discovery
            listen_port: "7447".to_string(),
            mode: "peer".to_string(),
            config_json: "{}".to_string(),
        });

        Self {
            detail_view: DetailView::TopicDetails,
            connection_status: ConnectionStatus::ConnectingPublishing,
            discovered_peers: 0,
            discovered_routers: 0,
            selected_topic: None,
            connect_transport: "tcp".to_string(),
            connect_address: "".to_string(),
            connect_port: "7447".to_string(),
            listen_port: "7447".to_string(),
            connection_mode: "peer".to_string(),
            config_json: "{}".to_string(),
            subscribe_key: "demo/**".to_string(),
            subscribe_reliability: "reliable".to_string(),
            subscribe_mode: "push".to_string(),
            publish_key: "demo/test".to_string(),
            publish_payload: "Hello Zenoh!".to_string(),
            publish_payload_bytes: None,
            publish_payload_filename: None,
            publish_payload_expanded: false,
            import_memory_bytes: 0,
            publish_encoding: "text/plain".to_string(),
            query_selector: "demo/**".to_string(),
            query_value: "".to_string(),
            query_timeout: "10000".to_string(),
            messages: VecDeque::new(),
            subscriptions: Vec::new(),
            browse_tree: Arc::new(RwLock::new(ZenohNode::new("root".to_string()))),
            command_sender: Some(command_sender),
            tree_filter: String::new(),
            event_receiver: Some(event_receiver),
            dark_mode: true,
            max_messages: 1000000,
            max_memory_mb: 100,
            current_memory_bytes: 0,
            message_filter: String::new(),
            auto_scroll: true,
            query_alert: None,
            messages_dropped: 0,
            rate_limiter: RateLimiter::new(1000),
            rate_limit_drops: 0,
            memory_warning_shown: false,
            last_health_check: Instant::now(),
            worker_healthy: true,
            message_hashes: HashMap::new(),
            dedup_ttl: Duration::from_secs(60),
            dedup_enabled: true,
            messages_deduped: 0,
            local_kvstore: Arc::new(RwLock::new(HashMap::new())),
            queryable_enabled: false,
            queryable_pattern: "**".to_string(),
            paused_keys: std::collections::HashSet::new(),
            json_parse_cache: std::collections::HashMap::new(),
            expanded_payloads: std::collections::HashSet::new(),
            payload_store: Arc::new(RwLock::new(HashMap::new())),
            settings_open: false,
            drop_zone_state: crate::ui::drop_zone::DropZoneState::default(),
            show_tree: false,
            tree_auto_opened: false,
            system_tab: None,
        }
    }

    pub(crate) fn background_color(&self) -> Color32 {
        if self.dark_mode {
            SendItColors::DARK_BACKGROUND
        } else {
            SendItColors::BACKGROUND
        }
    }

    /// Returns the appropriate button style for the current theme
    pub(crate) fn apply_theme(&self, ctx: &egui::Context) {
        ctx.request_repaint_after(std::time::Duration::from_millis(66));

        ctx.style_mut(|style| {
            style.animation_time = 0.2;
            if self.dark_mode {
                style.visuals.widgets.inactive.weak_bg_fill = SendItColors::DARK_PRIMARY;
                style.visuals.widgets.hovered.weak_bg_fill = SendItColors::DARK_PRIMARY_HOVER;
                style.visuals.widgets.active.weak_bg_fill = SendItColors::DARK_PRIMARY_HOVER;

                style.visuals.window_fill = SendItColors::DARK_BACKGROUND;
                style.visuals.panel_fill = SendItColors::DARK_CARD_BACKGROUND;
                style.visuals.extreme_bg_color = SendItColors::DARK_SURFACE;
                style.visuals.faint_bg_color = SendItColors::DARK_SIDEBAR;

                style.visuals.widgets.inactive.bg_fill = SendItColors::DARK_SURFACE;
                style.visuals.widgets.hovered.bg_fill = Color32::from_gray(70);
                style.visuals.widgets.active.bg_fill = SendItColors::DARK_SURFACE;

                style.visuals.widgets.inactive.bg_stroke.color = Color32::from_gray(100);
                style.visuals.widgets.hovered.bg_stroke.color = SendItColors::DARK_PRIMARY;
                style.visuals.widgets.active.bg_stroke.color = SendItColors::DARK_PRIMARY;

                style.visuals.widgets.inactive.fg_stroke.color = SendItColors::DARK_TEXT_PRIMARY;
                style.visuals.widgets.hovered.fg_stroke.color = SendItColors::DARK_TEXT_PRIMARY;
                style.visuals.widgets.active.fg_stroke.color = SendItColors::DARK_TEXT_PRIMARY;

                style.visuals.widgets.noninteractive.bg_fill =
                    SendItColors::DARK_CARD_BACKGROUND;
                style.visuals.widgets.noninteractive.fg_stroke.color =
                    SendItColors::DARK_TEXT_PRIMARY;

                style.visuals.code_bg_color = Color32::from_gray(30);

                style.visuals.selection.bg_fill = SendItColors::DARK_SELECTED_BACKGROUND;
                style.visuals.selection.stroke.color = SendItColors::DARK_TEXT_PRIMARY;

                style.visuals.override_text_color = Some(SendItColors::DARK_TEXT_PRIMARY);
            } else {
                style.visuals.widgets.inactive.weak_bg_fill = SendItColors::PRIMARY;
                style.visuals.widgets.hovered.weak_bg_fill = SendItColors::PRIMARY_HOVER;
                style.visuals.widgets.active.weak_bg_fill = SendItColors::PRIMARY_HOVER;

                style.visuals.window_fill = SendItColors::BACKGROUND;
                style.visuals.panel_fill = SendItColors::CARD_BACKGROUND;
                style.visuals.extreme_bg_color = SendItColors::SURFACE;
                style.visuals.faint_bg_color = SendItColors::SIDEBAR;

                style.visuals.widgets.inactive.bg_fill = Color32::WHITE;
                style.visuals.widgets.hovered.bg_fill = Color32::from_gray(250);
                style.visuals.widgets.active.bg_fill = Color32::WHITE;

                style.visuals.widgets.inactive.bg_stroke.color = Color32::from_gray(200);
                style.visuals.widgets.hovered.bg_stroke.color = SendItColors::PRIMARY;
                style.visuals.widgets.active.bg_stroke.color = SendItColors::PRIMARY;

                style.visuals.widgets.inactive.fg_stroke.color = SendItColors::TEXT_PRIMARY;
                style.visuals.widgets.hovered.fg_stroke.color = SendItColors::TEXT_PRIMARY;
                style.visuals.widgets.active.fg_stroke.color = SendItColors::TEXT_PRIMARY;

                style.visuals.widgets.noninteractive.bg_fill = SendItColors::CARD_BACKGROUND;
                style.visuals.widgets.noninteractive.fg_stroke.color = SendItColors::TEXT_PRIMARY;

                style.visuals.code_bg_color = Color32::from_gray(240);

                style.visuals.selection.bg_fill = SendItColors::SELECTED_BACKGROUND;
                style.visuals.selection.stroke.color = Color32::WHITE;

                style.visuals.override_text_color = Some(SendItColors::TEXT_PRIMARY);
            }
        });
    }

    #[allow(dead_code)]
    pub(crate) fn card_background_color(&self) -> Color32 {
        if self.dark_mode {
            SendItColors::DARK_CARD_BACKGROUND
        } else {
            SendItColors::CARD_BACKGROUND
        }
    }

    pub(crate) fn text_color(&self) -> Color32 {
        if self.dark_mode {
            SendItColors::DARK_TEXT_PRIMARY
        } else {
            SendItColors::TEXT_PRIMARY
        }
    }

    pub(crate) fn text_secondary_color(&self) -> Color32 {
        if self.dark_mode {
            SendItColors::DARK_TEXT_SECONDARY
        } else {
            SendItColors::TEXT_SECONDARY
        }
    }

    pub(crate) fn text_tertiary_color(&self) -> Color32 {
        if self.dark_mode {
            SendItColors::DARK_TEXT_TERTIARY
        } else {
            SendItColors::TEXT_TERTIARY
        }
    }

    /// Create smooth fade animation for UI elements
    pub(crate) fn animate_fade_in(&self, ctx: &egui::Context, id: &str, target: f32) -> f32 {
        ctx.animate_value_with_time(egui::Id::new(id), target, 0.001)
    }

    /// Create pulsing animation for warning indicators
    #[allow(dead_code)]
    pub(crate) fn animate_pulse(&self, ctx: &egui::Context, _id: &str) -> f32 {
        let time = ctx.input(|i| i.time) as f32;
        0.85 + (time * 3.0).sin() * 0.15
    }
}

/// Implementation of the eframe App trait for the main application.
/// This is called on each frame to update the UI.
impl eframe::App for SendItApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        // First frame debug message and ensure window is visible
        static ONCE: std::sync::Once = std::sync::Once::new();
        ONCE.call_once(|| {
            info!("First UI update frame - window should be visible now");
            ctx.send_viewport_cmd(egui::ViewportCommand::Visible(true));
            ctx.send_viewport_cmd(egui::ViewportCommand::Focus);
        });

        // Process any pending events from the Zenoh worker
        self.process_events();

        // Apply theme styling
        self.apply_theme(ctx);

        // Render the main UI panel
        egui::CentralPanel::default()
            .frame(
                egui::Frame::default()
                    .fill(self.background_color())
                    .inner_margin(Margin::same(8.0)),
            )
            .show(ctx, |ui| {
                // Toolbar: left toolbox | right app name
                ui.horizontal(|ui| {
                    // Left: files, system, theme buttons
                    if ui.add(egui::Button::new(RichText::new("files").strong().size(16.0))
                        .fill(Color32::from_rgb(35, 150, 65))
                        .min_size(egui::vec2(64.0, 28.0))).clicked()
                    {
                        self.show_tree = !self.show_tree;
                    }
                    if ui.add(egui::Button::new(RichText::new("settings").strong().size(16.0))
                        .fill(Color32::from_rgb(110, 110, 115))
                        .min_size(egui::vec2(72.0, 28.0))).clicked()
                    {
                        self.settings_open = !self.settings_open;
                        if !self.settings_open {
                            self.system_tab = None;
                        }
                    }
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        // App name (far right)
                        ui.label(
                            RichText::new("sendit")
                                .size(HEADING_LARGE_SIZE)
                                .strong()
                                .color(self.text_color()),
                        );

                        ui.separator();

                        // Connection status text
                        if matches!(
                            self.connection_status,
                            ConnectionStatus::ConnectingPublishing
                                | ConnectionStatus::ConnectingMonitor
                        ) {
                            ui.spinner();
                        }
                        ui.label(
                            RichText::new(self.connection_status.text())
                                .color(self.connection_status.color())
                                .size(TEXT_SMALL_SIZE),
                        );

                        // Peer count
                        let total_peers = self.discovered_peers + self.discovered_routers;
                        if total_peers > 0 {
                            ui.label(
                                RichText::new(format!("{} peers", total_peers))
                                    .color(self.text_tertiary_color())
                                    .size(TEXT_SMALL_SIZE),
                            );
                        }
                    });
                });

                ui.separator();

                // System panel: category tab bar (slides down from toolbar)
                let bg = self.background_color();
                egui::TopBottomPanel::top("system_tabs")
                    .resizable(false)
                    .frame(egui::Frame::none().fill(bg).inner_margin(egui::Margin { left: 0.0, right: 0.0, top: 0.0, bottom: 0.0 }))
                    .show_animated_inside(ui, self.settings_open, |ui| {
                        self.show_system_tab_bar(ui);
                    });

                // System panel: selected tab content (slides down below tab bar)
                egui::TopBottomPanel::top("system_content")
                    .resizable(false)
                    .show_animated_inside(ui, self.system_tab.is_some(), |ui| {
                        self.show_system_tab_content(ui);
                    });

                // Split panel layout: tree on left, drop zone in center
                egui::SidePanel::left("tree_panel")
                    .default_width(300.0)
                    .min_width(200.0)
                    .resizable(true)
                    .show_animated_inside(ui, self.show_tree, |ui| {
                        self.show_tree_panel(ui);
                    });

                // Right slide-out panel: topic details (when a topic is selected)
                if self.selected_topic.is_some() {
                    egui::SidePanel::right("detail_panel")
                        .default_width(400.0)
                        .min_width(250.0)
                        .resizable(true)
                        .show_inside(ui, |ui| {
                            // Close button
                            ui.horizontal(|ui| {
                                if ui.button("✖ Close").clicked() {
                                    self.selected_topic = None;
                                }
                            });
                            ui.separator();
                            self.show_topic_details(ui);
                        });
                }

                // Central panel: drop zone
                egui::CentralPanel::default().show_inside(ui, |ui| {
                    self.show_drop_zone(ctx, ui);
                });
            });

        // repaint for real-time message updates (throttled to ~15fps)
        ctx.request_repaint_after(std::time::Duration::from_millis(66));
    }
}
