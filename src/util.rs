use std::fs::File;
use std::io::{self, Read};
use std::net::{IpAddr, SocketAddr};
use std::str::FromStr;

use cidr::IpCidr;
use proxy_protocol::{version1 as v1, version2 as v2, ProxyHeader};

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

pub fn parse_allowed_subnets(path: &str) -> io::Result<Vec<IpCidr>> {
    let mut ret = Vec::new();
    let mut data = String::new();
    let mut file = File::open(path)?;

    file.read_to_string(&mut data)?;
    for line in data.lines() {
        match IpCidr::from_str(line) {
            Ok(cidr) => ret.push(cidr),
            Err(why) => {
                return Err(io::Error::new(io::ErrorKind::Other, why));
            }
        }
    }

    Ok(ret)
}

pub fn check_origin_allowed(addr: &IpAddr, subnets: &[IpCidr]) -> bool {
    for net in subnets.iter() {
        if net.contains(addr) {
            return true;
        }
    }

    false
}

pub fn parse_proxy_protocol_header(
    mut buffer: &[u8],
) -> io::Result<(Option<(SocketAddr, SocketAddr)>, &[u8])> {
    match proxy_protocol::parse(&mut buffer) {
        Ok(result) => match result {
            ProxyHeader::Version1 { addresses } => match addresses {
                v1::ProxyAddresses::Unknown => Ok((None, buffer)),
                v1::ProxyAddresses::Ipv4 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V4(source), SocketAddr::V4(destination))),
                    buffer,
                )),
                v1::ProxyAddresses::Ipv6 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V6(source), SocketAddr::V6(destination))),
                    buffer,
                )),
            },
            ProxyHeader::Version2 { addresses, .. } => match addresses {
                v2::ProxyAddresses::Unspec => Ok((None, buffer)),
                v2::ProxyAddresses::Ipv4 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V4(source), SocketAddr::V4(destination))),
                    buffer,
                )),
                v2::ProxyAddresses::Ipv6 {
                    source,
                    destination,
                } => Ok((
                    Some((SocketAddr::V6(source), SocketAddr::V6(destination))),
                    buffer,
                )),
                _ => todo!(),
            },
            _ => unreachable!(),
        },
        Err(err) => Err(io::Error::new(io::ErrorKind::Other, err)),
    }
}
