use std::{io, net::SocketAddr};

use crate::args::Args;
use crate::util::{check_origin_allowed, create_upstream_conn, parse_proxy_protocol_header};

use tokio::net::UdpSocket;

pub async fn listen(args: Args) -> io::Result<()> {
    let socket = UdpSocket::bind(args.listen_addr).await?;
    let mut buffer = [0u8; u16::MAX as usize];

    loop {
        let (read_bytes, addr) = socket.recv_from(&mut buffer).await?;
        log::info!("new connection: {addr}");

        if let Some(ref allowed_subnets) = args.allowed_subnets {
            let ip_addr = addr.ip();

            if !check_origin_allowed(&ip_addr, allowed_subnets) {
                log::debug!("connection origin is not allowed");
                continue;
            }
        }

        let (src_addr, mut rest, version) = match parse_proxy_protocol_header(&buffer[..read_bytes])
        {
            Ok((addr_pair, rest, version)) => match addr_pair {
                Some((src, _)) => (src, rest, version),
                None => (addr, rest, version),
            },
            Err(why) => {
                log::error!("{why}");
                continue;
            }
        };
        if version < 2 {
            log::warn!("proxy protocol version 1 doesn't support UDP connections");
            continue;
        }
        if src_addr == addr {
            log::debug!("unknown source, using the downstream connection address");
        }

        let target_addr = match src_addr {
            SocketAddr::V4(_) => args.ipv4_fwd,
            SocketAddr::V6(_) => args.ipv6_fwd,
        };

        log::info!("source addr: {src_addr}");
        log::info!("target addr: {target_addr}");

        println!("{:?}", String::from_utf8_lossy(&buffer[..read_bytes]));
    }
}
