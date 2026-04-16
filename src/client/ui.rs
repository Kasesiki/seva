use queue::Queue;
use ratatui::{
    Terminal,
    buffer::Buffer,
    layout::{Constraint, Flex, Layout, Rect, Spacing},
    style::{Color, Style, Stylize},
    symbols::{Marker, border, merge::MergeStrategy},
    text::Text,
    widgets::{
        Axis, Block, Chart, Clear, Dataset, GraphType, List, Padding, Paragraph, Widget, Wrap,
    },
};
use std::{io, ops::Deref, vec};

use crate::client::system::{byte_to_string, sec_to_time};

use super::art;

pub type Tui = Terminal<ratatui::prelude::CrosstermBackend<io::Stdout>>;

pub fn trend_ui(
    app: &crate::App,
    area: ratatui::prelude::Rect,
    buf: &mut ratatui::prelude::Buffer,
) {
    let [trend_disk, process] =
        Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

    let [trend, disk] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(trend_disk);
    // let [network, disk] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(network_disk);
    let pc = PercentageChart::set(
        String::from("trend"),
        vec![
            app.sys_line.memory_data.clone(),
            app.sys_line.swap_data.clone(),
            app.sys_line.cpu_data.clone(),
        ],
        vec![
            String::from("mem"),
            String::from("swap"),
            String::from("cpu"),
        ],
        vec![
            Style::new().red(),
            Style::new().green(),
            Style::new().cyan(),
        ],
    );
    pc.build(trend, buf);

    let item1 = Text::from(app.formats.disk_text.as_str())
        .centered()
        .bg(Color::White)
        .fg(Color::White);
    List::new(item1)
        .block(normal_block("Disk"))
        .render(disk, buf);

    let [process_top, process] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(5)]).areas(process);

    Paragraph::new("   PID      %CPU        MEM           TIME             CMD")
        .render(process_top, buf);

    let items = app.extend.processes.iter().map(|process| {
        format!(
            "{:<9}{:<12}{:<14}{:<17}{}",
            process.pid.as_u32(),
            format!("{:.2}%", process.cpu_usage),
            byte_to_string(process.memory),
            sec_to_time(process.run_time),
            process.cmd
        )
    });

    List::new(items)
        .block(normal_block("process"))
        .render(process, buf);
}

pub fn main_ui(app: &crate::App, area: ratatui::prelude::Rect, buf: &mut ratatui::prelude::Buffer) {
    let [tabs, main] = Layout::vertical([Constraint::Length(3), Constraint::Fill(0)]).areas(area);

    app.formats.tab.deref().render(tabs, buf);
    let (memline, os, network, process) = if buf.area.as_size().height > 25 {
        let [art_memory_os, network_process] =
            Layout::vertical([Constraint::Length(24), Constraint::Fill(1)]).areas(main);

        let [art, memory_os] =
            Layout::horizontal([Constraint::Length(53), Constraint::Fill(1)]).areas(art_memory_os);

        art::render_logo(art, buf);

        let [mem_line, os] =
            Layout::vertical([Constraint::Max(7), Constraint::Fill(0)]).areas(memory_os);

        let [network, process] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(network_process);

        (mem_line, os, network, process)
    } else {
        let [top, process] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(main);
        let [cpu_mem_os, os] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(top);

        let [mem_line, network] =
            Layout::vertical([Constraint::Max(7), Constraint::Fill(0)]).areas(cpu_mem_os);

        (mem_line, os, network, process)
    };

    let [cpu, memory, swap] = Layout::vertical([
        Constraint::Length(3),
        Constraint::Length(3),
        Constraint::Length(4),
    ])
    .spacing(Spacing::Overlap(1))
    .areas(memline);

    let [process_top, process] =
        Layout::vertical([Constraint::Length(1), Constraint::Min(5)]).areas(process);

    app.formats.mem_line.deref().render(memory, buf);

    Paragraph::new(format!(
        "{}/{} ",
        byte_to_string(app.sys.used_memory()),
        byte_to_string(app.sys.total_memory())
    ))
    .block(normal_block("memory").merge_borders(MergeStrategy::Exact))
    .alignment(ratatui::layout::HorizontalAlignment::Right)
    .render(memory, buf);

    app.formats.swap_line.deref().render(swap, buf);

    Paragraph::new(format!(
        "{}/{} ",
        byte_to_string(app.sys.used_swap()),
        byte_to_string(app.sys.total_swap())
    ))
    .block(normal_block("swap").merge_borders(MergeStrategy::Exact))
    .alignment(ratatui::layout::HorizontalAlignment::Right)
    .render(swap, buf);

    app.formats.cpu_line.deref().render(cpu, buf);

    Paragraph::new(format!("{}%", app.sys.global_cpu_usage()))
        .block(normal_block("cpu").merge_borders(MergeStrategy::Exact))
        .alignment(ratatui::layout::HorizontalAlignment::Right)
        .render(cpu, buf);

    Paragraph::new(app.formats.os_message_format.clone())
        .wrap(Wrap { trim: true })
        .block(normal_block("os"))
        .render(os, buf);

    let mut items: Vec<String> = vec![];
    for (pid, item) in &app.network {
        items.push(format!(
            "{}: {} (Down) / {} (Up)",
            pid,
            byte_to_string(item.total_received()),
            byte_to_string(item.total_transmitted())
        ));
    }

    List::new(items)
        .block(normal_block("network"))
        .render(network, buf);

    Paragraph::new("    PID      NAME            %CPU       MEM").render(process_top, buf);

    let items = app.extend.processes.iter().map(|x| {
        format!(
            "{:<9}{:<17}{:<10}{}",
            x.pid.as_u32(),
            x.name,
            format!("{:.2}%", x.cpu_usage),
            byte_to_string(x.memory),
        )
    });

    List::new(items)
        .block(normal_block("process"))
        .render(process, buf);
}

