//! ClawTerm UI: a Warp-style tabbed window where every tab is a live
//! conversation with one OpenClaw agent session.

use crate::backend::{Command, Event};
use crate::state::{self, PersistedState, PersistedTab};
use clawterm_gateway::{AgentInfo, ModelInfo};
use eframe::egui::{
    self, Align, CentralPanel, Color32, ComboBox, FontId, Key, Layout, RichText, ScrollArea,
    TextEdit, TopBottomPanel,
};
use std::sync::mpsc::Receiver;
use tokio::sync::mpsc::UnboundedSender;

const USER_COLOR: Color32 = Color32::from_rgb(122, 199, 255);
const AGENT_COLOR: Color32 = Color32::from_rgb(220, 220, 220);
const INFO_COLOR: Color32 = Color32::from_rgb(140, 140, 150);
const ERROR_COLOR: Color32 = Color32::from_rgb(255, 120, 120);
const ACCENT: Color32 = Color32::from_rgb(255, 99, 71);

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Role {
    User,
    Agent,
    Info,
    Error,
}

#[derive(Debug, Clone)]
struct Block {
    role: Role,
    text: String,
}

#[derive(Debug)]
struct Tab {
    id: u64,
    agent_id: String,
    session_key: String,
    model_ref: Option<String>,
    blocks: Vec<Block>,
    input: String,
    /// Index into `blocks` of the assistant block currently streaming.
    streaming_block: Option<usize>,
    session_ready: bool,
    focus_input: bool,
}

impl Tab {
    fn new(id: u64, agent_id: String) -> Self {
        let session_key = format!(
            "agent:{}:clawterm-{}",
            agent_id,
            &uuid::Uuid::new_v4().to_string()[..8]
        );
        Self {
            id,
            agent_id,
            session_key,
            model_ref: None,
            blocks: Vec::new(),
            input: String::new(),
            streaming_block: None,
            session_ready: false,
            focus_input: true,
        }
    }

    fn restored(id: u64, p: &PersistedTab) -> Self {
        Self {
            id,
            agent_id: p.agent_id.clone(),
            session_key: p.session_key.clone(),
            model_ref: p.model_ref.clone(),
            blocks: Vec::new(),
            input: String::new(),
            streaming_block: None,
            session_ready: false,
            focus_input: false,
        }
    }

    fn push(&mut self, role: Role, text: impl Into<String>) {
        self.blocks.push(Block {
            role,
            text: text.into(),
        });
    }
}

pub struct ClawTermApp {
    commands: UnboundedSender<Command>,
    events: Receiver<Event>,
    agents: Vec<AgentInfo>,
    models: Vec<ModelInfo>,
    tabs: Vec<Tab>,
    active: usize,
    next_tab_id: u64,
    status: String,
    connected: bool,
}

impl ClawTermApp {
    pub fn new(commands: UnboundedSender<Command>, events: Receiver<Event>) -> Self {
        let persisted = state::load();
        let mut tabs = Vec::new();
        let mut next_tab_id = 1u64;
        for p in &persisted.tabs {
            let tab = Tab::restored(next_tab_id, p);
            next_tab_id += 1;
            tabs.push(tab);
        }
        let active = persisted.active_tab.min(tabs.len().saturating_sub(1));
        Self {
            commands,
            events,
            agents: Vec::new(),
            models: Vec::new(),
            tabs,
            active,
            next_tab_id,
            status: "connecting to gateway…".into(),
            connected: false,
        }
    }

    fn persist(&self) {
        let snapshot = PersistedState {
            tabs: self
                .tabs
                .iter()
                .map(|t| PersistedTab {
                    agent_id: t.agent_id.clone(),
                    session_key: t.session_key.clone(),
                    model_ref: t.model_ref.clone(),
                    title: None,
                })
                .collect(),
            active_tab: self.active,
        };
        state::save(&snapshot);
    }

