use std::io;
use tokio::net::TcpSocket;

pub async fn listen(args: &crate::args::Args) -> io::Result<()> {
    // TODO: this bit should be shared between the udp listener
    let addr = match args.listen.parse() {
        Ok(addr) => addr,
        Err(why) => {
            return Err(io::Error::new(
                io::ErrorKind::Other,
                format!("couldn't parse the listen addresss: {why}"),
            ))
        }
    };
    let socket = TcpSocket::new_v4()?;

    if args.listeners > 1 {
        socket.set_reuseport(true)?;
    }
    // might want to remove this
    socket.set_reuseaddr(true)?;
    socket.bind(addr)?;

    // loop {
    //     let (sock, addr) = listener.accept().await?;
    //     println!("{addr}");
    // }

    Ok(())
}
