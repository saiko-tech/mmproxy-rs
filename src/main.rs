// TODO: delete this once the structure of the project is set in stone
#![allow(
    dead_code,
    unused_imports,
    unused_variables)]

use env_logger::{Env, DEFAULT_FILTER_ENV};
use std::io;

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));

    argwerk::define! {
        #[derive(Default)]
        #[usage = "mmproxy [-h]"]
        struct Args {
            help: bool,
            mark: i32,
        }

        /// Prints the help.
        ["-h" | "--help"] => {
            help = true;
        }

        /// The mark that will be set on outbound packets.
        ["-m" | "--mark", n] => {
            mark = str::parse(&n)?;
        }
    }

    // parse all of the command line arguments
    let args = match Args::args() {
        Ok(args) => args,
        Err(why) => {
            log::error!("{}", why);
            return;
        }
    };

    if args.help {
        println!("{}", Args::help());
        return;
    }

    dbg!(&args);
}
