use std::{collections::HashMap, fs, io, rc::Rc, time::Duration};

use crossterm::event::{
    self,
    KeyCode::{self, Backspace, Enter},
    KeyEvent, KeyEventKind, KeyModifiers,
};
use ratatui::{
    style::Style,
    symbols::{self, line::DOUBLE_VERTICAL, merge::MergeStrategy},
    text::Line,
    widgets::{LineGauge, Paragraph, Wrap},
};
use sysinfo::{Motherboard, Networks, Pid, Process, ProcessRefreshKind, ProcessesToUpdate, System};

use crate::{client::stream::ClientState, sys::get_gpu};
use crate::{
    client::{
        server::{self, Serve},
        system::{Config, FmtTime, SystemLine, from_osstring},
    },
    edid::{DisplayTag, find_edid},
    sys::{Device::Network, Disk, take_sys_device, take_sys_disk, take_sys_hdaudio},
    ui::build::{Tui, normal_block, set_alert},
};

pub mod client;
pub mod edid;
pub mod sys;
pub mod ui;

const APP_VERSION: &str = "1.2.5";

unsafe impl Send for App {}

pub struct App {
    pub state: ClientState,
    pub exit: bool,
    pub sys: System,
    pub extend: Extend,
    pub sys_line: SystemLine,
    pub err: Option<anyhow::Error>,
    pub config: Config,
    pub service: Serve,
    pub network: Networks,
    pub formats: Format,
    pub disks: Vec<Disk>,
    pub hdaudio: String,
    pub command_buf: Option<String>,
    pub displays: String,
    pub devices: String,
}

impl App {
    pub fn new() -> Result<App, anyhow::Error> {
        let screens = find_edid().unwrap_or_default();
        let displays = if !screens.is_empty() {
            let mut result = String::new();
            for (i, screen) in screens.iter().enumerate() {
                let mut productname = None;
                let mut refresh_rate = String::new();
                for descriptor in &screen.display_descriptors {
                    match descriptor {
                        DisplayTag::ProductName(name) => {
                            productname = Some(name.clone());
                        }
                        DisplayTag::DisplayRangeLimit((_, max)) => {
                            let reso = screen.resolution.unwrap_or((0, 0));
                            refresh_rate = format!("{}x{}@{}hz", reso.0, reso.1, max);
                        }
                        _ => {}
                    };
                }
                result.push_str(&format!(
                    "display{i}: {} {} {}\n",
                    if let Some(name) = productname {
                        format!(
                            "{name}({}{})",
                            screen.manufacture_pnp,
                            screen
                                .screen_size
                                .map(|f| format!(" {f}in"))
                                .unwrap_or_default()
                        )
                    } else {
                        format!(
                            "({}{})",
                            screen.manufacture_pnp,
                            screen
                                .screen_size
                                .map(|f| format!(" {f}in"))
                                .unwrap_or_default()
                        )
                    },
                    refresh_rate,
                    if let Some(color) = &screen.video_input.as_ref().and_then(|f| f.color_depth) {
                        format!("{color}bit")
                    } else {
                        String::new()
                    }
                ));
            }
            result
        } else {
            String::new()
        };
        let devices = match take_sys_device() {
            Ok(v) => v,
            Err(e) => {
                println!("scan sys device error: {e}");
                vec![]
            }
        };
        Ok(App {
            state: ClientState::Main,
            exit: false,
            sys: System::new_all(),
            sys_line: SystemLine::new(),
            err: None,
            extend: Extend::new()?,
            disks: take_sys_disk()?,
            hdaudio: take_sys_hdaudio().unwrap_or_default().iter().enumerate().fold(
                String::new(),
                |mut acc, (id, f)| {
                    acc.push_str(&format!(
                        "audio{}: {} {}\n",
                        id,
                        f.vendor_name.as_deref().unwrap_or_default(),
                        f.chip_name.as_deref().unwrap_or_default()
                    ));
                    acc
                },
            ),
            config: Config::new(),
            service: Serve::new(vec![]),
            network: Networks::new_with_refreshed_list(),
            formats: Format::default(),
            command_buf: None,
            displays,
            devices: devices
                .iter()
                .enumerate()
                .fold(String::new(), |mut acc, (id, f)| {
                    match f {
                        Network((vendor, device)) => {
                            acc.push_str(&format!("network{id}: {vendor} {device}\n"));
                        }
                    }
                    acc
                }),
        }
        .once())
    }

