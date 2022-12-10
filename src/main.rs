use env_logger::{Env, DEFAULT_FILTER_ENV};
use std::io;

#[tokio::main]
async fn main() -> io::Result<()> {
    env_logger::init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));
    log::info!("Hello, World!");

    Ok(())
}
