use simple_eyre::eyre::{eyre, Result, WrapErr};

use crate::{args::Args, util};
use socket2::SockRef;
use std::{
    collections::HashMap,
    net::SocketAddr,
    sync::{
        atomic::{AtomicU64, Ordering},
        Arc,
    },
    time::Duration,
};
use tokio::{net::UdpSocket, sync::mpsc, task::JoinHandle};

const MAX_DGRAM_SIZE: usize = 65_507;
type ConnectionsHashMap = HashMap<SocketAddr, (Arc<UdpProxyConn>, JoinHandle<()>)>;

#[derive(Debug)]
struct UdpProxyConn {
    pub sock: UdpSocket,
    pub last_activity: AtomicU64,
}

impl UdpProxyConn {
    fn new(sock: UdpSocket) -> Self {
        Self {
            sock,
            last_activity: AtomicU64::new(0),
        }
    }
}

pub async fn listen(args: Args) -> Result<()> {
    let socket = {
        let socket = UdpSocket::bind(args.listen_addr)
            .await
            .wrap_err_with(|| format!("failed to bind to {}", args.listen_addr))?;

        let sock_ref = SockRef::from(&socket);
        sock_ref
            .set_reuse_port(args.listeners > 1)
            .wrap_err("failed to set reuse port on listener socket")?;

        Arc::new(socket)
    };

    let mut buffer = [0u8; MAX_DGRAM_SIZE];
    let mut connections = ConnectionsHashMap::new();
    let (tx, mut rx) = mpsc::channel::<SocketAddr>(128);

    log::info!("listening on: {}", args.listen_addr);
    loop {
        tokio::select! {
            // close inactive connections in this branch
            addr = rx.recv() => {
                if let Some(addr) = addr {
                    if let Some((_conn, handle)) = connections.remove(&addr) {
                        log::info!("closing {addr} due to inactivity");
                        handle.abort();
                    }
                }
            }
            // handle incoming DGRAM packets in this branch
            ret = socket.recv_from(&mut buffer) => {
                let (read, addr) = ret.wrap_err("failed to accept connection")?;

                if let Some(ref allowed_subnets) = args.allowed_subnets {
                    let ip_addr = addr.ip();

                    if !util::check_origin_allowed(&ip_addr, allowed_subnets) {
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
                    tx.clone(),
                )
                .await
                {
                    log::error!("{why:#}");
                }
            }
        }
    }
}

async fn udp_handle_connection(
    args: &Args,
    src: Arc<UdpSocket>,
    addr: SocketAddr,
    buffer: &mut [u8],
    connections: &mut ConnectionsHashMap,
    tx: mpsc::Sender<SocketAddr>,
) -> Result<()> {
    let (src_addr, rest, version) = match util::parse_proxy_protocol_header(buffer) {
        Ok((addr_pair, rest, version)) => match addr_pair {
            Some((src, _)) => (src, rest, version),
            None => (addr, rest, version),
        },
        Err(err) => return Err(err).wrap_err("failed to parse proxy protocol header"),
    };

    if version < 2 {
        return Err(eyre!(
            "proxy protocol version 1 doesn't support UDP connections"
        ));
    }
    let target_addr = match src_addr {
        SocketAddr::V4(_) => args.ipv4_fwd,
        SocketAddr::V6(_) => args.ipv6_fwd,
    };

    let dst = match connections.get(&addr) {
        Some((dst, _handle)) => {
            dst.last_activity.fetch_add(1, Ordering::SeqCst);
            dst.clone()
        }
        // first time connecting
        None => {
            if src_addr == addr {
                log::debug!("unknown source, using the downstream connection address");
            }
            log::info!("[new conn] [origin: {addr}] [src: {src_addr}]");

            let dst = {
                let sock = util::udp_create_upstream_conn(src_addr, target_addr, args.mark).await?;
                Arc::new(UdpProxyConn::new(sock))
            };

            let src_clone = src.clone();
            let dst_clone = dst.clone();
            let handle = tokio::spawn(async move {
                if let Err(why) = udp_dst_to_src(addr, src_addr, src_clone, dst_clone).await {
                    log::error!("{why:#}");
                };
            });
            tokio::spawn(udp_close_after_inactivity(
                addr,
                args.close_after,
                tx.clone(),
                dst.clone(),
            ));

            connections.insert(addr, (dst.clone(), handle));
            dst
        }
    };

    match dst.sock.send(rest).await {
        Ok(size) => {
            log::debug!("from [{}] to [{}], size: {}", src_addr, addr, size);
            Ok(())
        }
        Err(err) => Err(err).wrap_err("failed to write data to the upstream connection"),
    }
}

async fn udp_dst_to_src(
    addr: SocketAddr,
    src_addr: SocketAddr,
    src: Arc<UdpSocket>,
    dst: Arc<UdpProxyConn>,
) -> Result<()> {
    let mut buffer = [0u8; MAX_DGRAM_SIZE];

    loop {
        let read_bytes = dst.sock.recv(&mut buffer).await?;
        let sent_bytes = src.send_to(&buffer[..read_bytes], addr).await?;
        if sent_bytes == 0 {
            return Err(eyre!("couldn't sent anything to downstream"));
        }
        log::debug!("from [{}] to [{}], size: {}", addr, src_addr, sent_bytes);

        dst.last_activity.fetch_add(1, Ordering::SeqCst);
    }
}

async fn udp_close_after_inactivity(
    addr: SocketAddr,
    close_after: Duration,
    tx: mpsc::Sender<SocketAddr>,
    dst: Arc<UdpProxyConn>,
) {
    let mut last_activity = dst.last_activity.load(Ordering::SeqCst);
    loop {
        tokio::time::sleep(close_after).await;
        if dst.last_activity.load(Ordering::SeqCst) == last_activity {
            break;
        }
        last_activity = dst.last_activity.load(Ordering::SeqCst);
    }

    if let Err(why) = tx.send(addr).await {
        log::error!("couldn't send the close command to conn channel: {why}");
    }
}
