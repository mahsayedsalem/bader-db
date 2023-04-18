use anyhow::Result;
use bytes::BytesMut;
use std::sync::Arc;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpStream};
use crate::resp::value::Value;
use crate::cache::Cache;

#[derive(Debug)]
pub struct Handler {
    stream: TcpStream,
    buffer: BytesMut,
}

impl Handler {
    pub fn new(stream: TcpStream) -> Self {
        return Self {
            stream,
            buffer: BytesMut::with_capacity(512),
        };
    }

    pub async fn read_value(&mut self) -> Result<Option<Value>> {
        loop {
            let bytes_read = self.stream.read_buf(&mut self.buffer).await?;
            // Connection closed
            if bytes_read == 0 {
                return Ok(None);
            }
            let value = Value::from(&self.buffer);
            return Ok(Some(value));
        }
    }

    pub async fn get_response(value: Value, client_store: &Arc<Cache>) -> Result<Value> {
        let (command, args) = value.to_command()?;
        let response = match command.to_ascii_lowercase().as_ref() {
            "ping" => Value::SimpleString("PONG".to_string()),
            "echo" => args.first().unwrap().clone(),
            "get" => {
                if let Some(Value::BulkString(key)) = args.get(0) {
                    if let Some(value) = client_store.get(key.clone()).await {
                        Value::SimpleString(value)
                    } else {
                        Value::Null
                    }
                } else {
                    Value::Error("Get requires one argument".to_string())
                }
            }
            "set" => {
                if let (Some(Value::BulkString(key)), Some(Value::BulkString(value))) =
                    (args.get(0), args.get(1))
                {
                    // TODO commands for different expiration types
                    if let (Some(Value::BulkString(_)), Some(Value::BulkString(amount))) =
                        (args.get(2), args.get(3))
                    {
                        client_store.set_with_expiry(
                            key.clone(),
                            value.clone(),
                            amount.parse::<u64>()?,
                        ).await;
                    } else {
                        client_store.set(key.clone(), value.clone()).await;
                    }
                    Value::SimpleString("OK".to_string())
                } else {
                    Value::Error("Set requires two or four arguments".to_string())
                }
            }
            _ => Value::Error(format!("command not implemented: {}", command)),
        };
        Ok(response)
    }

    pub async fn write_value(&mut self, value: Value) -> Result<()> {
        self.stream.write(value.encode().as_bytes()).await?;
        Ok(())
    }
}
