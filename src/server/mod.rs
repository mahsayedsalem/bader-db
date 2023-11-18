mod connection;
mod handler;
pub mod shutdown;

use crate::cache::Cache;
use crate::server::{connection::Connection, handler::Handler};
use anyhow::Result;
use std::str;
use std::sync::Arc;
use tokio::net::TcpListener;

#[derive(Debug)]
pub struct Server<'a> {
    socket_addr: &'a str,
    main_cache: Arc<Cache>,
    listener: TcpListener,
}

impl<'a> Server<'a> {
    pub fn new(socket_addr: &'a str, main_cache: Arc<Cache>, listener: TcpListener) -> Self {
        Server {
            socket_addr,
            main_cache,
            listener,
        }
    }

    pub async fn run(&self) -> Result<()> {
        log::info!("{:?} {:?}", "Server is running on", self.socket_addr);

        loop {
            let incoming = self.listener.accept().await;

            match incoming {
                Ok((s, _)) => {
                    let client_cache = self.main_cache.clone();
                    let mut handler = Handler::new(client_cache, Some(Connection::new(s)));
                    tokio::spawn(async move {
                        handler.handle_connection().await;
                    });
                }
                Err(e) => {
                    log::error!("error: {:?}", e);
                }
            }
        }
    }
}
