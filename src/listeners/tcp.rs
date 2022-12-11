use crate::args::Args;
use crate::util::check_origin_allowed;

use std::io;
use std::sync::Arc;

use cidr::IpCidr;
use tokio::net::{TcpSocket, TcpStream};

pub async fn listen(args: Args) -> io::Result<()> {
    // TODO: using sync::Arc might have cause performance penalties
    let args = Arc::new(args);
    let socket = TcpSocket::new_v4()?;

    if args.listeners > 1 {
        socket.set_reuseport(true)?;
    }
    // might want to remove this
    socket.set_reuseaddr(true)?;
    socket.bind(args.listen_addr)?;

    let listener = socket.listen(args.listeners)?;
    log::info!("listening on: {}", listener.local_addr()?);

    loop {
        let (conn, addr) = listener.accept().await?;

        if let Some(ref allowed_subnets) = args.allowed_subnets {
            let ip_addr = addr.ip();

            if !check_origin_allowed(&ip_addr, allowed_subnets) {
                log::debug!("connection origin is not allowed");
                continue;
            }
        }

        let cargs = args.clone();
        tokio::spawn(async move {
            tcp_handle_connection(conn, cargs).await;
        });
    }
}

// TODO: figure out how to do proper error handling for this function
async fn tcp_handle_connection(conn: TcpStream, args: Arc<Args>) {}
