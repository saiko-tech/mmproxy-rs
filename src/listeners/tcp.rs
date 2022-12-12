use std::{io, net::SocketAddr};

use crate::args::Args;
use crate::util::check_origin_allowed;

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
        log::info!("new connection: {addr}");

        if let Some(ref allowed_subnets) = args.allowed_subnets {
            let ip_addr = addr.ip();

            if !check_origin_allowed(&ip_addr, allowed_subnets) {
                log::debug!("connection origin is not allowed");
                continue;
            }
        }

        let args_clone = args.clone();
        tokio::spawn(async move {
            if let Err(err) = tcp_handle_connection(conn, args_clone).await {
                log::error!("{err}");
            }
        });
    }
}

async fn tcp_handle_connection(conn: TcpStream, args: Args) -> io::Result<()> {
    let mut buffer = [0u8; u16::MAX as usize];
    let read_bytes = conn.try_read(&mut buffer)?;

    let (src, dst, rest) = crate::util::parse_proxy_protocol_header(&buffer[..read_bytes])?;
    dbg!(src, dst, rest);

    Ok(())
}
