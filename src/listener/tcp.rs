use simple_eyre::eyre::{Result, WrapErr};

use crate::{
    args::Args,
    pipe::{splice, wouldblock, Pipe, PIPE_BUF_SIZE},
    util,
};

use std::{net::SocketAddr, os::fd::AsRawFd};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpSocket, TcpStream,
    },
};

pub async fn listen(args: Args) -> Result<()> {
    let socket = match args.listen_addr {
        SocketAddr::V4(_) => TcpSocket::new_v4(),
        SocketAddr::V6(_) => TcpSocket::new_v6(),
    };
    let socket = socket.wrap_err("failed to create socket")?;

    socket
        .set_reuseport(args.listeners > 1)
        .wrap_err("failed to set reuseport")?;
    socket
        .set_reuseaddr(true)
        .wrap_err("failed to set reuseaddr")?;
    socket
        .bind(args.listen_addr)
        .wrap_err_with(|| format!("failed to bind to {}", args.listen_addr))?;

    let listener = socket
        .listen(args.listeners)
        .wrap_err("failed to start the listener")?;

    log::info!("listening on: {}", args.listen_addr);
    loop {
        let (conn, addr) = listener
            .accept()
            .await
            .wrap_err("failed to accept connection")?;

        if let Some(ref allowed_subnets) = args.allowed_subnets {
            let ip_addr = addr.ip();

            if !util::check_origin_allowed(&ip_addr, allowed_subnets) {
                log::warn!("connection origin is not allowed: {ip_addr}");
                continue;
            }
        }

        let mark = args.mark;
        let ipv4_fwd = args.ipv4_fwd;
        let ipv6_fwd = args.ipv6_fwd;

        tokio::spawn(async move {
            if let Err(err) = tcp_handle_connection(conn, addr, mark, ipv4_fwd, ipv6_fwd).await {
                log::error!("{err:#}");
            }
        });
    }
}

async fn tcp_handle_connection(
    mut src: TcpStream,
    addr: SocketAddr,
    mark: u32,
    ipv4_fwd: SocketAddr,
    ipv6_fwd: SocketAddr,
) -> Result<()> {
    src.set_nodelay(true)
        .wrap_err_with(|| format!("failed to set nodelay on {addr} socket"))?;

    let mut buffer = [0u8; u16::MAX as usize];
    let read_bytes = src
        .read(&mut buffer)
        .await
        .wrap_err_with(|| format!("failed to read the initial proxy-protocol header on {addr}"))?;

    let (addr_pair, mut rest, _version) = util::parse_proxy_protocol_header(&buffer[..read_bytes])
        .wrap_err("failed to parse the proxy protocol header")?;

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
    log::info!("[new conn] [origin: {addr}] [src: {src_addr}]");

    let mut dst = util::tcp_create_upstream_conn(src_addr, target_addr, mark).await?;
    tokio::io::copy_buf(&mut rest, &mut dst)
        .await
        .wrap_err("failed to re-transmit rest of the initial tcp packet")?;

    let (mut sr, mut sw) = src.split();
    let (mut dr, mut dw) = dst.split();

    let src_to_dst = async {
        splice_copy(&mut sr, &mut dw).await?;
        dw.shutdown()
            .await
            .wrap_err("failed to shutdown the dst writer")
    };
    let dst_to_src = async {
        splice_copy(&mut dr, &mut sw).await?;
        sw.shutdown()
            .await
            .wrap_err("failed to shutdown the src writer")
    };

    tokio::try_join!(src_to_dst, dst_to_src)
        // discard the `Ok(_)` value as it's useless
        .map(|_| ())
}

// wait for src to be readable
// splice from src to the pipe buffer
// wait for dst to be writable
// splice to dst from the pipe buffer
async fn splice_copy(src: &mut ReadHalf<'_>, dst: &mut WriteHalf<'_>) -> Result<()> {
    use std::io::{Error, ErrorKind::WouldBlock};

    let pipe = Pipe::new().wrap_err("failed to create pipe")?;
    // number of bytes that the pipe buffer is currently holding
    let mut size = 0;
    let mut done = false;

    let src = src.as_ref();
    let dst = dst.as_ref();
    let src_fd = src.as_raw_fd();
    let dst_fd = dst.as_raw_fd();

    while !done {
        src.readable()
            .await
            .wrap_err("awaiting on readable failed")?;
        let ret = src.try_io(Interest::READABLE, || {
            while size < PIPE_BUF_SIZE {
                match splice(src_fd, pipe.w, PIPE_BUF_SIZE - size) {
                    r if r > 0 => size += r as usize,
                    r if r == 0 => {
                        done = true;
                        break;
                    }
                    r if r < 0 && wouldblock() => {
                        return Err(Error::new(WouldBlock, "EWOULDBLOCK"))
                    }
                    _ => return Err(Error::last_os_error()),
                }
            }
            Ok(())
        });
        if let Err(err) = ret {
            if err.kind() != WouldBlock {
                break;
            }
        }

        dst.writable()
            .await
            .wrap_err("awaiting on writable failed")?;
        let ret = dst.try_io(Interest::WRITABLE, || {
            while size > 0 {
                match splice(pipe.r, dst_fd, size) {
                    r if r > 0 => size -= r as usize,
                    r if r < 0 && wouldblock() => {
                        return Err(Error::new(WouldBlock, "EWOULDBLOCK"))
                    }
                    _ => return Err(Error::last_os_error()),
                }
            }
            Ok(())
        });
        if let Err(err) = ret {
            if err.kind() != WouldBlock {
                break;
            }
        }
    }

    if done {
        Ok(())
    } else {
        Err(Error::last_os_error().into())
    }
}
