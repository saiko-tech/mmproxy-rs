use crate::{
    args::Args,
    pipe::{splice, wouldblock, Pipe, PIPE_BUF_SIZE},
    util,
};

use std::{io, net::SocketAddr, os::fd::AsRawFd};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt, Interest},
    net::{
        tcp::{ReadHalf, WriteHalf},
        TcpSocket, TcpStream,
    },
};

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
    mut src: TcpStream,
    addr: SocketAddr,
    mark: u32,
    ipv4_fwd: SocketAddr,
    ipv6_fwd: SocketAddr,
) -> io::Result<()> {
    src.set_nodelay(true)?;
    let mut buffer = [0u8; u16::MAX as usize];
    let read_bytes = src.read(&mut buffer).await?;

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

    let mut dst = util::tcp_create_upstream_conn(src_addr, target_addr, mark).await?;
    tokio::io::copy_buf(&mut rest, &mut dst).await?;

    let (mut sr, mut sw) = src.split();
    let (mut dr, mut dw) = dst.split();

    let src_to_dst = async {
        splice_copy(&mut sr, &mut dw).await?;
        dw.shutdown().await
    };
    let dst_to_src = async {
        splice_copy(&mut dr, &mut sw).await?;
        sw.shutdown().await
    };

    tokio::try_join!(src_to_dst, dst_to_src).map(|_| ())
}

// wait for src to be readable
// splice from src to the pipe buffer
// wait for dst to be writable
// splice to dst from the pipe buffer
async fn splice_copy(src: &mut ReadHalf<'_>, dst: &mut WriteHalf<'_>) -> io::Result<()> {
    use std::io::{Error, ErrorKind::WouldBlock};

    let pipe = Pipe::new()?;
    let mut size = 0;
    let mut done = false;

    let src = src.as_ref();
    let dst = dst.as_ref();
    let src_fd = src.as_raw_fd();
    let dst_fd = dst.as_raw_fd();

    'LOOP: while !done {
        src.readable().await?;
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
                break 'LOOP;
            }
        }

        dst.writable().await?;
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
                break 'LOOP;
            }
        }
    }

    if done {
        Ok(())
    } else {
        Err(Error::last_os_error())
    }
}
