use crate::util;
use crate::Protocol;
use cidr::IpCidr;
use std::net::SocketAddr;

argwerk::define! {
    // #[derive(Default)]
    #[usage = "mmproxy [options] -m <mark>"]
    pub struct Args {
        pub help: bool,
        pub ipv4_fwd: SocketAddr = "127.0.0.1:443".parse().unwrap(),
        pub ipv6_fwd: SocketAddr = "[::1]:443".parse().unwrap(),
        pub allowed_subnets: Option<Vec<IpCidr>>,
        pub close_after: u32 = 60,
        #[required = "mark is required"]
        pub mark: i32,
        pub listen_addr: SocketAddr = "0.0.0.0:8443".parse().unwrap(),
        pub listeners: u32 = 1,
        pub protocol: Protocol = Protocol::Tcp
    }
    /// Prints the help.
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
        close_after = str::parse(&n)?;
    }
    /// Address the proxy listens on. (default: "0.0.0.0:8443")
    ["-l" | "--listen-addr", #[option] string] => {
        if let Some(string) = string {
            listen_addr = string.parse()?;
        }
    }
    /// Number of listener sockets that will be opened for the listen address. (Linux 3.9+) (default: 1)
    ["--listeners", n] => {
        listeners = str::parse(&n)?;
    }
    /// Protocol that will be proxied: tcp, udp. (default: tcp)
    ["-p" | "--protocol", #[option] p] => {
        if let Some(p) = p {
            protocol = match &p.to_lowercase()[..] {
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
