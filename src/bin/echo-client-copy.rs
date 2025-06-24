use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let mut socket = TcpStream::connect("127.0.0.1:6142").await?;

    let t1 = tokio::spawn(async move {
        // `TcpStream::split` has no Arc or Mutex runtime overhead, but it requires
        // the splitted `rd` and `wr` run in the same thread.
        let (mut rd, mut wr) = socket.split();
        wr.write_all(b"hello\n").await?;
        wr.write_all(b"world\n").await?;

        let mut buf = vec![0; 128];

        loop {
            // A zero size means the write half of tcp stream is closed.
            let n = rd.read(&mut buf).await?;
            if n == 0 {
                break;
            }

            println!("got echo: {:?}", &buf[..n]);
        }

        Ok::<_, io::Error>(())
    });

    if let Err(e) = t1.await {
        eprintln!("failed in t1 worker: {e:?}");
    }

    Ok(())
}
