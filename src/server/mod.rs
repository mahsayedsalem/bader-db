mod handler;

use std::net::SocketAddr;
use std::sync::Arc;
use std::str;
use std::time::Duration;
use bytes::BytesMut;
use anyhow::Result;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpStream, net::TcpListener};
use crate::server::handler::Handler;
use crate::resp::value::Value;
use crate::cache::Cache;


#[derive(Debug)]
pub struct Server<'a> {
    socket_addr: &'a str,
    main_cache: Arc<Cache>,
}

impl<'a> Server<'a> {
    pub fn new(socket_addr: &'a str, sample: usize, threshold: f64, frequency: Duration) -> Self {
        Server {
            socket_addr,
            main_cache: Arc::new(Cache::new(
                sample,
                threshold,
                frequency,
            ))
        }
    }

    pub async fn run(&self) {
        log::info!("{:?}", "Server is running");

        // Start a server listening on socket address
        let listener = TcpListener::bind(self.socket_addr).await.unwrap();
        log::info!("{:?} {:?}", "Server is running on", self.socket_addr);

        let clone = self.main_cache.clone();

        tokio::spawn(async move {
            clone.monitor_for_expiry().await
        });

        loop {
            let incoming = listener.accept().await;
            let client_cache = self.main_cache.clone();
            match incoming {
                Ok((s, addr)) => {
                    log::debug!("Accepted new connection");
                    tokio::spawn(async move {
                        Self::handle_connection(s, addr, client_cache).await;
                    });
                },
                Err(e) => {
                    log::error!("error: {:?}", e);
                }
            }
        }
    }

    pub async fn handle_connection(mut stream: TcpStream, addr: SocketAddr, client_store: Arc<Cache>) {

        log::info!("Received connection from {:?}", addr);

        loop {
            let mut buffer = BytesMut::with_capacity(512);

            match Self::read_value(&mut stream, &mut buffer).await {
                Ok(value) => {
                    if let Some(v) = value {
                        let mut handler = Handler::new(v);
                        let response: Value = handler.handle_request(&client_store).await.unwrap();

                        log::debug!("response: {:?}", response);
                        Self::write_value(&mut stream, response).await.unwrap();
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

    pub async fn read_value(stream: &mut TcpStream, buffer: &mut BytesMut) -> Result<Option<Value>> {
        loop {
            let bytes_read = stream.read_buf(buffer).await?;
            // Connection closed
            if bytes_read == 0 {
                return Ok(None);
            }
            let value = Value::from(&buffer.clone());
            return Ok(Some(value));
        }
    }

    pub async fn write_value(stream: &mut TcpStream, value: Value) -> Result<()> {
        stream.write(value.encode().as_bytes()).await?;
        Ok(())
    }
}
