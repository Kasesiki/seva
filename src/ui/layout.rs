use ratatui::layout::{Constraint, Layout, Rect};

use crate::ui::art;

pub fn main_layout(
    area: Rect,
    buf: &mut ratatui::prelude::Buffer,
) -> (Rect, Rect, Rect, Rect, Rect) {
    let [tabs, main] = Layout::vertical([Constraint::Length(3), Constraint::Fill(0)]).areas(area);

    if buf.area.as_size().height > 25 {
        let [art_network, mem_os_process] =
            Layout::horizontal([Constraint::Length(53), Constraint::Fill(1)]).areas(main);

        let [art, network] =
            Layout::vertical([Constraint::Length(24), Constraint::Fill(1)]).areas(art_network);

        let [mem_os, process] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(mem_os_process);

        let [line, os] =
            Layout::vertical([Constraint::Length(7), Constraint::Fill(1)]).areas(mem_os);

        art::render_logo(art, buf);

        (tabs, line, os, network, process)
    } else {
        let [top, process] =
            Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(main);
        let [cpu_mem_os, os] =
            Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(top);

        let [line, network] =
            Layout::vertical([Constraint::Max(7), Constraint::Fill(0)]).areas(cpu_mem_os);

        (tabs, line, os, network, process)
    }
}

pub fn trend_layout(area: Rect, _buf: &mut ratatui::prelude::Buffer) -> (Rect, Rect, Rect) {
    let [trend_disk, process] =
        Layout::vertical([Constraint::Fill(1), Constraint::Fill(1)]).areas(area);

    let [trend, disk] =
        Layout::horizontal([Constraint::Fill(1), Constraint::Fill(1)]).areas(trend_disk);

    (trend, disk, process)
}

pub fn info_layout(
    area: Rect,
    _buf: &mut ratatui::prelude::Buffer,
) -> (Rect, Rect, Rect, Rect, Rect, Rect) {
    let [hello, area] = Layout::vertical([Constraint::Length(3), Constraint::Fill(1)]).areas(area);
    let [hello, _] = Layout::horizontal([Constraint::Fill(1), Constraint::Fill(3)]).areas(hello);
    let [motherboard, cpu, memory] = Layout::horizontal([
        Constraint::Fill(1),
        Constraint::Fill(1),
        Constraint::Fill(1),
    ])
    .areas(area);

    let [disk_space, _empty] =
        Layout::horizontal([Constraint::Fill(2), Constraint::Fill(1)]).areas(area);
    let [_empty, disk] = Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)])
        .areas(disk_space);

    let [product_cache, _empty] =
        Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)])
            .areas(motherboard);
    let [product, cache] =
        Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)])
            .areas(product_cache);

    let [cpu, _empty] =
        Layout::vertical([Constraint::Percentage(70), Constraint::Percentage(30)]).areas(cpu);
    (hello, product, cache, cpu, disk, memory)
}
