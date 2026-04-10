use std::{
    collections::HashMap,
    ffi::{OsStr, OsString},
    io::{self},
    time::Duration,
};

use crossterm::event::{self, EventStream, KeyCode, KeyEvent, KeyEventKind};
use futures::{FutureExt, StreamExt};
use ratatui::{Frame, widgets::Widget};
use sysinfo::{Disks, Networks, Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::client::system::{Config, SystemLine};

use super::{
    art,
    server::{self, Serve},
    // sftp::{self, FtpStruct},
    ui::{self, ClientState, Tui},
};

pub static MENU_TITLES: [&str; 3] = ["Home", "Control", "Terminal"];

pub async fn main() -> anyhow::Result<()> {
    art::init_art();

    let app = App::new()?;
    let terminal: ui::Tui = ratatui::init();
    run(app, terminal).await?;
    Ok(())
}

async fn run(mut main: App, mut tui: Tui) -> anyhow::Result<()> {
    let mut tick = tokio::time::interval(Duration::from_millis(1000));
    //let mut fan = tokio::time::interval(Duration::from_millis(16));

    let mut reader = EventStream::new();

    // let mut effects: EffectManager<()> = EffectManager::default();
    // let timer = (1000, Interpolation::Linear);
    // let fg_shift = [120.0, 25.0, 25.0];
    // let bg_shift = [-40.0, -50.0, -50.0];

    // let fade_effect = fx::hsl_shift(Some(fg_shift), Some(bg_shift), timer)
    //     .with_pattern(DiagonalPattern::bottom_left_to_top_right());
    // let fx = fx::freeze_at(0.5, false, fade_effect);

    // effects.add_effect(fx);

    // let mut last_frame = Instant::now();
    loop {
        let next = reader.next().fuse();
        tokio::select! {
            event = next => {
                if let Some(Ok(event::Event::Key(key))) = event {
                    handle_key(&mut main, key)?;
                }
            }
            _ = tick.tick() => {
                main.flash()?;
            }
            // _ = fan.tick() => {

            // }
        };
        if main.exit {
            break;
        }
        // let elapsed = last_frame.elapsed();
        // last_frame = Instant::now();
        tui.draw(|frame| {
            main.handle_ui(frame);

            //let screen_area = frame.area();
            //effects.process_effects(elapsed.into(), frame.buffer_mut(), screen_area);
        })?;
    }
    ratatui::restore();
    tui.show_cursor()?;

    Ok(())
}

unsafe impl Send for App {}
pub struct App {
    pub state: ui::ClientState,
    pub exit: bool,
    pub sys: System,
    pub extend: Extend,
    pub sys_line: SystemLine,
    // pub sftp: FtpStruct,
    pub err: Option<anyhow::Error>,
    pub config: Config,
    pub service: Serve,
    pub network: Networks,
}

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
        let mut result = Extend::default();
        result.package_text = String::from("packages: ");
        if let Ok(n) = crate::command_runs(&[&["dpkg", "-l"], &["grep", "ii"], &["wc", "-l"]]) {
            result.package_text += &format!("{} (dpkg), ", n.trim());
        }
        if let Ok(n) = crate::command_runs(&[&["pacman", "-Q"], &["wc", "-l"]]) {
            result.package_text += &format!("{} (pacman), ", n.trim());
        }
        result.package_text = result.package_text.trim_end_matches(", ").to_string();
        result.disks = Disks::new_with_refreshed_list();
        Ok(result)
    }
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

pub fn from_osstring(cmd: &[OsString]) -> String {
    cmd.join(OsStr::new(""))
        .to_string_lossy()
        .trim()
        .to_string()
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

impl App {
    pub fn new() -> Result<App, anyhow::Error> {
        let config = Config::new();
        Ok(App {
            state: ui::ClientState::Main,
            exit: false,
            sys: System::new_all(),
            sys_line: SystemLine::new(),
            // sftp: FtpStruct::new(),
            err: None,
            extend: Extend::new()?,
            config: config.clone(),
            service: Serve::new(config.services),
            network: Networks::new_with_refreshed_list(),
        })
    }

    pub fn handle_ui(&self, frame: &mut Frame) {
        frame.render_widget(self, frame.area());
    }

    pub fn destory(self) -> io::Result<()> {
        Ok(())
    }

    pub fn flash(&mut self) -> anyhow::Result<()> {
        if self.extend.space {
            return Ok(());
        }
        self.extend.disks.iter_mut().for_each(|disk| {
            disk.refresh();
        });
        self.network = Networks::new_with_refreshed_list();
        self.sys.refresh_cpu_usage();
        self.sys.refresh_memory();
        self.merge_process();
        let cpu_us: f64 = self.sys.global_cpu_usage().into();
        let swap_us: f64 = self.sys.used_swap() as f64;
        let swap_total: f64 = self.sys.total_swap() as f64;
        self.sys_line
            .swap_data
            .force_queue(format!("{:.2}", (swap_us / swap_total) * 100.0).parse::<f64>()?);
        self.sys_line.cpu_data.force_queue(cpu_us);
        let us_memory = self.sys.used_memory() as f64;
        let to_memory = self.sys.total_memory() as f64;
        self.sys_line
            .memory_data
            .force_queue(format!("{:.2}", (us_memory / to_memory) * 100.0).parse::<f64>()?);
        Ok(())
    }

    pub fn merge_process(&mut self) {
        self.sys.refresh_processes_specifics(
            ProcessesToUpdate::All,
            true,
            ProcessRefreshKind::everything(),
        );
        let mut memory_verify: HashMap<String, MutProcess> = HashMap::new();
        for item in self.sys.processes().values() {
            if item.thread_kind().is_some() {
                continue;
            }
            let item = MutProcess::from_process(item);
            let mem = item.memory;
            let cpu_usage = item.cpu_usage;

            if let Some(o2) = memory_verify.get_mut(&item.cmd) {
                o2.cpu_usage += cpu_usage;
                o2.memory += mem;
                continue;
            } else {
                memory_verify.insert(item.cmd.clone(), item);
            }
        }

        self.extend.processes = memory_verify.into_values().collect();

        if self.extend.trend_sort == "mem" {
            self.extend
                .processes
                .sort_by(|k, v| v.memory.cmp(&k.memory));
        } else {
            self.extend
                .processes
                .sort_by(|k, v| v.cpu_usage.total_cmp(&k.cpu_usage));
        }
    }
}

fn handle_key(main: &mut App, key: KeyEvent) -> anyhow::Result<()> {
    if key.kind == KeyEventKind::Press {
        if let Some(_) = &main.err
            && key.code == KeyCode::Enter
        {
            main.err = None;
        }
        if key.code == KeyCode::Tab {
            rset_state(&mut main.state);
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

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        ui::main_ui_draw(self, area, buf);
        if let Some(err) = &self.err {
            ui::set_alert(area, buf, &err.to_string());
        }
    }
}

pub fn rset_state(state: &mut ClientState) {
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
