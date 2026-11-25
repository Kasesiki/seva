use seva::ui::build::Tui;

fn main() {
    let mut app = seva::App::new().expect("Create App Error");
    let terminal: Tui = ratatui::init();

    if let Err(e) = app.run(terminal) {
        eprintln!("{e}");
    };
}
