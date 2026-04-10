use std::process::{Command, Stdio};

pub mod client;
// pub mod control;
// pub mod network;
// pub mod server;

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
