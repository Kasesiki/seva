use std::{
    collections::VecDeque,
    fs::{self, File},
    io::{self, Error, Read, Write},
    net::TcpStream,
    path::{Path, PathBuf},
    sync::{Arc, Mutex},
};

use crossterm::event::{KeyCode, KeyEvent};
use ratatui::{
    layout::{Constraint, Layout},
    style::{Color, Style},
    text::Text,
    widgets::{Clear, List, Paragraph, Tabs, Widget},
};
use ssh2::{OpenFlags, Session};
use sysinfo::System;

use super::{
    client::{self, App},
    ui::{self, create_pop, normal_block, Alert, ClientState},
};

pub fn main_ui(
    app: &client::App,
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
) {
    let [main, log, help] = Layout::vertical([
        Constraint::Fill(2),
        Constraint::Fill(1),
        Constraint::Length(1),
    ])
    .areas(area);
    let [left_ui, right_ui] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(main);
    let left = &app.sftp.left;
    let right = &app.sftp.right;

    let mut left_style_vec = Vec::new();
    for (i, ve) in left.path_vec.iter().enumerate() {
        if i == left.cursor && app.sftp.cursor == Use::Left {
            left_style_vec.push(Text::styled(ve, Style::default().bg(Color::Yellow)));
        } else {
            left_style_vec.push(Text::styled(ve, Style::default()));
        }
    }

    let mut right_style_vec = Vec::new();
    for (i, ve) in right.path_vec.iter().enumerate() {
        if i == right.cursor && app.sftp.cursor == Use::Right {
            right_style_vec.push(Text::styled(ve, Style::default().bg(Color::Yellow)));
        } else {
            right_style_vec.push(Text::styled(ve, Style::default()));
        }
    }
    if left.cursor > 8 {
        let seek = left.cursor - 8;
        left_style_vec = left_style_vec[seek..].to_vec();
    }
    if right.cursor > 8 {
        let seek = right.cursor - 8;
        right_style_vec = right_style_vec[seek..].to_vec();
    }

    List::new(left_style_vec)
        .block(normal_block(&format!(
            "{}:{}",
            left.address,
            left.path.to_str().unwrap()
        )))
        .render(left_ui, buf);

    List::new(right_style_vec)
        .block(normal_block(&format!(
            "{}:{}",
            right.address,
            right.path.to_str().unwrap()
        )))
        .render(right_ui, buf);

    List::new(app.sftp.log.clone())
        .block(normal_block("log"))
        .render(log, buf);

    Tabs::new(vec![
        "M mkdir",
        "D delete",
        "S set remote",
        "P set path",
        "Q return",
        "T transfers",
        "C check log",
    ])
    .style(Style::default())
    .highlight_style(Style::default())
    .render(help, buf);

    if let State::Set(date) = app.sftp.set {
        Clear::default().render(ui::create_pop(60, 24, area), buf);

        let mut items = Vec::new();

        items.push(Text::styled(
            format!("address: {}", right.address),
            Style::default(),
        ));
        items.push(Text::styled(
            format!("port: {}", right.port),
            Style::default(),
        ));
        items.push(Text::styled(
            format!("username: {}", right.username),
            Style::default(),
        ));
        items.push(Text::styled(
            format!("password: {}", right.password),
            Style::default(),
        ));
        items[date] = items[date].clone().style(Style::default().bg(Color::Blue));
        List::new(items)
            .style(Style::default())
            .block(normal_block("set remote address"))
            .render(ui::create_pop(60, 24, area), buf);
    } else if let State::Delete(path) = &app.sftp.set {
        Alert::new(
            70,
            10,
            format!("Are you sure you want to delete {} (y/n)", path),
        )
        .render(area, buf);
    } else if let State::Mkdir(text) = &app.sftp.set {
        Alert::new(70, 10, format!("create dir: {}", text)).render(area, buf);
    } else if let State::Path(text) = &app.sftp.set {
        Clear::default().render(ui::create_pop(70, 10, area), buf);
        Paragraph::new(format!("set path: {}", text))
            .block(normal_block("set path"))
            .style(Style::default())
            .render(create_pop(70, 10, area), buf);
    }
}

#[derive(Clone)]
pub struct FtpStruct {
    pub left: FtpUser,
    pub right: FtpUser,
    pub cursor: Use,
    pub set: State,
    pub key_file: PathBuf,
    pub pub_file: PathBuf,
    pub log: VecDeque<String>,
}

