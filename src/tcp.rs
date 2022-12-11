use std::io;
use tokio::net::TcpListener;

// TODO: set so_reuseaddr if number of listeners are going to be
// more than 1
pub async fn listen(args: &crate::args::Args) -> io::Result<()> {
    let listener = TcpListener::bind(&args.listen).await?;
    println!("nice");

    loop {
        let (sock, addr) = listener.accept().await?;
        println!("{addr}");
    }
}
