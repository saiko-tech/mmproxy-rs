use crate::util::{self, Protocol};
use std::net::SocketAddr;
use std::time::Duration;

argwerk::define! {
    #[usage = "mmproxy [-h] [options]"]
    #[derive(Clone)]
    pub struct Args {
        pub help: bool = false,
        pub ipv4_fwd: SocketAddr = "127.0.0.1:443".parse().unwrap(),
        pub ipv6_fwd: SocketAddr = "[::1]:443".parse().unwrap(),
        pub allowed_subnets: Option<Vec<cidr::IpCidr>> = None,
        pub close_after: Duration = Duration::from_secs(60),
        pub mark: u32 = 0,
        pub listen_addr: SocketAddr = "0.0.0.0:8443".parse().unwrap(),
        pub listeners: u32 = 1,
        pub protocol: Protocol = Protocol::Tcp
    }
    /// Prints the help string.
    ["-h" | "--help"] => {
        println!("{}", Args::help());
        help = true;
    }
    /// Address to which IPv4 traffic will be forwarded to. (default: "127.0.0.1:443")
    ["-4" | "--ipv4", addr] => {
        ipv4_fwd = addr.parse()?;
    }
    /// Address to which IPv6 traffic will be forwarded to. (default: "[::1]:443")
    ["-6" | "--ipv6", addr] => {
        ipv6_fwd = addr.parse()?;
    }
    /// Path to a file that contains allowed subnets of the proxy servers.
    ["-a" | "--allowed-subnets", path] => {
        let ret = util::parse_allowed_subnets(&path)?;
        allowed_subnets = if ret.len() > 0 { Some (ret) } else { None }
    }
    /// Number of seconds after which UDP socket will be cleaned up. (default: 60)
    ["-c" | "--close-after", n] => {
        close_after = Duration::from_secs(str::parse(&n)?);
    }
    /// Address the proxy listens on. (default: "0.0.0.0:8443")
    ["-l" | "--listen-addr", string] => {
        listen_addr = string.parse()?;
    }
    /// Number of listener sockets that will be opened for the listen address. (Linux 3.9+) (default: 1)
    ["--listeners", n] => {
        listeners = str::parse(&n)?;
    }
    /// Protocol that will be proxied: tcp, udp. (default: tcp)
    ["-p" | "--protocol", p] => {
        protocol = match &p.to_lowercase()[..] {
            "tcp" => Protocol::Tcp,
            "udp" => Protocol::Udp,
            _ => return Err(format!("invalid protocol value: {p}").into()),
        };
    }
    /// The mark that will be set on outbound packets. (default: 0)
    ["-m" | "--mark", n] => {
        mark = str::parse::<u32>(&n)?;
    }
}

pub fn parse_args() -> Result<Args, argwerk::Error> {
    match Args::args() {
        Ok(args) => {
            if args.help {
                std::process::exit(1);
            }
            Ok(args)
        }
        Err(err) => Err(err),
    }
}
