// TODO: delete this once the structure of the project is set in stone
#![allow(dead_code, unused_imports, unused_variables)]

mod args;
mod misc;
mod tcp;

use env_logger::{Env, DEFAULT_FILTER_ENV};
use std::io;

use misc::Protocol;
use tcp::listen as tcp_listen;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));

    // parse all of the command line arguments
    let args = match args::Args::args() {
        Ok(args) => args,
        Err(why) => {
            log::error!("{}", why);
            return;
        }
    };
    if args.help {
        return;
    }

    let allowed_subnets = match args.allowed_subnets {
        Some(ref path) => match misc::parse_allowed_subnets(&path) {
            Ok(data) => Some(data),
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
        Protocol::Tcp => tcp_listen(&args).await,
        Protocol::Udp => unimplemented!(),
    };

    if let Err(why) = result {
        log::error!("{why}");
        return;
    }
}
