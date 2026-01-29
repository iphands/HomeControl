mod colors;
mod looper;
mod modes;
mod opts;
mod server;
mod strip;

use looper::Looper;
use std::env;

#[actix_web::main]
async fn main() -> std::io::Result<()> {
    let args: Vec<String> = env::args().collect();
    let debug_mode = args.contains(&"--debug".to_string()) || args.contains(&"-d".to_string());

    println!("Starting HomeCtrl Server (Rust)");
    println!("Debug mode: {}", debug_mode);

    let looper = Looper::new(debug_mode);

    server::start_server(looper).await
}
