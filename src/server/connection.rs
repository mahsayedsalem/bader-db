use anyhow::Result;
use bytes::BytesMut;
use tokio::io::{AsyncReadExt, AsyncWriteExt};
use tokio::net::TcpStream;

use crate::resp::value::Value;

#[derive(Debug)]
pub struct Connection {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Connection {
    pub fn new(socket: TcpStream) -> Connection {
        Connection {
            stream: socket,
            buffer: BytesMut::with_capacity(512),
        }
    }

    pub async fn read_value(&mut self) -> Result<Option<Value>> {
        let bytes_read = self.stream.read_buf(&mut self.buffer).await?;

        // Connection closed
        if bytes_read == 0 {
            return Ok(None);
        }

        let value = Value::from(&mut self.buffer.clone());
        return Ok(Some(value));
    }

    pub async fn write_value(&mut self, value: Value) {
        _ = self.stream.write(value.encode().as_bytes()).await;
    }
}
