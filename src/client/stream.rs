use crate::client::{
    server,
    ui::{ClientState, main_ui, trend_ui},
};

pub enum Event<I> {
    Tick,
    Input(I),
}

pub fn main_ui_draw(
    app: &crate::App,
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
) {
    match app.state {
        ClientState::Trend => trend_ui(app, area, buf),
        ClientState::Main => main_ui(app, area, buf),
        // ClientState::Sftp => sftp::main_ui(app, area, buf),
        ClientState::Serve => server::main_ui(app, area, buf),
    }
}

pub fn reset_state(state: &mut ClientState) {
    match *state {
        ClientState::Main => {
            *state = ClientState::Trend;
        }
        ClientState::Trend => {
            *state = ClientState::Serve;
        }
        // ClientState::Sftp => {
        //     *state = ClientState::Serve;
        // }
        ClientState::Serve => {
            *state = ClientState::Main;
        }
    }
}
