use seva::ui::{art, build::Tui};

#[tokio::main]
async fn main() {
    // let args: Vec<String> = env::args().collect();
    art::init_art();
    let mut app = seva::App::new().expect("Create App Error");
    let terminal: Tui = ratatui::init();

    if let Err(e) = app.run(terminal).await {
        eprintln!("{e}");
    };
}
