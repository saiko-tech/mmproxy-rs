// TODO: needs heavy refactoring

use std::{collections::HashMap, io, net::SocketAddr, sync::Arc};

use crate::args::Args;
use crate::util::{check_origin_allowed, parse_proxy_protocol_header, udp_create_upstream_conn};

use tokio::net::UdpSocket;
use tokio::sync::mpsc;

#[derive(Debug)]
struct UdpProxyConn {
    pub upstream_conn: UdpSocket,
    pub last_activity: std::sync::atomic::AtomicU64,
}

impl UdpProxyConn {
    fn new(upstream_conn: UdpSocket) -> Self {
        Self {
            upstream_conn,
            last_activity: std::sync::atomic::AtomicU64::new(0),
        }
    }
}

pub async fn listen(args: Args) -> io::Result<()> {
    let socket = Arc::new(UdpSocket::bind(args.listen_addr).await?);
    log::info!("listening on: {}", socket.local_addr()?);

    let mut buffer = [0u8; u16::MAX as usize];
    let mut connections =
        HashMap::<SocketAddr, (Arc<UdpProxyConn>, tokio::task::JoinHandle<()>)>::new();
    let (ctx, mut crx) = mpsc::channel::<SocketAddr>(128);

    loop {
        tokio::select! {
            ret = socket.recv_from(&mut buffer) => {
                let (read, addr) = ret?;
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
                    &mut buffer[..read],
                    &mut connections,
                    ctx.clone(),
                )
                .await
                {
                    log::error!("{why}");
                }
            }
            // close connections in this branch
            addr = crx.recv() => {
                if let Some(addr) = addr {
                    if let Some((_conn, handle)) = connections.remove(&addr) {
                        log::info!("closing {addr} due inactivity");
                        handle.abort();
                    }
                }
            }
        }
    }
}

async fn udp_handle_connection(
    args: &Args,
    socket: Arc<UdpSocket>,
    addr: SocketAddr,
    buffer: &mut [u8],
    connections: &mut HashMap<SocketAddr, (Arc<UdpProxyConn>, tokio::task::JoinHandle<()>)>,
    ctx: mpsc::Sender<SocketAddr>,
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
        // first time connecting
        None => {
            log::info!("[new conn] [origin: {addr}] [src: {src_addr}]");
            if src_addr == addr {
                log::debug!("unknown source, using the downstream connection address");
            }

            let upstream_conn = udp_create_upstream_conn(src_addr, target_addr, args.mark).await?;
            let proxy_conn = Arc::new(UdpProxyConn::new(upstream_conn));
            let sock_clone = socket.clone();
            let (ctx1, proxy_clone1) = (ctx.clone(), proxy_conn.clone());
            let (ctx2, proxy_clone2) = (ctx.clone(), proxy_conn.clone());

            let close_after = args.close_after;
            let handle = tokio::spawn(async move {
                udp_copy_upstream_to_downstream(addr, ctx1, sock_clone, proxy_clone1).await;
            });
            tokio::spawn(async move {
                udp_close_after_inactivity(addr, close_after, ctx2, proxy_clone2).await;
            });

            connections.insert(addr, (proxy_conn.clone(), handle));

            proxy_conn
        }
        Some((proxy_conn, _handle)) => {
            proxy_conn
                .last_activity
                .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
            proxy_conn.clone()
        }
    };

    proxy_conn.upstream_conn.writable().await?;
    proxy_conn.upstream_conn.try_send(&rest)?;

    Ok(())
}

// TODO: do proper error handling
async fn udp_copy_upstream_to_downstream(
    addr: SocketAddr,
    ctx: mpsc::Sender<SocketAddr>,
    downstream: Arc<UdpSocket>,
    upstream: Arc<UdpProxyConn>,
) {
    let mut buffer = [0u8; u16::MAX as usize];

    loop {
        let read_bytes = match upstream.upstream_conn.recv(&mut buffer).await {
            Ok(read) => read,
            Err(why) => {
                log::error!("{why}");
                ctx.send(addr).await.ok();
                return;
            }
        };
        if let Err(why) = downstream.send_to(&buffer[..read_bytes], addr).await {
            log::error!("{why}");
            ctx.send(addr).await.ok();
            return;
        }

        upstream
            .last_activity
            .fetch_add(1, std::sync::atomic::Ordering::SeqCst);
    }
}

async fn udp_close_after_inactivity(
    addr: SocketAddr,
    close_after: std::time::Duration,
    ctx: mpsc::Sender<SocketAddr>,
    upstream: Arc<UdpProxyConn>,
) {
    loop {
        let last_activity = upstream
            .last_activity
            .load(std::sync::atomic::Ordering::SeqCst);
        tokio::time::sleep(close_after).await;
        if upstream
            .last_activity
            .load(std::sync::atomic::Ordering::SeqCst)
            == last_activity
        {
            break;
        }
    }

    if let Err(why) = ctx.send(addr).await {
        log::error!("{why}");
    }
}
