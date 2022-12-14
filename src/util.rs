use std::net::{IpAddr, SocketAddr};
use std::{
    fs::File,
    io::{self, Read},
    str::FromStr,
};

use proxy_protocol::{version1 as v1, version2 as v2, ProxyHeader};
use socket2::{Domain, SockRef, Socket, Type};
use tokio::net::{TcpSocket, TcpStream, UdpSocket};

// this is returned from `util::parse_proxy_protocol_header` function
pub type ProxyProtocolResult<'a> = io::Result<(Option<(SocketAddr, SocketAddr)>, &'a [u8], i32)>;

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

pub fn check_origin_allowed(addr: &IpAddr, subnets: &[cidr::IpCidr]) -> bool {
    for net in subnets.iter() {
        if net.contains(addr) {
            return true;
        }
    }

    false
}

pub fn parse_allowed_subnets(path: &str) -> io::Result<Vec<cidr::IpCidr>> {
    let mut data = Vec::new();
    let mut file = File::open(path)?;

    let mut contents = String::new();
    file.read_to_string(&mut contents)?;

    for line in contents.lines() {
        match cidr::IpCidr::from_str(line) {
            Ok(cidr) => data.push(cidr),
            Err(why) => {
                return Err(io::Error::new(io::ErrorKind::Other, why));
            }
        }
    }

    Ok(data)
}

fn setup_socket(socket_ref: &SockRef, src: SocketAddr, mark: u32) -> io::Result<()> {
    // needs CAP_NET_ADMIN
    socket_ref.set_ip_transparent(true)?;

    socket_ref.set_nonblocking(true)?;
    socket_ref.set_reuse_address(true)?;
    socket_ref.set_mark(mark)?;
    socket_ref.bind(&src.into())?;

    Ok(())
}

pub async fn tcp_create_upstream_conn(
    src: SocketAddr,
    target: SocketAddr,
    mark: u32,
) -> io::Result<TcpStream> {
    let socket = match src {
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
    };

    let socket_ref = SockRef::from(&socket);
    socket_ref.set_nodelay(true)?;
    setup_socket(&socket_ref, src, mark)?;

    socket.connect(target).await
}

pub async fn udp_create_upstream_conn(
    src: SocketAddr,
    target: SocketAddr,
    mark: u32,
) -> io::Result<UdpSocket> {
    let socket = match src {
        SocketAddr::V4(_) => Socket::new(Domain::IPV4, Type::DGRAM, None)?,
        SocketAddr::V6(_) => Socket::new(Domain::IPV6, Type::DGRAM, None)?,
    };

    setup_socket(&SockRef::from(&socket), src, mark)?;
    let udp_socket = UdpSocket::from_std(socket.into())?;
    udp_socket.connect(target).await?;

    Ok(udp_socket)
}

pub fn parse_proxy_protocol_header(mut buffer: &[u8]) -> ProxyProtocolResult {
    match proxy_protocol::parse(&mut buffer) {
        Ok(result) => match result {
            ProxyHeader::Version1 { addresses } => match addresses {
                v1::ProxyAddresses::Unknown => Ok((None, buffer, 1)),
                v1::ProxyAddresses::Ipv4 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V4(source), SocketAddr::V4(destination))),
                    buffer,
                    1,
                )),
                v1::ProxyAddresses::Ipv6 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V6(source), SocketAddr::V6(destination))),
                    buffer,
                    1,
                )),
            },
            ProxyHeader::Version2 { addresses, .. } => match addresses {
                v2::ProxyAddresses::Unspec => Ok((None, buffer, 2)),
                v2::ProxyAddresses::Ipv4 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V4(source), SocketAddr::V4(destination))),
                    buffer,
                    2,
                )),
                v2::ProxyAddresses::Ipv6 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V6(source), SocketAddr::V6(destination))),
                    buffer,
                    2,
                )),
                v2::ProxyAddresses::Unix { .. } => Err(io::Error::new(
                    io::ErrorKind::Other,
                    "unix sockets are not supported",
                )),
            },
            _ => unreachable!(),
        },
        Err(err) => Err(io::Error::new(io::ErrorKind::Other, err)),
    }
}