    fn agent_name(&self, agent_id: &str) -> String {
        self.agents
            .iter()
            .find(|a| a.id == agent_id)
            .map(|a| a.display_name().to_owned())
            .unwrap_or_else(|| agent_id.to_owned())
    }

    fn default_agent(&self) -> String {
        self.agents
            .first()
            .map(|a| a.id.clone())
            .unwrap_or_else(|| "main".into())
    }

    fn open_tab(&mut self, agent_id: String) {
        let tab = Tab::new(self.next_tab_id, agent_id.clone());
        self.next_tab_id += 1;
        let _ = self.commands.send(Command::EnsureSession {
            tab_id: tab.id,
            key: tab.session_key.clone(),
            agent_id,
        });
        self.tabs.push(tab);
        self.active = self.tabs.len() - 1;
        self.persist();
    }

    /// Rebind a tab to a different agent: fresh session under the new agent.
    fn switch_agent(&mut self, tab_idx: usize, agent_id: String) {
        let tab_id = self.tabs[tab_idx].id;
        let fresh = Tab::new(tab_id, agent_id.clone());
        let tab = &mut self.tabs[tab_idx];
        tab.agent_id = agent_id.clone();
        tab.session_key = fresh.session_key;
        tab.model_ref = None;
        tab.blocks.clear();
        tab.streaming_block = None;
        tab.session_ready = false;
        let _ = self.commands.send(Command::EnsureSession {
            tab_id,
            key: self.tabs[tab_idx].session_key.clone(),
            agent_id,
        });
        self.persist();
    }