#[derive(Clone)]
pub enum State {
    Normal,
    Set(usize),
    Mkdir(String),
    Delete(String),
    Path(String),
}

#[derive(PartialEq, Clone, Copy)]
pub enum Use {
    Left,
    Right,
}

impl FtpStruct {
    pub fn new() -> FtpStruct {
        let mut key_file = dirs::home_dir().unwrap();
        key_file.push(Path::new(".ssh"));
        let mut pub_file = key_file.clone();
        key_file.push(Path::new("id_rsa"));
        pub_file.push(Path::new("id_rsa.pub"));
        let mut ftp = FtpStruct {
            left: FtpUser::new(),
            right: FtpUser::new(),
            cursor: Use::Left,
            set: State::Normal,
            key_file,
            pub_file,
            log: VecDeque::with_capacity(20),
        };
        ftp.left.address = System::host_name().unwrap();
        for entry in fs::read_dir("/").unwrap() {
            let entry = entry.unwrap();
            if entry.metadata().unwrap().is_dir() {
                ftp.left
                    .path_vec
                    .push(entry.file_name().to_str().unwrap().to_string() + "/");
            } else {
                ftp.left
                    .path_vec
                    .push(entry.file_name().to_str().unwrap().to_string());
            }
        }
        ftp.right.address = String::from("");
        ftp
    }
    pub fn to_file(
        &mut self,
        left: &mut FtpUser,
        right: &mut FtpUser,
        src_path: &Path,
        dst_path: &Path,
        app: Arc<Mutex<App>>,
    ) -> Result<(), io::Error> {
        let mut dir = false;
        if let Some(session) = &left.session {
            if let Ok(_) = session.sftp().unwrap().opendir(src_path) {
                dir = true;
            };
        } else {
            if fs::metadata(src_path)?.is_dir() {
                dir = true;
            }
        }

        if dir {
            if let Some(session) = &right.session {
                if let Err(_) = session.sftp()?.mkdir(dst_path, 0755) {}
            } else {
                fs::create_dir_all(dst_path)?;
            }
            for file in fs::read_dir(src_path)? {
                let file = file?;
                let mut dst_path = dst_path.to_path_buf();
                dst_path.push(file.file_name());
                self.to_file(left, right, &file.path(), &dst_path, app.clone())?;
            }
        } else {
            let mut r = left.open(&src_path)?;
            let mut w = right.open(&dst_path)?;
            w.write_rw(&r.read_rw().unwrap())?;
            right.sync();
            {
                let mut app = app.lock().unwrap();
                app.sftp.log.push_front(format!(
                    "transfer file {} success",
                    src_path.file_name().unwrap().to_str().unwrap()
                ));
            }
        }
        Ok(())
    }
}

#[derive(Clone)]
pub struct FtpUser {
    pub address: String,
    pub port: String,
    pub path: PathBuf,
    pub path_vec: Vec<String>,
    pub cursor: usize,
    pub session: Option<Session>,
    pub username: String,
    pub password: String,
}

impl FtpUser {
    pub fn new() -> FtpUser {
        let mut path = PathBuf::new();
        if cfg!(windows) {
            path.push("C:\\");
        } else {
            path.push("/");
        }
        FtpUser {
            address: String::new(),
            path,
            port: String::from("22"),
            path_vec: vec![],
            cursor: 0,
            session: None,
            username: String::from("root"),
            password: String::from(""),
        }
    }

    pub fn open(&mut self, file_path: &Path) -> Result<Box<dyn RWer>, Error> {
        if let Some(session) = &self.session {
            let sftp = session.sftp()?;
            let mut flags = OpenFlags::READ;
            flags.insert(OpenFlags::WRITE);
            flags.insert(OpenFlags::CREATE);
            return Ok(Box::new(sftp.open_mode(
                file_path,
                flags,
                0755,
                ssh2::OpenType::File,
            )?));
        } else {
            return Ok(Box::new(
                File::options()
                    .create(true)
                    .read(true)
                    .write(true)
                    .open(file_path)?,
            ));
        }
    }

