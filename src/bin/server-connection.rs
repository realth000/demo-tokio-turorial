use demo_async::Connection;
use tokio::{
    io::{self},
    net::TcpListener,
};

#[tokio::main]
async fn main() -> io::Result<()> {
    let listener = TcpListener::bind("127.0.0.1:6379").await?;

    let mut work_rounds = 0;

    loop {
        work_rounds += 1;
        if work_rounds > 100 {
            println!("time to take a rest");
            break;
        }

        // Use Connection.
        let (socket, addr) = listener.accept().await?;
        tokio::spawn(async move {
            println!("accept addr: {:?}", addr);

            let mut conn = Connection::new(socket);

            loop {
                match conn.read_frame() {
                    Ok(maybe_frame) => {
                        match maybe_frame {
                            Some(frame) => {
                                // Have a frame, write back.
                                println!("[{addr:?}] echo frame back: {frame:?}");
                                if let Err(e) = conn.write_frame(&frame) {
                                    eprintln!("[{addr:?}] failed to echo back: {e:?}");
                                    return;
                                }
                            }
                            None => {
                                println!("[{addr:?}] work is done");
                                return;
                            }
                        }
                    }
                    Err(e) => {
                        eprintln!("[{addr:?}] failed to read frame from connection: {e:?}");
                        return;
                    }
                }
            }
        });
    }
    Ok(())
}
