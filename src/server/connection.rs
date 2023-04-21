use bytes::{Buf, BytesMut};
use tokio::io::{AsyncReadExt, AsyncWriteExt, BufWriter};
use tokio::net::TcpStream;
use anyhow::Result;

use crate::resp::value::Value;

#[derive(Debug)]
pub struct Connection {
    stream: BufWriter<TcpStream>,
    buffer: BytesMut,
}


impl Connection {

    pub fn new(socket: TcpStream) -> Connection {
        Connection {
            stream: BufWriter::new(socket),
            buffer: BytesMut::with_capacity(4 * 1024),
        }
    }

    pub async fn read_value(&mut self) -> Result<Option<Value>> {
        loop {
            let bytes_read = self.stream.read_buf(&mut self.buffer).await?;

            // Connection closed
            if bytes_read == 0 {
                return Ok(None);
            }

            let value = Value::from(&mut self.buffer.clone());
            return Ok(Some(value));
        }
    }

    pub async fn write_value(&mut self, value: Value) -> Result<()> {
        self.stream.write(value.encode().as_bytes()).await?;
        Ok(())
    }

}