    fn once(mut self) -> Self {
        let gpu = get_gpu().unwrap_or_default();
        self.formats.tab = Rc::new(
            Paragraph::new(format!(
                "Welcome to SeVA {APP_VERSION}!   Press 'Q' to quit SEVA"
            ))
            .alignment(ratatui::layout::Alignment::Center)
            .block(normal_block("SEVA Control")),
        );

        let f = format!(
            "os: {}-{}\ncpu name: {}{}\nkernel version: {}\nhost name: {}\nup time: {}\n{}\n{}",
            System::name().unwrap_or_default(),
            System::os_version().unwrap_or(String::from("Unknown os version")),
            self.sys.cpus()[0].brand(),
            Motherboard::new()
                .and_then(|x| x.name().filter(|f| f != "INVALID"))
                .map(|f| "\nMotherboard: ".to_string() + &f)
                .unwrap_or("".to_string()),
            System::kernel_version().unwrap_or_default(),
            System::host_name().unwrap_or(String::from("linux")),
            FmtTime(System::uptime()),
            self.extend.package_text,
            gpu.iter()
                .enumerate()
                .fold(String::new(), |mut acc, (i, dev)| {
                    if let Some(name) = &dev.device_name {
                        acc += &format!("gpu {}: {}\n", i, name);
                    }
                    acc
                })
        );
        self.formats.os_message_len = f.lines().collect::<Vec<&str>>().len();

        self.formats.os_message_format = Rc::new(
            Paragraph::new(f)
                .wrap(Wrap { trim: true })
                .block(normal_block("os")),
        );

        // let cache;
        // if let Ok(c) = command_runs(&[&["lscpu"], &["grep", "^L"]]) {
        //     cache = c.replace(" ", "");
        // } else {
        //     cache = String::from("无法获取缓存信息");
        // }
        // self.formats.cache_len = cache.lines().collect::<Vec<&str>>().len();

        // self.formats.cache_message = Rc::new(
        //     Paragraph::new(cache)
        //         .wrap(Wrap { trim: true })
        //         .block(normal_block("cache")),
        // );

        self.flash().unwrap();

        self
    }

    pub fn destory(self) -> io::Result<()> {
        Ok(())
    }

    pub fn flash(&mut self) -> anyhow::Result<()> {
        if self.extend.space {
            return Ok(());
        }

        match self.state {
            ClientState::Main => {
                self.sys.refresh_cpu_usage();
                self.sys.refresh_memory();
                self.network.refresh(true);
                self.merge_process();
            }
            ClientState::Trend => {
                self.sys.refresh_cpu_all();
                self.sys.refresh_memory();
                self.merge_process();
            }
            ClientState::Info => {}
            ClientState::Serve => todo!(),
        }

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
            ProcessRefreshKind::nothing()
                .with_cpu()
                .with_memory()
                .with_exe(sysinfo::UpdateKind::OnlyIfNotSet)
                .without_tasks(),
        );
        let mut memory_verify: HashMap<String, MutProcess> = HashMap::new();
        for item in self.sys.processes().values() {
            if item.thread_kind().is_some() {
                continue;
            }
            let item = MutProcess::from_process(item);
            if let Some(filter) = self.extend.process_filter.as_ref()
                && !item.cmd.contains(filter)
            {
                continue;
            }
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
                .sort_by_key(|v| std::cmp::Reverse(v.memory));
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

        let swap_ratio = self.sys.used_swap() as f64 / self.sys.total_swap() as f64;

        self.formats.swap_line = Rc::new(
            LineGauge::default()
                .block(normal_block("swap").merge_borders(MergeStrategy::Exact))
                .filled_style(blue_style)
                .filled_symbol(symbols::line::DOUBLE_VERTICAL)
                .unfilled_symbol(symbols::line::DOUBLE_VERTICAL)
                .label(Line::default())
                .ratio(if !swap_ratio.is_nan() { swap_ratio } else { 0.0 }),
        );

        // self.formats.disk_text = self.extend.disks.iter().fold(String::new(), |acc, disk| {
        //     let total_space = disk.total_space();
        //     if total_space < 8 * 1024 * 1024 * 1024 {
        //         return acc;
        //     }
        //     acc + &format!(
        //         "Disk Name: {:?}\n   file system: {:?}\n   used/total: {}/ {}\n   write/read: {}/ {}\n\n",
        //         disk.name(),
        //         disk.file_system(),
        //         HumanBytes(total_space - disk.available_space()),
        //         HumanBytes(total_space),
        //         HumanBytes(disk.usage().written_bytes),
        //         HumanBytes(disk.usage().read_bytes)
        //     )
        // });
    }

