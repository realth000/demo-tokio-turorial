use std::{
    io::{self, Cursor},
    sync::{Arc, Mutex},
};

use bytes::{Buf, BytesMut};
use futures::executor::block_on;
use mini_redis::{Frame, Result};
use tokio::{
    io::{AsyncReadExt, AsyncWriteExt},
    net::TcpStream,
};

pub struct Connection {
    inner: Arc<Mutex<ConnectionInner>>,
}

impl Connection {
    pub fn new(stream: TcpStream) -> Self {
        Self {
            inner: Arc::new(Mutex::new(ConnectionInner::new(stream))),
        }
    }

    pub fn read_frame(&mut self) -> Result<Option<Frame>> {
        println!(">>> read frame: waiting !");
        let mut lock = self.inner.lock().unwrap();
        println!(">>> read frame: running !");
        block_on(lock.read_frame())
    }

    pub fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        println!(">>> write frame: waiting !");
        let mut lock = self.inner.lock().unwrap();
        println!(">>> write frame: running !");
        block_on(lock.write_frame(frame))
    }
}

struct ConnectionInner {
    stream: TcpStream,
    buffer: BytesMut,
    cursor: usize,
}

impl ConnectionInner {
    fn new(stream: TcpStream) -> Self {
        Self {
            stream: stream,
            buffer: BytesMut::with_capacity(1024),
            cursor: 0,
        }
    }

    async fn read_frame(&mut self) -> Result<Option<Frame>> {
        loop {
            println!(">>> read_frame: parse frame");
            if let Some(frame) = self.parse_frame()? {
                println!(">>> read_frame: got frame");
                // Enough data for a frame, so the frame returned.
                return Ok(Some(frame));
            }

            if self.buffer.len() == self.cursor {
                // Buffer is full, grow up.
                self.buffer.resize(self.cursor * 2, 0);
                println!(">>> read_frame: resize buffer to {}", self.cursor * 2);
            }

            println!(">>> read_frame: read stream ready");
            let n = self.stream.read(&mut self.buffer[self.cursor..]).await?;
            println!(">>> read_frame: read stream done: n={n}");

            if n == 0 {
                if self.cursor == 0 {
                    println!(">>> read_frame: zero cursor, done");
                    // No more data in buffer, try read more data from stream to buffer.
                    return Ok(None);
                } else {
                    println!(">>> read_frame: have data");
                    // Still sending frame but unexpected close by peer.
                    return Err("connection reset by peer".into());
                }
            } else {
                self.cursor += n;
            }
        }
    }

    pub async fn write_frame(&mut self, frame: &Frame) -> io::Result<()> {
        match frame {
            Frame::Simple(v) => {
                self.stream.write_u8(b'+').await?;
                self.stream.write_all(v.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Error(v) => {
                self.stream.write_u8(b'-').await?;
                self.stream.write_all(v.as_bytes()).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Integer(v) => {
                self.stream.write_u8(b':').await?;
                self.write_decimal(*v).await?;
            }
            Frame::Bulk(v) => {
                let len = v.len();

                self.stream.write_u8(b'$').await?;
                self.write_decimal(len as u64).await?;
                self.stream.write_all(v).await?;
                self.stream.write_all(b"\r\n").await?;
            }
            Frame::Null => self.stream.write_all(b"$-1\r\n").await?,
            Frame::Array(_) => {
                panic!("writing arraies are not implemented")
            }
        }

        if let Err(e) = self.stream.flush().await {
            eprintln!("failed to flush stream: {e:?}");
        }

        Ok(())
    }

    /// Return a frame if data in buffer contains an entire frame.
    fn parse_frame(&mut self) -> Result<Option<Frame>> {
        let mut buf = Cursor::new(&self.buffer[..]);
        match Frame::check(&mut buf) {
            Ok(_) => {
                let len = buf.position() as usize;
                buf.set_position(0);
                let frame = Frame::parse(&mut buf)?;
                self.buffer.advance(len);
                Ok(Some(frame))
            }
            Err(mini_redis::frame::Error::Incomplete) => Ok(None),
            Err(e) => Err(e.into()),
        }
    }

    // Copied
    /// Write a decimal frame to the stream
    async fn write_decimal(&mut self, val: u64) -> io::Result<()> {
        use std::io::Write;

        // Convert the value to a string
        let mut buf = [0u8; 12];
        let mut buf = Cursor::new(&mut buf[..]);
        write!(&mut buf, "{}", val)?;

        let pos = buf.position() as usize;
        self.stream.write_all(&buf.get_ref()[..pos]).await?;
        self.stream.write_all(b"\r\n").await?;

        Ok(())
    }
}
