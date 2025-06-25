use std::sync::{Arc, Mutex};

use demo_async::Connection;
use mini_redis::Frame;
use tokio::{io, net::TcpStream};

#[tokio::main]
async fn main() -> io::Result<()> {
    let socket = TcpStream::connect("127.0.0.1:6379").await?;

    let conn = Arc::new(Mutex::new(Connection::new(socket)));
    let conn2 = conn.clone();

    let sender = tokio::spawn(async move {
        {
            println!("send hello frame to server");
            let mut c = conn.lock().unwrap();
            if let Err(e) =
                c.write_frame(&Frame::Bulk("hello from connection version client".into()))
            {
                eprintln!("failed to write hello frame: {e:?}");
                return;
            }
            if let Err(e) = c.write_frame(&Frame::Integer(100)) {
                eprintln!("failed to write integer frame: {e:?}");
                return;
            }
            println!("sender finished");
        }

        let mut c = conn2.try_lock().unwrap();
        let mut read_count = 2;
        loop {
            read_count -= 1;
            if read_count < 0 {
                break;
            }

            match c.read_frame() {
                Ok(frame) => match frame {
                    Some(frame) => {
                        println!("got echo frame: {frame:?}");
                    }
                    None => {
                        println!("no more data to get echo");
                        return;
                    }
                },
                Err(e) => {
                    eprintln!("error in echo reply : {e:?}");
                    return;
                }
            }
        }
    });

    sender.await?;

    Ok(())
}
