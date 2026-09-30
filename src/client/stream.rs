use std::sync::atomic::AtomicBool;

use ratatui::{layout::{Constraint, Layout}, widgets::{Paragraph, Widget}};

use crate::{
    client::server,
    ui::build::{info_ui, main_ui, trend_ui},
};

pub enum Event<I> {
    Tick,
    Input(I),
}

#[derive(PartialEq, Clone)]
pub enum ClientState {
    Trend,
    Main,
    Info,
    Serve,
}

pub fn ui_state(
    app: &crate::App,
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
) {
    let [area, command] = Layout::vertical([Constraint::Fill(1), Constraint::Length(1)]).areas(area);
    match app.state {
        ClientState::Trend => trend_ui(app, area, buf),
        ClientState::Main => main_ui(app, area, buf),
        ClientState::Info => info_ui(app, area, buf),
        ClientState::Serve => server::main_ui(app, area, buf),
    }
    if let Some(text) = app.command_buf.as_ref() {
        Paragraph::new(text.as_str()).render(command, buf);
    }
}

static SERVE_READY: AtomicBool = AtomicBool::new(false);

pub fn reset_state(state: &mut ClientState) {
    match *state {
        ClientState::Main => {
            *state = ClientState::Trend;
        }
        ClientState::Trend => {
            *state = ClientState::Info;
        }
        ClientState::Info => {
            if SERVE_READY.load(std::sync::atomic::Ordering::Relaxed) {
                *state = ClientState::Serve;
            } else {
                *state = ClientState::Main;
            }
        }
        ClientState::Serve => {
            *state = ClientState::Main;
        }
    }
}
