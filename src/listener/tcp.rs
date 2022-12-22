use crate::{args::Args, util};
use std::{io, net::SocketAddr};
use tokio::net::{TcpSocket, TcpStream};

pub async fn listen(args: Args) -> io::Result<()> {
    let socket = match args.listen_addr {
        SocketAddr::V4(_) => TcpSocket::new_v4()?,
        SocketAddr::V6(_) => TcpSocket::new_v6()?,
    };

    socket.set_reuseport(args.listeners > 1)?;
    socket.set_reuseaddr(true)?;
    socket.bind(args.listen_addr)?;

    let listener = socket.listen(args.listeners)?;
    log::info!("listening on: {}", listener.local_addr()?);

    loop {
        let (conn, addr) = listener.accept().await?;
        if let Some(ref allowed_subnets) = args.allowed_subnets {
            let ip_addr = addr.ip();

            if !util::check_origin_allowed(&ip_addr, allowed_subnets) {
                log::debug!("connection origin is not allowed: {ip_addr}");
                continue;
            }
        }

        let mark = args.mark;
        let ipv4_fwd = args.ipv4_fwd;
        let ipv6_fwd = args.ipv6_fwd;

        tokio::spawn(async move {
            if let Err(err) = tcp_handle_connection(conn, addr, mark, ipv4_fwd, ipv6_fwd).await {
                log::error!("while handling a connection: {err}");
            }
        });
    }
}

async fn tcp_handle_connection(
    mut conn: TcpStream,
    addr: SocketAddr,
    mark: u32,
    ipv4_fwd: SocketAddr,
    ipv6_fwd: SocketAddr,
) -> io::Result<()> {
    let mut buffer = [0u8; u16::MAX as usize];
    conn.readable().await?;
    let read_bytes = conn.try_read(&mut buffer)?;

    let (addr_pair, mut rest, _version) = util::parse_proxy_protocol_header(&buffer[..read_bytes])?;
    let src_addr = match addr_pair {
        Some((src, _dst)) => src,
        None => {
            log::debug!("unknown source, using the downstream connection address");
            addr
        }
    };
    let target_addr = match src_addr {
        SocketAddr::V4(_) => ipv4_fwd,
        SocketAddr::V6(_) => ipv6_fwd,
    };
    log::info!("[new conn] [origin: {addr} [src: {src_addr}]");

    let mut upstream_conn = util::tcp_create_upstream_conn(src_addr, target_addr, mark).await?;
    conn.set_nodelay(true)?;

    tokio::io::copy_buf(&mut rest, &mut upstream_conn).await?;
    tokio::io::copy_bidirectional(&mut conn, &mut upstream_conn).await?;

    Ok(())
}
