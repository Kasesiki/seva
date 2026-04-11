
use crossterm::event::{KeyCode, KeyEvent, KeyEventKind};
use sysinfo::{Disks, Pid, Process};

use crate::App;
use crate::client::system::{from_osstring};
use crate::client::system::command_runs;
use super::{
    server::{self},
    // sftp::{self, FtpStruct},
    ui::{ClientState},
};

pub static MENU_TITLES: [&str; 3] = ["Home", "Control", "Terminal"];

#[derive(Default)]
pub struct Extend {
    pub package_text: String,
    pub trend_sort: String,
    pub space: bool,
    pub disks: Disks,
    pub processes: Vec<MutProcess>,
}

impl Extend {
    pub fn new() -> anyhow::Result<Extend> {
        let mut result = Extend {
            package_text: String::from("packages: "),
            ..Default::default()
        };
        if let Ok(n) = command_runs(&[&["dpkg", "-l"], &["grep", "ii"], &["wc", "-l"]]) {
            result.package_text += &format!("{} (dpkg), ", n.trim());
        }
        if let Ok(n) = command_runs(&[&["pacman", "-Q"], &["wc", "-l"]]) {
            result.package_text += &format!("{} (pacman), ", n.trim());
        }
        result.package_text = result.package_text.trim_end_matches(", ").to_string();
        result.disks = Disks::new_with_refreshed_list();
        Ok(result)
    }
}


pub fn handle_key(main: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    if key.kind == KeyEventKind::Press {
        if let Some(_) = &main.err
            && key.code == KeyCode::Enter
        {
            main.err = None;
        }
        if key.code == KeyCode::Tab {
            crate::client::stream::reset_state(&mut main.state);
        }

        let state = &main.state;
        if *state == ClientState::Serve {
            server::main_event(main, key)?;
        } else if *state == ClientState::Trend {
            if key.code == KeyCode::Char('m') {
                main.extend.trend_sort = String::from("mem")
            } else if key.code == KeyCode::Char(' ') {
                main.extend.space = !main.extend.space;
            } else if key.code == KeyCode::Char('c') {
                main.extend.trend_sort = String::from("")
            }
        }
        if key.code == KeyCode::Char('q') {
            main.exit = true;
        }
    }
    Ok(())
}

pub struct MutProcess {
    pub pid: Pid,
    pub cmd: String,
    pub cpu_usage: f32,
    pub memory: u64,
    pub virtual_memory: u64,
    pub run_time: u64,
    pub name: String,
}

impl MutProcess {
    pub fn from_process(process: &Process) -> MutProcess {
        let cmd;
        if let Some(pat) = process.exe() {
            cmd = pat.to_string_lossy().to_string();
        } else {
            cmd = from_osstring(process.cmd());
        }

        MutProcess {
            pid: process.pid(),
            cmd,
            cpu_usage: process.cpu_usage(),
            memory: process.memory(),
            virtual_memory: process.virtual_memory(),
            run_time: process.run_time(),
            name: process.name().to_string_lossy().to_string(),
        }
    }
}
