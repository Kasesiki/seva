use seva::client::{
    art,
    client::{self, App},
    ui,
};

#[tokio::main]
async fn main() {
    // let args: Vec<String> = env::args().collect();
    art::init_art();
    let app = App::new().expect("Create App Error");
    let terminal: ui::Tui = ratatui::init();
    if let Err(e) = client::run(app, terminal).await {
        eprintln!("{e}");
    };
}
