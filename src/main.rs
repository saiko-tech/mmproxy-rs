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

    // parse all of the command line arguments
    let args = match args::Args::args() {
        Ok(args) => {
            if args.help {
                return;
            }
            args
        }
        Err(why) => {
            log::error!("{}", why);
            return;
        }
    };

    dbg!(&args);

    let result = match args.protocol {
        Protocol::Tcp => listeners::tcp_listen(args).await,
        Protocol::Udp => unimplemented!(),
    };

    if let Err(why) = result {
        log::error!("{why}");
        return;
    }
}
