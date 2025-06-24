use tokio::{
    io::{self, AsyncReadExt, AsyncWriteExt},
    net::TcpListener,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6142").await?;

    loop {
        let (mut socket, addr) = listener.accept().await?;
        tokio::spawn(async move {
            println!("accept addr: {:?}", addr);

            // Approach 1:
            //
            // // We are writing data back into the socket by `io::copy`.
            // // Because `io::copy` requires mutable references on both reader and writer,
            // // and we only have one socket act both sides, two mutable references can not
            // // exists at the same time.
            // // To fix it , use `io::split` or `TcpStream::split`.
            // let (mut rd, mut wr) = socket.split();
            //
            // Use `io::copy` to handle the copy process automatically.
            // if io::copy(&mut rd, &mut wr).await.is_err() {
            //     eprintln!("failed to copy");
            // }

            // Approach 2:
            //
            // Manually read the data and write it back.
            let mut buf = vec![0; 1024];
            loop {
                match socket.read(&mut buf).await {
                    Ok(0) => {
                        println!("[client={addr:?}] read buffer ended, done");
                        return;
                    }
                    Ok(n) => {
                        println!("[client={addr:?}] echo back: : {:?}", &buf[..n]);
                        if let Err(e) = socket.write_all(&buf[..n]).await {
                            // Unexpected error.
                            // We can do nothing except terminate the works.
                            eprintln!("[client={addr:?}] ERROR: failed to echo back: {e:?}");
                            return;
                        }
                    }
                    Err(e) => {
                        eprintln!("[client={addr:?}] ERROR: failed to read data: {e:?}");
                        return;
                    }
                }
            }
        });
    }
    Ok(())
}
