use crate::{args::Args, util};
use socket2::SockRef;
use std::{
    collections::HashMap,
    io,
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
    pub upstream_conn: UdpSocket,
    pub last_activity: AtomicU64,
}

impl UdpProxyConn {
    fn new(upstream_conn: UdpSocket) -> Self {
        Self {
            upstream_conn,
            last_activity: AtomicU64::new(0),
        }
    }
}

pub async fn listen(args: Args) -> io::Result<()> {
    let socket = {
        let socket = UdpSocket::bind(args.listen_addr).await?;
        let sock_ref = SockRef::from(&socket);
        sock_ref.set_reuse_port(args.listeners > 1)?;
        Arc::new(socket)
    };
    log::info!("listening on: {}", socket.local_addr()?);

    let mut buffer = [0u8; MAX_DGRAM_SIZE];
    let mut connections = ConnectionsHashMap::new();
    let (conn_tx, mut conn_rx) = mpsc::channel::<SocketAddr>(128);

    loop {
        tokio::select! {
            // close inactive connections in this branch
            addr = conn_rx.recv() => {
                if let Some(addr) = addr {
                    if let Some((_conn, handle)) = connections.remove(&addr) {
                        log::info!("closing {addr} due inactivity");
                        if !handle.is_finished() {
                            handle.abort();
                        }
                    }
                }
            }
            // handle incoming DGRAM packets in this branch
            ret = socket.recv_from(&mut buffer) => {
                let (read, addr) = ret?;

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
                    conn_tx.clone(),
                )
                .await
                {
                    log::error!("while handling connection: {why}");
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
    connections: &mut ConnectionsHashMap,
    conn_tx: mpsc::Sender<SocketAddr>,
) -> io::Result<()> {
    let (src_addr, rest, version) = match util::parse_proxy_protocol_header(buffer) {
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
            if src_addr == addr {
                log::debug!("unknown source, using the downstream connection address");
            }
            log::info!("[new conn] [origin: {addr}] [src: {src_addr}]");

            let proxy_conn =
                match util::udp_create_upstream_conn(src_addr, target_addr, args.mark).await {
                    Ok(sock) => Arc::new(UdpProxyConn::new(sock)),
                    Err(err) => {
                        log::error!("failed to create an upstream connection to {target_addr}");
                        return Err(err);
                    }
                };

            let sock_clone = socket.clone();
            let proxy_clone1 = proxy_conn.clone();

            let close_after = args.close_after;
            let handle = tokio::spawn(async move {
                if let Err(why) =
                    udp_copy_upstream_to_downstream(addr, src_addr, sock_clone, proxy_clone1).await
                {
                    log::error!("while copying from upstream to downstream [{src_addr}]: {why}");
                };
            });
            tokio::spawn(udp_close_after_inactivity(
                addr,
                close_after,
                conn_tx.clone(),
                proxy_conn.clone(),
            ));

            connections.insert(addr, (proxy_conn.clone(), handle));
            proxy_conn
        }
        Some((proxy_conn, _handle)) => {
            proxy_conn.last_activity.fetch_add(1, Ordering::SeqCst);
            proxy_conn.clone()
        }
    };

    match proxy_conn.upstream_conn.send(rest).await {
        Ok(size) => {
            log::debug!("from [{}] to [{}], size: {}", src_addr, addr, size);
            Ok(())
        }
        Err(err) => {
            log::error!("failed to write data to upstream connection");
            Err(err)
        }
    }
}

async fn udp_copy_upstream_to_downstream(
    addr: SocketAddr,
    src_addr: SocketAddr,
    downstream: Arc<UdpSocket>,
    proxy_conn: Arc<UdpProxyConn>,
) -> io::Result<()> {
    let mut buffer = [0u8; MAX_DGRAM_SIZE];

    loop {
        let read_bytes = proxy_conn.upstream_conn.recv(&mut buffer).await?;
        let sent_bytes = downstream.send_to(&buffer[..read_bytes], addr).await?;
        if sent_bytes == 0 {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                "couldn't sent anything to downstream",
            ));
        }
        log::debug!("from [{}] to [{}], size: {}", addr, src_addr, sent_bytes);

        proxy_conn.last_activity.fetch_add(1, Ordering::SeqCst);
    }
}

async fn udp_close_after_inactivity(
    addr: SocketAddr,
    close_after: Duration,
    conn_tx: mpsc::Sender<SocketAddr>,
    proxy_conn: Arc<UdpProxyConn>,
) {
    let mut last_activity = proxy_conn.last_activity.load(Ordering::SeqCst);
    loop {
        tokio::time::sleep(close_after).await;
        if proxy_conn.last_activity.load(Ordering::SeqCst) == last_activity {
            break;
        }
        last_activity = proxy_conn.last_activity.load(Ordering::SeqCst);
    }

    if let Err(why) = conn_tx.send(addr).await {
        log::error!("couldn't send the close command to conn channel: {why}");
    }
}
