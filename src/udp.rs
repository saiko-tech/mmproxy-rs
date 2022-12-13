use std::{collections::HashMap, io, net::SocketAddr, sync::Arc};

use crate::args::Args;
use crate::util::{check_origin_allowed, parse_proxy_protocol_header, udp_create_upstream_conn};

use tokio::net::UdpSocket;

struct UdpProxyConn {
    pub upstream_conn: UdpSocket,
    pub last_activity: usize,
}

impl UdpProxyConn {
    fn new(upstream_conn: UdpSocket) -> Self {
        Self {
            upstream_conn,
            last_activity: 0,
        }
    }
}

pub async fn listen(args: Args) -> io::Result<()> {
    let socket = Arc::new(UdpSocket::bind(args.listen_addr).await?);
    log::info!("listening on: {}", socket.local_addr()?);

    let mut buffer = [0u8; u16::MAX as usize];
    let mut connections = HashMap::<SocketAddr, Arc<UdpProxyConn>>::new();

    loop {
        let (read_bytes, addr) = socket.recv_from(&mut buffer).await?;
        if let Some(ref allowed_subnets) = args.allowed_subnets {
            let ip_addr = addr.ip();

            if !check_origin_allowed(&ip_addr, allowed_subnets) {
                log::warn!("connection origin is not allowed: {ip_addr}");
                continue;
            }
        }

        if let Err(why) = udp_handle_connection(
            &args,
            socket.clone(),
            addr,
            &mut buffer[..read_bytes],
            &mut connections,
        )
        .await
        {
            log::error!("{why}");
        }
    }
}
async fn udp_handle_connection(
    args: &Args,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    buffer: &mut [u8],
    connections: &mut HashMap<SocketAddr, Arc<UdpProxyConn>>,
) -> io::Result<()> {
    let (src_addr, rest, version) = match parse_proxy_protocol_header(buffer) {
        Ok((addr_pair, rest, version)) => match addr_pair {
            Some((src, _)) => (src, rest, version),
            None => (addr, rest, version),
        },
        Err(err) => return Err(err),
    };
    if version < 2 {
        return Err(io::Error::new(
            io::ErrorKind::Other,
            "proxy protocol version 1 doesn't support UDP connections",
        ));
    }
    let target_addr = match src_addr {
        SocketAddr::V4(_) => args.ipv4_fwd,
        SocketAddr::V6(_) => args.ipv6_fwd,
    };

    let proxy_conn = match connections.get(&addr) {
        // first time connectting
        None => {
            log::info!("[new conn] [origin: {addr}] [src: {src_addr}]");
            if src_addr == addr {
                log::debug!("unknown source, using the downstream connection address");
            }

            let upstream_conn = udp_create_upstream_conn(src_addr, target_addr, args.mark).await?;
            let proxy_conn = Arc::new(UdpProxyConn::new(upstream_conn));
            let sock_clone = socket.clone();
            let proxy_clone = proxy_conn.clone();

            connections.insert(addr, proxy_conn.clone());

            tokio::spawn(async move {
                udp_copy_upstream_to_downstream(addr, sock_clone, proxy_clone).await;
            });

            proxy_conn
        }
        Some(proxy_conn) => proxy_conn.clone(),
    };

    proxy_conn.upstream_conn.writable().await?;
    proxy_conn.upstream_conn.try_send(&rest)?;

    Ok(())
}

async fn udp_copy_upstream_to_downstream(
    addr: SocketAddr,
    downstream: Arc<UdpSocket>,
    upstream: Arc<UdpProxyConn>,
) {
    let mut buffer = [0u8; u16::MAX as usize];

    loop {
        let read_bytes = upstream.upstream_conn.recv(&mut buffer).await.unwrap();
        downstream
            .send_to(&buffer[..read_bytes], addr)
            .await
            .unwrap();
    }
}
