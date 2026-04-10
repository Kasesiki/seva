#[tokio::main]
async fn main() {
    // let args: Vec<String> = env::args().collect();

    if let Err(err) = seva::client::client::main().await {
        println!("{:?}", err);
    }
}