    fn handle_events(&mut self) {
        while let Ok(ev) = self.events.try_recv() {
            match ev {
                Event::Ready { agents, models } => {
                    self.agents = agents;
                    self.models = models;
                    self.connected = true;
                    self.status = "connected".into();
                    if self.tabs.is_empty() {
                        self.open_tab(self.default_agent());
                    } else {
                        // Reattach restored tabs to their gateway sessions.
                        for tab in &self.tabs {
                            let _ = self.commands.send(Command::EnsureSession {
                                tab_id: tab.id,
                                key: tab.session_key.clone(),
                                agent_id: tab.agent_id.clone(),
                            });
                        }
                    }
                }
                Event::SessionReady { tab_id, key } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if tab.session_key == key {
                            tab.session_ready = true;
                            if tab.blocks.is_empty() {
                                let _ = self.commands.send(Command::LoadHistory {
                                    tab_id,
                                    key: key.clone(),
                                });
                            }
                        }
                    }
                }
                Event::History {
                    tab_id,
                    key,
                    messages,
                } => {
                    if let Some(tab) = self.tabs.iter_mut().find(|t| t.id == tab_id) {
                        if tab.session_key == key && tab.streaming_block.is_none() {
                            tab.blocks.clear();
                            for m in messages {
                                let role = match m.role.as_str() {
                                    "user" => Role::User,
                                    "assistant" => Role::Agent,
                                    _ => Role::Info,
                                };
                                tab.blocks.push(Block { role, text: m.text });
                            }
                        }
                    }
                }
                Event::Delta { key, text, replace } => {
                    for tab in self.tabs.iter_mut().filter(|t| t.session_key == key) {
                        let idx = match tab.streaming_block {
                            Some(i) => i,
                            None => {
                                tab.push(Role::Agent, String::new());
                                let i = tab.blocks.len() - 1;
                                tab.streaming_block = Some(i);
                                i
                            }
                        };
                        if replace {
                            tab.blocks[idx].text = text.clone();
                        } else {
                            tab.blocks[idx].text.push_str(&text);
                        }
                    }
                }
                Event::RunDone { key, text, error } => {
                    for tab in self.tabs.iter_mut().filter(|t| t.session_key == key) {
                        match tab.streaming_block.take() {
                            Some(i) => {
                                // Prefer the final cumulative text when present.
                                if let Some(t) = &text {
                                    tab.blocks[i].text = t.clone();
                                }
                                if tab.blocks[i].text.is_empty() {
                                    tab.blocks.remove(i);
                                }
                            }
                            None => {
                                if let Some(t) = &text {
                                    tab.push(Role::Agent, t.clone());
                                }
                            }
                        }
                        if let Some(err) = &error {
                            tab.push(Role::Error, err.clone());
                        }
                    }
                }
                Event::ModelPatched { key, model_ref } => {
                    for tab in self.tabs.iter_mut().filter(|t| t.session_key == key) {
                        tab.model_ref = model_ref.clone();
                        let label = model_ref.as_deref().unwrap_or("agent default");
                        tab.push(Role::Info, format!("session model → {label}"));
                    }
                    self.persist();
                }
                Event::Error { context, message } => {
                    self.status = format!("{context}: {message}");
                    if let Some(tab) = self.tabs.get_mut(self.active) {
                        tab.push(Role::Error, format!("{context}: {message}"));
                    }
                }
                Event::Disconnected { message } => {
                    self.connected = false;
                    self.status = format!("disconnected: {message} — restart ClawTerm");
                }
            }
        }
    }

    fn tab_bar(&mut self, ui: &mut egui::Ui) {
        let mut close_tab: Option<usize> = None;
        let mut open_new = false;
        ui.horizontal(|ui| {
            ui.label(RichText::new("ClawTerm").color(ACCENT).strong());
            ui.separator();
            for (i, tab) in self.tabs.iter().enumerate() {
                let name = self
                    .agents
                    .iter()
                    .find(|a| a.id == tab.agent_id)
                    .map(|a| a.display_name().to_owned())
                    .unwrap_or_else(|| tab.agent_id.clone());
                let selected = i == self.active;
                let label = if selected {
                    RichText::new(format!("● {name}")).color(ACCENT).strong()
                } else {
                    RichText::new(name).color(AGENT_COLOR)
                };
                if ui.selectable_label(selected, label).clicked() {
                    self.active = i;
                }
                if self.tabs.len() > 1
                    && selected
                    && ui
                        .small_button(RichText::new("x").color(INFO_COLOR))
                        .clicked()
                {
                    close_tab = Some(i);
                }
            }
            if ui.button(RichText::new("+").strong()).clicked() {
                open_new = true;
            }
            ui.with_layout(Layout::right_to_left(Align::Center), |ui| {
                let dot = if self.connected { "●" } else { "○" };
                let color = if self.connected {
                    Color32::from_rgb(120, 220, 120)
                } else {
                    ERROR_COLOR
                };
                ui.label(RichText::new(dot).color(color));
                ui.label(RichText::new(&self.status).color(INFO_COLOR).small());
            });
        });
        if let Some(i) = close_tab {
            self.tabs.remove(i);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            }
            self.persist();
        }
        if open_new {
            self.open_tab(self.default_agent());
        }
    }

    fn toolbar(&mut self, ui: &mut egui::Ui) {
        let Some(tab_idx) = (self.active < self.tabs.len()).then_some(self.active) else {
            return;
        };
        let mut switch_to: Option<String> = None;
        let mut patch_model: Option<Option<String>> = None;
        {
            let tab = &self.tabs[tab_idx];
            let agent_label = self.agent_name(&tab.agent_id);
            let model_label = tab
                .model_ref
                .clone()
                .unwrap_or_else(|| "agent default".into());

            ui.horizontal(|ui| {
                ui.label(RichText::new("agent").color(INFO_COLOR).small());
                ComboBox::from_id_salt(("agent", tab.id))
                    .selected_text(RichText::new(agent_label).strong())
                    .show_ui(ui, |ui| {
                        for a in &self.agents {
                            let checked = a.id == tab.agent_id;
                            if ui.selectable_label(checked, a.display_name()).clicked() && !checked
                            {
                                switch_to = Some(a.id.clone());
                            }
                        }
                    });
                ui.separator();
                ui.label(RichText::new("model").color(INFO_COLOR).small());
                ComboBox::from_id_salt(("model", tab.id))
                    .selected_text(model_label)
                    .width(260.0)
                    .show_ui(ui, |ui| {
                        let is_default = tab.model_ref.is_none();
                        if ui.selectable_label(is_default, "agent default").clicked() && !is_default
                        {
                            patch_model = Some(None);
                        }
                        for m in &self.models {
                            let mref = m.model_ref();
                            let checked = tab.model_ref.as_deref() == Some(mref.as_str());
                            if ui.selectable_label(checked, &mref).clicked() && !checked {
                                patch_model = Some(Some(mref.clone()));
                            }
                        }
                    });
                ui.separator();
                ui.label(
                    RichText::new(&tab.session_key)
                        .color(INFO_COLOR)
                        .small()
                        .monospace(),
                );
                if tab.streaming_block.is_some() {
                    ui.separator();
                    if ui
                        .button(RichText::new("[stop]").color(ERROR_COLOR).small())
                        .clicked()
                    {
                        let _ = self.commands.send(Command::Abort {
                            key: tab.session_key.clone(),
                        });
                    }
                }
            });
        }
        if let Some(agent_id) = switch_to {
            self.switch_agent(tab_idx, agent_id);
        }
        if let Some(model_ref) = patch_model {
            let key = self.tabs[tab_idx].session_key.clone();
            let _ = self.commands.send(Command::PatchModel { key, model_ref });
        }
    }

    fn transcript(&mut self, ui: &mut egui::Ui) {
        let Some(tab) = self.tabs.get(self.active) else {
            ui.centered_and_justified(|ui| {
                ui.label("no tabs — press ＋ to open one");
            });
            return;
        };
        let agent_name = self.agent_name(&tab.agent_id);
        ScrollArea::vertical()
            .auto_shrink([false, false])
            .stick_to_bottom(true)
            .show(ui, |ui| {
                ui.add_space(6.0);
                for block in &tab.blocks {
                    let (prefix, color) = match block.role {
                        Role::User => ("you ❯".to_owned(), USER_COLOR),
                        Role::Agent => (format!("{agent_name} ❮"), ACCENT),
                        Role::Info => ("·".to_owned(), INFO_COLOR),
                        Role::Error => ("error".to_owned(), ERROR_COLOR),
                    };
                    ui.horizontal_wrapped(|ui| {
                        ui.label(RichText::new(prefix).color(color).strong().monospace());
                    });
                    let text_color = match block.role {
                        Role::User => USER_COLOR,
                        Role::Agent => AGENT_COLOR,
                        Role::Info => INFO_COLOR,
                        Role::Error => ERROR_COLOR,
                    };
                    ui.label(
                        RichText::new(&block.text)
                            .color(text_color)
                            .font(FontId::monospace(14.0)),
                    );
                    ui.add_space(8.0);
                }
                if tab.streaming_block.is_some() {
                    ui.label(RichText::new("▌").color(ACCENT));
                }
            });
    }

    fn input_line(&mut self, ui: &mut egui::Ui) {
        let Some(tab) = self.tabs.get_mut(self.active) else {
            return;
        };
        let streaming = tab.streaming_block.is_some();
        let mut submitted: Option<String> = None;
        ui.horizontal(|ui| {
            ui.label(RichText::new("❯").color(ACCENT).strong().monospace());
            let edit = TextEdit::singleline(&mut tab.input)
                .desired_width(f32::INFINITY)
                .font(FontId::monospace(14.0))
                .hint_text(if streaming {
                    "agent is responding…"
                } else if tab.session_ready {
                    "message this agent — Enter to send"
                } else {
                    "binding session…"
                });
            let response = ui.add(edit);
            if tab.focus_input {
                response.request_focus();
                tab.focus_input = false;
            }
            let submit = response.lost_focus()
                && ui.input(|i| i.key_pressed(Key::Enter))
                && !tab.input.trim().is_empty();
            if submit {
                submitted = Some(std::mem::take(&mut tab.input));
                response.request_focus();
            }
        });
        if let Some(text) = submitted {
            self.submit_input(text);
        }
    }

    /// Handle one line of input: either a client-side `:command` or a chat message.
    fn submit_input(&mut self, text: String) {
        let tab_idx = self.active;
        let trimmed = text.trim().to_owned();
        if let Some(rest) = trimmed.strip_prefix(":agent ") {
            let wanted = rest.trim().to_lowercase();
            let found = self
                .agents
                .iter()
                .find(|a| {
                    a.id.to_lowercase() == wanted || a.display_name().to_lowercase() == wanted
                })
                .map(|a| a.id.clone());
            match found {
                Some(agent_id) => self.switch_agent(tab_idx, agent_id),
                None => {
                    if let Some(tab) = self.tabs.get_mut(tab_idx) {
                        tab.push(Role::Error, format!("unknown agent: {wanted}"));
                    }
                }
            }
            return;
        }
        if let Some(rest) = trimmed.strip_prefix(":model ") {
            let wanted = rest.trim();
            let model_ref = if wanted == "default" {
                None
            } else {
                let found = self
                    .models
                    .iter()
                    .map(|m| m.model_ref())
                    .find(|r| r == wanted || r.ends_with(&format!("/{wanted}")));
                match found {
                    Some(r) => Some(r),
                    None => {
                        if let Some(tab) = self.tabs.get_mut(tab_idx) {
                            tab.push(Role::Error, format!("unknown model: {wanted}"));
                        }
                        return;
                    }
                }
            };
            if let Some(tab) = self.tabs.get(tab_idx) {
                let _ = self.commands.send(Command::PatchModel {
                    key: tab.session_key.clone(),
                    model_ref,
                });
            }
            return;
        }
        if let Some(tab) = self.tabs.get_mut(tab_idx) {
            tab.push(Role::User, trimmed.clone());
            let _ = self.commands.send(Command::Send {
                key: tab.session_key.clone(),
                text: trimmed,
            });
        }
    }

    fn handle_shortcuts(&mut self, ctx: &egui::Context) {
        let new_tab = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::T));
        let close_tab = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::W));
        let next_tab = ctx.input_mut(|i| i.consume_key(egui::Modifiers::CTRL, Key::Tab));
        if new_tab {
            self.open_tab(self.default_agent());
        }
        if close_tab && self.tabs.len() > 1 {
            self.tabs.remove(self.active);
            if self.active >= self.tabs.len() {
                self.active = self.tabs.len().saturating_sub(1);
            }
            self.persist();
        }
        if next_tab && !self.tabs.is_empty() {
            self.active = (self.active + 1) % self.tabs.len();
            if let Some(tab) = self.tabs.get_mut(self.active) {
                tab.focus_input = true;
            }
        }
    }
}

impl eframe::App for ClawTermApp {
    fn update(&mut self, ctx: &egui::Context, _frame: &mut eframe::Frame) {
        self.handle_events();
        self.handle_shortcuts(ctx);

        TopBottomPanel::top("tabs").show(ctx, |ui| {
            ui.add_space(4.0);
            self.tab_bar(ui);
            ui.add_space(2.0);
            self.toolbar(ui);
            ui.add_space(4.0);
        });

        TopBottomPanel::bottom("input").show(ctx, |ui| {
            ui.add_space(6.0);
            self.input_line(ui);
            ui.add_space(6.0);
        });

        CentralPanel::default().show(ctx, |ui| {
            self.transcript(ui);
        });

        // Keep painting while a reply is streaming so deltas appear live.
        if self.tabs.iter().any(|t| t.streaming_block.is_some()) {
            ctx.request_repaint_after(std::time::Duration::from_millis(50));
        }
    }

    fn on_exit(&mut self, _gl: Option<&eframe::glow::Context>) {
        self.persist();
    }
}
