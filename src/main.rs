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

    let allowed_subnets = match args.allowed_subnets {
        Some(ref path) => match util::parse_allowed_subnets(&path) {
            Ok(data) => {
                if data.len() > 0 {
                    Some(data)
                } else {
                    None
                }
            }
            Err(why) => {
                log::error!("{}", why);
                return;
            }
        },
        None => None,
    };

    dbg!(&args);
    dbg!(&allowed_subnets);

    let result = match args.protocol {
        Protocol::Tcp => listeners::tcp_listen(args, allowed_subnets).await,
        Protocol::Udp => unimplemented!(),
    };

    if let Err(why) = result {
        log::error!("{why}");
        return;
    }
}
