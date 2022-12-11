// TODO: delete this once the structure of the project is set in stone
#![allow(dead_code, unused_imports, unused_variables)]

mod args;
mod listeners;
mod util;

use env_logger::{Env, DEFAULT_FILTER_ENV};
use std::io;
use util::Protocol;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));

    let args = match args::parse_args() {
        Ok(args) => args,
        Err(why) => {
            log::error!("{why}");
            return;
        }
    };

    let result = match args.protocol {
        Protocol::Tcp => listeners::tcp_listen(args).await,
        Protocol::Udp => unimplemented!(),
    };

    if let Err(why) = result {
        log::error!("{why}");
        return;
    }
}
