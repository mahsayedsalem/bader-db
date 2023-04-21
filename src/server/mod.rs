mod handler;
mod connection;
pub mod shutdown;

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
use crate::server::connection::Connection;


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
                None,
                None
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
            match incoming {
                Ok((s, addr)) => {
                    let client_cache = self.main_cache.clone();
                    let mut handler = Handler::new(client_cache,
                                                   Some(Connection::new(s)), None, None);
                    tokio::spawn(async move {
                        handler.handle_connection().await;
                    });
                },
                Err(e) => {
                    log::error!("error: {:?}", e);
                }
            }
        }
    }
}
