use std::io;

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout},
    style::Style,
    widgets::{List, Paragraph, Tabs, Widget},
};

use super::ui::{ClientState, normal_block};

pub fn main_ui(app: &crate::App, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
    let [main, help] = Layout::vertical([Constraint::Fill(6), Constraint::Fill(1)]).areas(area);
    let [lis, details] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(3)]).areas(main);
    List::new(app.service.services.clone())
        .block(normal_block("servers"))
        .render(lis, buf);
    Paragraph::new("")
        .block(normal_block("details"))
        .render(details, buf);
    Tabs::new(vec!["N new", "D del"])
        .highlight_style(Style::default())
        .style(Style::default())
        .block(normal_block(""))
        .render(help, buf);
}

pub fn main_event(app: &mut crate::App, key: KeyEvent) -> Result<(), io::Error> {
    match key.code {
        KeyCode::Char('n') => {
            if app.service.statu != ServeStatu::New {
                app.service.statu = ServeStatu::New;
            }
        }
        KeyCode::Char('q') => {
            if app.service.statu == ServeStatu::Normal {
                app.state = ClientState::Main;
            } else {
                app.service.statu = ServeStatu::Normal;
            }
        }
        _ => {}
    }
    Ok(())
}

pub struct Serve {
    _cursor: usize,
    statu: ServeStatu,
    services: Vec<String>,
}

impl Serve {
    pub fn new(services: Vec<String>) -> Serve {
        Serve {
            _cursor: 0,
            statu: ServeStatu::Normal,
            services,
        }
    }
}

#[derive(PartialEq)]
pub enum ServeStatu {
    Normal,
    Set,
    New,
}
