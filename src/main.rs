mod args;
mod tcp;
mod udp;
mod util;

use env_logger::{Env, DEFAULT_FILTER_ENV};

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

    let ret = match args.protocol {
        util::Protocol::Tcp => tcp::listen(args).await,
        util::Protocol::Udp => udp::listen(args).await,
    };

    if let Err(why) = ret {
        log::error!("{why}");
        return;
    }
}