    pub fn sync(&mut self) {
        self.path_vec = vec![];
        if let Some(session) = &self.session {
            let sft = session.sftp().unwrap();

            for (buf, stat) in sft.readdir(Path::new(&self.path)).unwrap() {
                if stat.is_dir() {
                    self.path_vec
                        .push(buf.file_name().unwrap().to_str().unwrap().to_string() + "/")
                } else {
                    self.path_vec
                        .push(buf.file_name().unwrap().to_str().unwrap().to_string())
                }
            }
        } else {
            for entry in fs::read_dir(&self.path).unwrap() {
                let entry = entry.unwrap();
                if entry.metadata().unwrap().is_dir() {
                    self.path_vec
                        .push(entry.file_name().to_str().unwrap().to_string() + "/");
                } else {
                    self.path_vec
                        .push(entry.file_name().to_str().unwrap().to_string());
                }
            }
        }
    }
}

pub fn handle_event(app: &mut App, key: KeyEvent) -> Result<(), io::Error> {
    let mut sftp: FtpStruct;
    sftp = app.sftp.clone();
    let mut set = sftp.left.clone();
    let mut dset = sftp.right.clone();
    if sftp.cursor == Use::Right {
        set = sftp.right.clone();
        dset = sftp.left.clone();
    }
    match &sftp.set {
        State::Normal => match key.code {
            KeyCode::Down => {
                if set.cursor > set.path_vec.len() - 2 {
                    return Ok(());
                }
                set.cursor += 1;
            }
            KeyCode::Char('c') => {
                sftp.log = VecDeque::with_capacity(15);
            }
            KeyCode::Char('m') => {
                sftp.set = State::Mkdir(String::new());
            }
            KeyCode::Char('d') => {
                sftp.set = State::Delete(format!(
                    "{}{}",
                    set.path.to_str().unwrap(),
                    set.path_vec[set.cursor]
                ));
            }
            KeyCode::Char('p') => {
                sftp.set = State::Path(set.path.to_str().unwrap().to_string());
            }
            KeyCode::Up => {
                if set.cursor > 0 {
                    set.cursor -= 1;
                }
            }
            KeyCode::Left => {
                sftp.cursor = Use::Left;
                return Ok(());
            }
            KeyCode::Char('t') => {
                let mut src_path = set.path.clone();
                src_path.push(&set.path_vec[set.cursor]);
                let mut dst_path = dset.path.clone();
                dst_path.push(&set.path_vec[set.cursor]);
                sftp.to_file(&mut set, &mut dset, &src_path, &dst_path, app.clone())?;
            }
            KeyCode::Right => {
                sftp.cursor = Use::Right;
                return Ok(());
            }
            KeyCode::Enter => {
                let mut pass = set.path.clone();
                pass.push(&set.path_vec[set.cursor]);
                if fs::metadata(pass)?.is_dir() {
                    set.path.push(&set.path_vec[set.cursor]);
                    set.sync();
                    set.cursor = 0;
                }
            }
            KeyCode::Char('q') => {
                let path = PathBuf::from(&set.path);
                if let Some(path) = path.parent() {
                    set.cursor = 0;
                    set.path = path.to_path_buf();
                    set.sync();
                } else {
                    let mut app = app.lock().unwrap();
                    app.state = ClientState::Main;
                }
            }
            KeyCode::Char('s') => {
                sftp.set = State::Set(0);
            }
            _ => {}
        },
        State::Set(date) => {
            let date = *date;
            match key.code {
                KeyCode::Char(k) => {
                    if date == 0 {
                        sftp.right.address += &k.to_string();
                    } else if date == 1 && k.is_ascii_digit() {
                        sftp.right.port += &k.to_string();
                    } else if date == 2 {
                        sftp.right.username += &k.to_string();
                    } else if date == 3 {
                        sftp.right.password += &k.to_string();
                    }
                }
                KeyCode::Enter => {
                    sftp.set = State::Normal;
                    let mut ses = Session::new()?;
                    ses.set_tcp_stream(TcpStream::connect(format!(
                        "{}:{}",
                        sftp.right.address, sftp.right.port
                    ))?);
                    ses.handshake()?;
                    // if let Err(err) = ses.userauth_pubkey_file(&sftp.right.username, None, &sftp.key_file, None) {
                    //     sftp.log.push_front(err.to_string());
                    //     let key_file = sftp.key_file.to_str().unwrap().to_string();
                    //     sftp.log.push_front(key_file);
                    //     ses.userauth_password(&sftp.right.username, &sftp.right.password)?;
                    // }
                    ses.userauth_password(&sftp.right.username, &sftp.right.password)?;
                    sftp.right.session = Some(ses);
                    sftp.right.path = PathBuf::from("/");
                    sftp.right.sync();
                }
                KeyCode::Backspace => {
                    if date == 0 {
                        sftp.right.address.pop();
                    } else if date == 1 {
                        let mut sport = sftp.right.port.to_string();
                        sport.pop();
                        sftp.right.port.pop();
                    } else if date == 2 {
                        sftp.right.username.pop();
                    } else if date == 3 {
                        sftp.right.password.pop();
                    }
                }
                KeyCode::Delete => {
                    sftp.set = State::Normal;
                }
                KeyCode::Esc => {
                    sftp.set = State::Normal;
                }
                KeyCode::Down => {
                    if date < 3 {
                        sftp.set = State::Set(date + 1);
                    }
                }
                KeyCode::Up => {
                    if date > 0 {
                        sftp.set = State::Set(date - 1);
                    }
                }
                _ => {}
            }
            app.sftp = sftp;
            return Ok(());
        }
        State::Mkdir(text) => match key.code {
            KeyCode::Char(ch) => {
                sftp.set = State::Mkdir(format!("{}{}", text, ch));
            }
            KeyCode::Backspace => {
                let mut text = text.clone();
                text.pop();
                sftp.set = State::Mkdir(text);
            }
            KeyCode::Delete => {
                sftp.set = State::Normal;
            }
            KeyCode::Enter => {
                let dir_name = set.path.to_str().unwrap().to_string() + &text;
                if let Some(session) = set.session.clone() {
                    if let Err(_) = session.sftp()?.mkdir(Path::new(&dir_name), 0755) {}
                } else {
                    fs::create_dir_all(dir_name)?;
                }
                set.sync();
                sftp.set = State::Normal;
            }
            _ => {}
        },
        State::Delete(path) => match key.code {
            KeyCode::Enter | KeyCode::Char('y') => {
                let path = Path::new(&path);
                if let Some(session) = &set.session {
                    if path.is_dir() {
                        session.sftp()?.rmdir(path)?;
                    } else {
                        session.sftp()?.unlink(path)?;
                    }
                } else {
                    if path.is_dir() {
                        fs::remove_dir(path)?;
                    } else {
                        fs::remove_file(path)?;
                    }
                }
                set.sync();
                sftp.set = State::Normal;
            }
            KeyCode::Char('n') => {
                sftp.set = State::Normal;
            }
            _ => {}
        },
        State::Path(text) => match key.code {
            KeyCode::Char(ch) => {
                let mut path = text.to_owned();
                path.push(ch);
                sftp.set = State::Path(path);
            }
            KeyCode::Backspace => {
                let mut path = text.to_owned();
                path.pop();
                sftp.set = State::Path(path);
            }
            KeyCode::Enter => {
                set.cursor = 0;
                set.path = PathBuf::from(text);
                set.sync();
                sftp.set = State::Normal;
            }
            _ => {}
        },
    }

    if sftp.cursor == Use::Right {
        sftp.right = set;
        sftp.left = dset;
    } else {
        sftp.left = set;
        sftp.right = dset;
    }
    {
        let mut app = app.lock().unwrap();
        app.sftp = sftp;
    }
    Ok(())
}

pub trait RWer {
    fn write_rw(&mut self, buf: &[u8]) -> Result<(), Error>;

    fn read_rw(&mut self) -> Result<Vec<u8>, Error>;
}

impl RWer for ssh2::File {
    fn write_rw(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.write_all(buf)?;
        self.flush()?;
        self.fsync()?;
        Ok(())
    }

    fn read_rw(&mut self) -> Result<Vec<u8>, Error> {
        let mut buf: Vec<u8> = vec![];
        self.read_to_end(&mut buf)?;
        Ok(buf)
    }
}

impl RWer for File {
    fn read_rw(&mut self) -> Result<Vec<u8>, Error> {
        let mut buf: Vec<u8> = vec![];
        self.read_to_end(&mut buf)?;
        Ok(buf)
    }

    fn write_rw(&mut self, buf: &[u8]) -> Result<(), Error> {
        self.write_all(buf)?;
        self.flush()?;
        Ok(())
    }
}
