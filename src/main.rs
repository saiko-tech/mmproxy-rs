mod args;
mod listeners;
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
        util::Protocol::Tcp => listeners::tcp_listen(args).await,
        util::Protocol::Udp => unimplemented!(),
    };

    if let Err(why) = ret {
        log::error!("{why}");
        return;
    }
}
