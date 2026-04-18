use std::{collections::HashMap, io, rc::Rc, time::Duration};

use crossterm::event::{self, EventStream};
use futures::{FutureExt, StreamExt};
use ratatui::{
    Frame,
    style::Style,
    symbols::{self, line::DOUBLE_VERTICAL, merge::MergeStrategy},
    text::Line,
    widgets::{LineGauge, Paragraph, Widget},
};
use sysinfo::{Motherboard, Networks, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::{client::{
    client::{ClientState, MutProcess, handle_key},
    server::Serve,
    system::{Config, SystemLine, byte_to_string, sec_to_time},
}, ui::build::{Tui, normal_block, set_alert}};
use crate::sys::get_gpu;

pub mod client;
pub mod ui;
pub mod sys;
// pub mod control;
// pub mod network;
// pub mod server;

unsafe impl Send for App {}
pub struct App {
    pub state: crate::client::client::ClientState,
    pub exit: bool,
    pub sys: System,
    pub extend: crate::client::client::Extend,
    pub sys_line: SystemLine,
    // pub sftp: FtpStruct,
    pub err: Option<anyhow::Error>,
    pub config: Config,
    pub service: Serve,
    pub network: Networks,
    pub formats: Format,
}

impl App {
    pub fn new() -> Result<App, anyhow::Error> {
        let config = Config::new();
        Ok(App {
            state: ClientState::Main,
            exit: false,
            sys: System::new_all(),
            sys_line: SystemLine::new(),
            // sftp: FtpStruct::new(),
            err: None,
            extend: crate::client::client::Extend::new()?,
            config: config.clone(),
            service: Serve::new(config.services),
            network: Networks::new_with_refreshed_list(),
            formats: Format::new(),
        }
        .once())
    }

    fn once(mut self) -> Self {
        let gpu = get_gpu().unwrap_or_default();
        self.formats.tab = Rc::new(
            Paragraph::new("Welcome to SeVA!   Press 'Q' to quit SEVA")
                .alignment(ratatui::layout::Alignment::Center)
                .block(normal_block("SEVA Control")),
        );

        self.formats.os_message_format = format!(
            "os name: {}\nos version: {}\ncpu name: {}\ncpu arch: {}\nMotherboard: {}\nkernel version: {}\nhost name: {}\nrunning time: {}\n{}\n",
            System::name().unwrap_or_default(),
            System::os_version().unwrap_or(String::from("Unknown os version")),
            self.sys.cpus()[0].brand(),
            System::cpu_arch(),
            Motherboard::new()
                .map(|x| x.name().unwrap_or(String::new()))
                .unwrap_or("".to_string()),
            System::kernel_version().unwrap_or_default(),
            System::host_name().unwrap_or(String::from("linux")),
            sec_to_time(System::uptime()),
            self.extend.package_text,
        );

        for (i, dev) in gpu.iter().enumerate() {
            if let Some(name) = &dev.device_name {
                self.formats.os_message_format += &format!("gpu {}: {}\n", i, name);
            }
        }

        self.flash().unwrap();

        self
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
        self.sys_line.swap_data.force_queue(
            format!(
                "{:.2}",
                (self.sys.used_swap() as f64 / self.sys.total_swap() as f64) * 100.0
            )
            .parse::<f64>()?,
        );
        self.sys_line
            .cpu_data
            .force_queue(self.sys.global_cpu_usage().into());
        let us_memory = self.sys.used_memory() as f64;
        let to_memory = self.sys.total_memory() as f64;
        self.sys_line
            .memory_data
            .force_queue(format!("{:.2}", (us_memory / to_memory) * 100.0).parse::<f64>()?);
        self.merge_ui();
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

    pub fn merge_ui(&mut self) {
        let blue_style = Style::new().blue().on_black().bold();
        self.formats.cpu_line = Rc::new(
            LineGauge::default()
                .block(normal_block("cpu").merge_borders(MergeStrategy::Exact))
                .filled_style(blue_style)
                .filled_symbol(DOUBLE_VERTICAL)
                .unfilled_symbol(symbols::line::DOUBLE_VERTICAL)
                .label(Line::default())
                .ratio(self.sys.global_cpu_usage() as f64 / 100.0),
        );

        self.formats.mem_line = Rc::new(
            LineGauge::default()
                .block(normal_block("memory").merge_borders(MergeStrategy::Exact))
                .filled_style(blue_style)
                .filled_symbol(symbols::line::DOUBLE_VERTICAL)
                .unfilled_symbol(symbols::line::DOUBLE_VERTICAL)
                .label(Line::default())
                .ratio(self.sys.used_memory() as f64 / self.sys.total_memory() as f64),
        );

        self.formats.swap_line = Rc::new(
            LineGauge::default()
                .block(normal_block("swap").merge_borders(MergeStrategy::Exact))
                .filled_style(blue_style)
                .filled_symbol(symbols::line::DOUBLE_VERTICAL)
                .unfilled_symbol(symbols::line::DOUBLE_VERTICAL)
                .label(Line::default())
                .ratio(self.sys.used_swap() as f64 / self.sys.total_swap() as f64),
        );

        self.formats.disk_text = self.extend.disks.iter().fold(String::new(), |acc, disk| {
            let total_space = disk.total_space();
            if total_space < 8 * 1024 * 1024 * 1024 {
                return acc;
            }
            acc + &format!(
                "Disk Name: {:?}\n   file system: {:?}\n   used/total: {}/ {}\n   write/read: {}/ {}\n\n",
                disk.name(),
                disk.file_system(),
                byte_to_string(total_space - disk.available_space()),
                byte_to_string(total_space),
                byte_to_string(disk.usage().written_bytes),
                byte_to_string(disk.usage().read_bytes)
            )
        });
    }

    pub async fn run(&mut self, mut terminal: Tui) -> anyhow::Result<()> {
        let mut tick = tokio::time::interval(Duration::from_millis(1000));
        let mut reader = EventStream::new();

        loop {
            let next = reader.next().fuse();
            tokio::select! {
                event = next => {
                    if let Some(Ok(event::Event::Key(key))) = event {
                        handle_key(self, key)?;
                    }
                }
                _ = tick.tick() => {
                    self.flash()?;
                }
                // _ = fan.tick() => {

                // }
            };
            if self.exit {
                break;
            }
            // let elapsed = last_frame.elapsed();
            // last_frame = Instant::now();

            terminal.draw(|frame| {
                self.handle_ui(frame);

                //let screen_area = frame.area();
                //effects.process_effects(elapsed.into(), frame.buffer_mut(), screen_area);
            })?;
        }
        ratatui::restore();
        terminal.show_cursor()?;

        Ok(())
    }
}

impl Widget for &App {
    fn render(self, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer)
    where
        Self: Sized,
    {
        crate::client::stream::ui_state(self, area, buf);
        if let Some(err) = &self.err {
            set_alert(area, buf, &err.to_string());
        }
    }
}

#[derive(Default)]
pub struct Format {
    os_message_format: String,
    tab: Rc<Paragraph<'static>>,
    cpu_line: Rc<LineGauge<'static>>,
    mem_line: Rc<LineGauge<'static>>,
    swap_line: Rc<LineGauge<'static>>,
    disk_text: String,
}

impl Format {
    pub fn new() -> Format {
        Format {
            ..Default::default()
        }
    }
}

// pub async fn run(mut main: App, mut tui: Tui) -> anyhow::Result<()> {

//     //let mut fan = tokio::time::interval(Duration::from_millis(16));
//     // let mut effects: EffectManager<()> = EffectManager::default();
//     // let timer = (1000, Interpolation::Linear);
//     // let fg_shift = [120.0, 25.0, 25.0];
//     // let bg_shift = [-40.0, -50.0, -50.0];

//     // let fade_effect = fx::hsl_shift(Some(fg_shift), Some(bg_shift), timer)
//     //     .with_pattern(DiagonalPattern::bottom_left_to_top_right());
//     // let fx = fx::freeze_at(0.5, false, fade_effect);

//     // effects.add_effect(fx);

//     // let mut last_frame = Instant::now();

// }
