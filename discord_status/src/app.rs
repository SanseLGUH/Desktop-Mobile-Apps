use crate::core::*;
use eframe::egui;

use serde::Serialize;

use tokio::{ task::JoinHandle };
use std::sync::Arc;

use crossbeam::atomic::AtomicCell;


#[derive(Default)]
pub struct WebsocketBackend {
    task: Option<JoinHandle<()>>, 
    connection_state: Arc< AtomicCell<ConnectionState> > ,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ConnectionState {
    Disconnecting,
    Disconnected,
    Connecting,
    Connected,
    Failed,
}

impl Default for ConnectionState {
    fn default() -> Self {
        ConnectionState::Disconnected
    }
}

#[derive(Serialize, Clone)]
pub struct GatewayEvent {
    op: u8, 
    d: GatewayEventData
}

#[derive(Serialize, Clone)]
pub struct GatewayEventData {
    since: u64,
    activities: Vec< Settings >,
    status: String,
    afk: bool
}

impl GatewayEvent {
    fn from_settings( settings: Settings ) -> Self {
        let data = GatewayEventData {
            since: 91879200,
            activities: vec![ settings ],
            status: "online".to_string(),
            afk: false,
        };

        GatewayEvent {
            op: 3,
            d: data
        }
    }
}
#[derive(Serialize, Clone,Default)]
pub struct Settings {
    details: String,
    state: String,
    name: String,
    r#type: i64,
    url: String
}

#[derive(Default)]
pub struct DiscordActivityApp {
    token: String,
    websocket_backend: WebsocketBackend,
    settings: Settings,
    offline_mode: bool
}

// eframe::run_native("My egui App", native_options, Box::new(|cc| Ok(Box::new(app::MyEguiApp::new(cc)))));

impl DiscordActivityApp {
    pub fn new(cc: &eframe::CreationContext<'_>) -> Self {
        // Customize egui here with cc.egui_ctx.set_fonts and cc.egui_ctx.set_visuals.
        // Restore app state using cc.storage (requires the "persistence" feature).
        // Use the cc.gl (a glow::Context) to create graphics shaders and buffers that you can use
        // for e.g. egui::PaintCallback.
        Self::default()
    }

    fn connecting_ws(&mut self) -> Result<(), ()> {
        let ( token, payload, arc_conn_state ) = (self.token.clone(), GatewayEvent::from_settings(self.settings.clone()), self.websocket_backend.connection_state.clone());

        self.websocket_backend.task = Some( tokio::task::spawn( async move {
            arc_conn_state.store( ConnectionState::Connecting );

            match connect(&token).await {
                Ok(mut conn) => { 
                    arc_conn_state.store( ConnectionState::Connected );
                    conn.send_request( serde_json::to_string(&payload).unwrap(), 3000).await;
                },
                Err(_) => { arc_conn_state.store( ConnectionState::Failed ) }
            }
        }) );
        
        Ok(())
    }
    
    fn handle_failure(&mut self) -> Result<(), ()> {
        self.websocket_backend.connection_state.store( ConnectionState::Disconnected );        

        Ok(())
    }

    fn disconnecting_ws(&mut self) -> Result<(), ()> {
        if let Some(task) = &self.websocket_backend.task {
            task.abort();
            self.websocket_backend.task = None;
        }

        self.websocket_backend.connection_state.store( ConnectionState::Disconnected );

        Ok(())
    }
}


impl eframe::App for DiscordActivityApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        egui::CentralPanel::default().show(ctx, |ui| {
            let conn_state = self.websocket_backend.connection_state.load();

            ui.vertical_centered(|ui| {
                ui.heading("🎮 Discord Custom Activity");
                ui.label("Configure and run your custom Discord rich presence.");
                ui.add_space(10.0);

                ui.separator();

                ui.group(|ui| {
                    ui.label("📋 Activity Settings");
                    ui.add_space(5.0);

                    ui.horizontal(|ui| {
                        ui.label("Name:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.name).hint_text("Game / App Name").desired_width(200.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Details:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.details).hint_text("Status or detail").desired_width(200.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("State:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.state).hint_text("Status or detail").desired_width(200.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("URL:");
                        ui.add(egui::TextEdit::singleline(&mut self.settings.url).hint_text("https://...").desired_width(200.0));
                    });

                    ui.horizontal(|ui| {
                        ui.label("Type:");
                        ui.add(egui::DragValue::new(&mut self.settings.r#type).clamp_range(-1..=5).speed(0.3));
                        ui.label("(0: Playing, 1: Streaming, etc.)").on_hover_text("Refer to Discord activity types");
                    });

                    ui.horizontal(|ui| {
                        ui.label("🖼 Icon:");
                        ui.label("Drag and drop an image into the app");
                    });
                });

                ui.separator();

                ui.group(|ui| {
                    ui.label("🔐 Discord Token");
                    ui.add_space(5.0);
                    ui.add_enabled_ui(!self.offline_mode, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut self.token)
                                .hint_text("Paste your token here")
                                .desired_width(300.0)
                                .background_color( if conn_state == ConnectionState::Failed { egui::Color32::LIGHT_RED } else { egui::Color32::from_gray(10) }  )
                        );
                    });
                });

                ui.add_space(10.0);

                // Mode toggle and start/stop
                ui.group(|ui| {
                    let btn_label = if conn_state == ConnectionState::Connected { "⏹ Stop" } else { "▶ Start" };
                    let button = egui::Button::new(btn_label).min_size(egui::Vec2::new(65.0, 15.0));
                    
                    ui.horizontal(|ui| {
                        ui.selectable_value(&mut self.offline_mode, true, "Offline Mode");
                        ui.selectable_value(&mut self.offline_mode, false, "WebSocket Mode");
                         
                        if ui.add(button).clicked() {
                            if !self.offline_mode {
                                match conn_state {
                                    ConnectionState::Connected => {
                                        self.disconnecting_ws();
                                    }
                                    ConnectionState::Disconnected => {
                                        self.connecting_ws();          
                                    }
                                    ConnectionState::Failed => {
                                        self.handle_failure();
                                    }
                                    _ => {}
                                }
                            } 
                        }
                    });
               });
            });
        });
    }
}

