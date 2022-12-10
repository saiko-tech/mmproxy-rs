// TODO: delete this once the structure of the project is set in stone
#![allow(
    dead_code,
    unused_imports,
    unused_variables)]

use env_logger::{Env, DEFAULT_FILTER_ENV};
use std::io;

// TODO: move this into another module
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Protocol {
    Tcp,
    Udp,
}

impl Default for Protocol {
    fn default() -> Self {
        Self::Tcp
    }
}

#[tokio::main]
async fn main() {
    env_logger::init_from_env(Env::default().filter_or(DEFAULT_FILTER_ENV, "debug"));

    argwerk::define! {
        #[derive(Default)]
        #[usage = "mmproxy [options] -m <mark>"]
        struct Args {
            help: bool,
            close_after: i32 = 60,
            #[required = "mark is required"]
            mark: i32,
            listen: String = "0.0.0.0:8443".to_string(),
            protocol: Protocol = Protocol::Tcp
        }
        /// Prints the help.
        ["-h" | "--help"] => {
            println!("{}", Args::help());
            help = true;
        }
        /// Number of seconds after which UDP socket will be cleaned up. (default: 60)
        ["-c" | "--close-after", n] => {
            close_after = str::parse(&n)?;
        }
        /// Address the proxy listens on. (default: "0.0.0.0:8443")
        ["-l" | "--listen", #[option] string] => {
            if let Some(string) = string {
                listen = string;
            }
        }
        /// Protocol that will be proxied: tcp, udp. (default: tcp)
        ["-p" | "--protocol", #[option] p] => {
            if let Some(p) = p {
                protocol = match &p[..] {
                    "tcp" => Protocol::Tcp,
                    "udp" => Protocol::Udp,
                    _ => return Err(format!("invalid protocol value: {p}").into()),
                };
            }
        }
        /// The mark that will be set on outbound packets.
        ["-m" | "--mark", n] => {
            mark = Some(str::parse::<i32>(&n)?);
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
        return;
    }

    dbg!(&args);
}