#[derive(PartialEq, Clone)]
pub enum ClientState {
    Trend,
    Main,
    // Sftp,
    Serve,
}

pub fn normal_block(name: &str) -> Block<'_> {
    Block::bordered()
        .title(name)
        .padding(Padding::horizontal(2))
        .border_style(Style::default().fg(ratatui::style::Color::Yellow))
        .border_set(border::THICK)
}

pub struct PercentageChart {
    data: Vec<Queue<f64>>,
    data_name: Vec<String>,
    name: String,
    colors: Vec<Style>,
    live_vec: Vec<Vec<(f64, f64)>>,
}

impl PercentageChart {
    pub fn set(
        name: String,
        data: Vec<Queue<f64>>,
        data_name: Vec<String>,
        colors: Vec<Style>,
    ) -> PercentageChart {
        PercentageChart {
            data,
            data_name,
            name,
            colors,
            live_vec: vec![],
        }
    }
    pub fn build(mut self, area: Rect, buf: &mut Buffer) {
        let mut top: f64 = 0.0;
        let mut min: f64 = 100.0;
        let mut capacity: f64 = self.data[0].capacity().unwrap() as f64;
        let mut re_vec = vec![];
        self.data.into_iter().for_each(|data| {
            if capacity > data.capacity().unwrap() as f64 {
                capacity = data.capacity().unwrap() as f64;
            }
            let mut live_vec = vec![];

            for (i, v) in data.vec().iter().enumerate() {
                let v = *v;
                if v > top {
                    top = v + 1.0;
                }
                if v < min && v >= 0.4 {
                    min = v - 0.4;
                } else if v < min {
                    min = v;
                }
                live_vec.push((i as f64, v));
            }
            self.live_vec.push(live_vec);
        });
        for (ref_i, point) in self.live_vec.iter().enumerate() {
            re_vec.push(
                Dataset::default()
                    .name(self.data_name[ref_i].clone())
                    .marker(Marker::Braille)
                    .graph_type(GraphType::Line)
                    .style(self.colors[ref_i])
                    .data(point),
            );
        }

        top = format!("{:.2}", top).parse::<f64>().unwrap();
        min = format!("{:.2}", min).parse::<f64>().unwrap();
        let mid = format!("{:.2}", (top + min) / 2.0).parse::<f64>().unwrap();
        let tmid = format!("{:.2}", (top + mid) / 2.0).parse::<f64>().unwrap();
        let mmid = format!("{:.2}", (min + mid) / 2.0).parse::<f64>().unwrap();

        Chart::new(re_vec)
            .x_axis(Axis::default().bounds([0.0, capacity]))
            .y_axis(Axis::default().bounds([min, top]).labels([
                min.to_string() + "%",
                mmid.to_string() + "%",
                mid.to_string() + "%",
                tmid.to_string() + "%",
                top.to_string() + "%",
            ]))
            .hidden_legend_constraints((Constraint::Ratio(1, 2), Constraint::Ratio(1, 2)))
            .block(normal_block(&self.name))
            .render(area, buf);
    }
}

pub fn create_pop(x: u16, y: u16, area: Rect) -> Rect {
    let vertical = Layout::vertical([Constraint::Percentage(y)]).flex(Flex::Center);
    let horizontal = Layout::horizontal([Constraint::Percentage(x)]).flex(Flex::Center);
    let [area] = vertical.areas(area);
    let [area] = horizontal.areas(area);
    area
}

pub fn set_alert(area: Rect, buf: &mut Buffer, text: &str) {
    Clear.render(create_pop(70, 10, area), buf);
    Paragraph::new(text)
        .block(normal_block("alert"))
        .render(create_pop(60, 10, area), buf);
}

pub struct Alert {
    x: u16,
    y: u16,
    text: String,
    title: String,
}

impl Alert {
    pub fn new(x: u16, y: u16, text: String) -> Alert {
        Alert {
            x,
            y,
            text,
            title: String::new(),
        }
    }

    pub fn set_title(mut self, title: String) -> Self {
        self.title = title;
        self
    }

    pub fn render(self, area: Rect, buf: &mut Buffer) {
        Clear.render(create_pop(self.x, self.y, area), buf);
        Paragraph::new(self.text)
            .block(normal_block(&self.title))
            .render(create_pop(self.x, self.y, area), buf);
    }
}
