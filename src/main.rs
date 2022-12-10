use env_logger::{Env, DEFAULT_FILTER_ENV};

fn main() {
    env_logger::init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));
    log::info!("Hello, World!");
}
