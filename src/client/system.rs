use std::{
    ffi::{OsStr, OsString},
    process::{Command, Stdio},
};

use queue::Queue;

pub struct SystemLine {
    pub cpu_data: Queue<f64>,
    pub memory_data: Queue<f64>,
    pub network_data: Queue<f64>,
    pub swap_data: Queue<f64>,
}

#[derive(Clone)]
pub struct Config {
    pub services: Vec<String>,
}

impl Config {
    pub fn new() -> Config {
        Config {
            services: Vec::new(),
        }
    }
}

impl Default for Config {
    fn default() -> Self {
        Self::new()
    }
}

impl SystemLine {
    pub fn new() -> SystemLine {
        SystemLine {
            cpu_data: queue::Queue::with_capacity(20),
            memory_data: queue::Queue::with_capacity(20),
            network_data: queue::Queue::with_capacity(20),
            swap_data: queue::Queue::with_capacity(20),
        }
    }
}

impl Default for SystemLine {
    fn default() -> Self {
        Self::new()
    }
}

pub struct HumanBytes(pub u64);

impl std::fmt::Display for HumanBytes {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        const UNITS: [&str; 7] = ["B", "KiB", "MiB", "GiB", "TiB", "PiB", "EiB"];
        let bytes = self.0 as f32;
        let i = ((bytes.log2() / 10.0) as usize).min(UNITS.len() - 1);
        let unit = UNITS[i];
        let size = bytes / 1024_f32.powi(i as i32);

        // Don't show a fractional number of bytes.
        if i == 0 {
            return write!(f, "{size}{unit}");
        }

        f.pad(&format!("{size:.2}{unit}"))
    }
}

pub fn sec_to_time(mut sec: u64) -> String {
    let mut ider: u64 = 0;
    let times = ["sec", "min", "hour", "day"];
    let mut timelevel = 0;
    loop {
        if timelevel < 2 && sec >= 60 {
            ider = sec % 60;
            sec /= 60;

            timelevel += 1;
        } else if timelevel == 2 && sec >= 24 {
            ider = sec % 24;
            sec /= 24;
            timelevel += 1;
            break;
        } else {
            break;
        }
    }
    if timelevel == 0 {
        return format!("{}{}", sec, times[timelevel]);
    }
    format!(
        "{}{}{}{}",
        sec,
        times[timelevel],
        ider,
        times[timelevel - 1]
    )
}

pub fn from_osstring(cmd: &[OsString]) -> String {
    cmd.join(OsStr::new(""))
        .to_string_lossy()
        .trim()
        .to_string()
}

pub fn command_runs(cmds: &[&[&str]]) -> anyhow::Result<String> {
    let mut child: Option<std::process::Child> = None;
    if cmds.len() > 1 {
        for args in &cmds[..cmds.len() - 1] {
            let len = args.len();
            if len == 0 {
                continue;
            }
            let mut cmd = Command::new(args[0]);
            if len > 1 {
                cmd.args(&args[1..]);
            }
            if let Some(before) = child {
                cmd.stdin(Stdio::from(before.stdout.unwrap()));
            }
            child = Some(cmd.stdout(Stdio::piped()).spawn()?);
        }
    }
    let last = cmds[cmds.len() - 1];
    let mut cmd = Command::new(last[0]);
    cmd.args(&last[1..]);
    if let Some(before) = child {
        cmd.stdin(before.stdout.unwrap());
    }

    Ok(String::from_utf8(
        cmd.stdout(Stdio::piped()).output()?.stdout,
    )?)
}