    pub fn run(&mut self, mut terminal: Tui) -> anyhow::Result<()> {
        loop {
            self.flash()?;

            if self.exit {
                break;
            }
            terminal.draw(|frame| {
                let area = frame.area();
                let buf = frame.buffer_mut();
                crate::client::stream::ui_state(self, area, buf);
                if let Some(err) = &self.err {
                    set_alert(area, buf, &err.to_string());
                }
            })?;
            if event::poll(Duration::from_secs(1))?
                && let event::Event::Key(key) = event::read()?
            {
                handle_key(self, key)?;
            }
        }
        ratatui::restore();
        terminal.show_cursor()?;

        Ok(())
    }
}

#[derive(Default)]
pub struct Format {
    os_message_format: Rc<Paragraph<'static>>,
    os_message_len: usize,
    tab: Rc<Paragraph<'static>>,
    cpu_line: Rc<LineGauge<'static>>,
    mem_line: Rc<LineGauge<'static>>,
    swap_line: Rc<LineGauge<'static>>,
    _disk_text: String,
}

#[derive(Default)]
pub struct Extend {
    pub package_text: String,
    pub trend_sort: String,
    pub space: bool,
    //pub disks: Disks,
    pub processes: Vec<MutProcess>,
    pub process_filter: Option<String>,
}

impl Extend {
    pub fn new() -> anyhow::Result<Extend> {
        let mut result = Extend {
            package_text: String::from("packages: "),
            ..Default::default()
        };

        if let Ok(str) = fs::read_to_string("/var/lib/dpkg/status") {
            let n = str
                .lines()
                .filter(|p| p.trim() == "Status: install ok installed")
                .collect::<Vec<_>>()
                .len();
            result.package_text += &format!("{} (dpkg), ", n);
        }

        // if let Ok(n) = command_runs(&[&["dpkg", "-l"], &["grep", "ii"], &["wc", "-l"]]) {
        //     result.package_text += &format!("{} (dpkg), ", n.trim());
        // }

        if let Ok(dir) = fs::read_dir("/var/lib/pacman/local/") {
            let n = dir.collect::<Vec<_>>().len();
            result.package_text += &format!("{} (pacman), ", n - 1);
        }
        result.package_text = result.package_text.trim_end_matches(", ").to_string();
        //result.disks = Disks::new_with_refreshed_list();
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
        if let Some(text) = main.command_buf.as_mut() {
            if let Some(char) = key.code.as_char() {
                text.push(char);
            } else if key.code == Backspace {
                text.pop();
            } else if key.code == Enter {
                if text.is_empty() {
                    main.command_buf = None;
                    return Ok(());
                }
                if &text.as_bytes()[1..] == b"unset" {
                    main.extend.process_filter = None;
                } else {
                    main.extend.process_filter = Some(text[1..].to_string());
                }
                main.command_buf = None;
            }
            return Ok(());
        }
        if key.code == KeyCode::Char('/') {
            main.command_buf = Some(String::from("/"));
            return Ok(());
        }
        if key.code == KeyCode::Tab {
            crate::client::stream::reset_state(&mut main.state);
        } else if key.code == KeyCode::Enter {
            //main.flash()?;
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
        if key.code == KeyCode::Char('q')
            || (key.code == KeyCode::Char('c') && key.modifiers.contains(KeyModifiers::CONTROL))
        {
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
