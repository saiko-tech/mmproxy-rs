use crate::Protocol;

argwerk::define! {
    #[derive(Default)]
    #[usage = "mmproxy [options] -m <mark>"]
    pub struct Args {
        pub help: bool,
        pub ipv4_fwd: String = "127.0.0.1:443".to_string(),
        pub ipv6_fwd: String = "[::1]:443".to_string(),
        pub allowed_subnets: Option<String>,
        pub close_after: u32 = 60,
        #[required = "mark is required"]
        pub mark: i32,
        pub listen: String = "0.0.0.0:8443".to_string(),
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
        ipv4_fwd = addr;
    }
    /// Address to which IPv6 traffic will be forwarded to. (default: "[::1]:443")
    ["-6" | "--ipv6", addr] => {
        ipv6_fwd = addr;
    }
    /// Path to a file that contains allowed subnets of the proxy servers.
    ["-a" | "--allowed-subnets", path] => {
        allowed_subnets = Some(path);
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
