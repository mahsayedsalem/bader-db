use anyhow::Result;
use std::sync::Arc;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpStream, net::TcpListener, sync::broadcast, sync::mpsc};
use crate::resp::value::Value;
use crate::cache::expiry::ExpiryFormat;
use crate::cache::Cache;
use bytes::BytesMut;
use crate::server::shutdown::Shutdown;
use crate::server::connection::Connection;

#[derive(Debug)]
pub struct Handler {
    store: Arc<Cache>,
    connection: Connection,
    shutdown: Shutdown,
    _shutdown_complete: mpsc::Sender<()>,
}

impl Handler {
    pub fn new(store: Arc<Cache>, connection: Connection, shutdown: Shutdown, _shutdown_complete: mpsc::Sender<()>) -> Self {
        return Self {
            store,
            connection,
            shutdown,
            _shutdown_complete
        };
    }

    pub async fn handle_connection(&mut self) {

        log::info!("Received connection");

        while !self.shutdown.is_shutdown()  {

            match self.connection.read_value().await {
                Ok(value) => {
                    if let Some(v) = value {
                        match self.handle_request(v).await {
                            Ok(response) => {
                                log::debug!("response: {:?}", response);
                                self.connection.write_value(response);
                            }
                            Err(e) => {
                                log::error!("error: {:?}", e);
                                break;
                            }
                        }
                    } else {
                        break;
                    }
                }
                Err(e) => {
                    log::error!("error: {:?}", e);
                    break;
                }
            }
        }
    }


    pub async fn handle_request(&mut self, value: Value) -> Result<Value> {
        let (first_arg, args) = value.to_command()?;
        let command = first_arg.to_ascii_lowercase().as_str().into();
        let response = match command {
            Command::PING => Value::SimpleString("PONG".to_string()),
            Command::ECHO=> args.first().unwrap().clone(),
            Command::Get => self.handle_get(&args).await,
            Command::SET => self.handle_set(&args).await,
            Command::DELETE => self.handle_delete(&args).await,
            Command::EXISTS => self.handle_exists(&args).await,
            _ => Value::Error(format!("command not implemented: {}", first_arg)),
        };
        Ok(response)
    }

    async fn handle_get(&self, args: &Vec<Value>) -> Value {
        if let Some(Value::BulkString(key)) = args.get(0) {
            if let Some(value) = self.store.get(key.clone()).await {
                Value::SimpleString(value)
            } else {
                Value::Null
            }
        } else {
            Value::Error("GET requires one argument".to_string())
        }
    }

    async fn handle_set(&self, args: &Vec<Value>) -> Value {
        if let (Some(Value::BulkString(key)), Some(Value::BulkString(value))) =
            (args.get(0), args.get(1))
        {
            if let (Some(Value::BulkString(expiry_format)), Some(Value::BulkString(amount))) =
                (args.get(2), args.get(3))
            {
                let e = ExpiryFormat::from(expiry_format.as_str());
                if e != ExpiryFormat::Uninitialized {
                    self.handle_set_with_expiry(key, value, amount, Some(expiry_format)).await
                } else {
                    self.handle_set_with_expiry(key, value, amount, None).await
                }
            } else {
                self.store.set(key.clone(), value.clone()).await;
                Value::SimpleString("OK".to_string())
            }
        } else {
            Value::Error("SET requires two or four arguments".to_string())
        }
    }

    async fn handle_set_with_expiry(&self, key: &String, value: &String, amount: &String, expiry_format: Option<&String>) -> Value {
        if let Ok(amount) = amount.parse::<u64>() {

            match expiry_format {
                Some(e) => {
                    self.store.set_with_expiry(
                    key.clone(),
                    value.clone(),
                    (amount, e),
                ).await;
                }
                _ => {
                    self.store.set_with_expiry(
                        key.clone(),
                        value.clone(),
                        amount,
                    ).await;
                }

            }
            Value::SimpleString("OK".to_string())
        } else {
            Value::Error("Unsupported expiry format".to_string())
        }
    }

    async fn handle_delete(&self, args: &Vec<Value>) -> Value {
        if let Some(Value::BulkString(key)) = args.get(0) {
            match self.store.remove(key.clone()).await {
                Ok(e) => Value::SimpleString("OK".to_string()),
                Err(e) => Value::Error(format!("Error while deleting: {:?}", e))
            }
        } else {
            Value::Error("DEL requires one argument".to_string())
        }
    }

    async fn handle_exists(&self, args: &Vec<Value>) -> Value {
        if let Some(Value::BulkString(key)) = args.get(0) {
            match self.store.exists(key.clone()).await {
                true => Value::SimpleString("true".to_string()),
                false => Value::SimpleString("false".to_string()),
            }
        } else {
            Value::Error("EXISTS requires one argument".to_string())
        }
    }

}

#[derive(Debug, PartialEq)]
pub enum Command {
    PING,
    ECHO,
    Get,
    SET,
    DELETE,
    EXISTS,
    Uninitialized,
}

impl From<&str> for Command {
    fn from(s: &str) -> Self {
        match s {
            "ping" => Command::PING,
            "echo" => Command::ECHO,
            "get" => Command::Get,
            "set" => Command::SET,
            "del" => Command::DELETE,
            "exists" => Command::EXISTS,
            _ => Command::Uninitialized,
        }
    }
}


