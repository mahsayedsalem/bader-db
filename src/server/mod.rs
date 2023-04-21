mod handler;
pub mod shutdown;
mod connection;

use std::sync::Arc;
use std::str;
use anyhow::Result;
use tokio::{net::TcpListener, sync::broadcast};
use crate::cache::Cache;
use crate::server::{connection::Connection, handler::Handler, shutdown::Shutdown};


#[derive(Debug)]
pub struct Server<'a> {
    socket_addr: &'a str,
    main_cache: Arc<Cache>,
    listener: TcpListener,
    pub notify_shutdown: broadcast::Sender<()>,
}

impl<'a> Server<'a> {
    pub fn new(socket_addr: &'a str,
               main_cache: Arc<Cache>,
               listener: TcpListener,
               notify_shutdown: broadcast::Sender<()>) -> Self {

        Server {
            socket_addr,
            main_cache,
            listener,
            notify_shutdown,
        }

    }

    pub async fn run(&self) -> Result<()> {

        log::info!("{:?} {:?}", "Server is running on", self.socket_addr);

        loop {

            let incoming = self.listener.accept().await;

            match incoming {
                Ok((s, _)) => {
                    let client_cache = self.main_cache.clone();
                    let mut handler = Handler::new(client_cache,
                                                   Some(Connection::new(s)),
                                                   Some(Shutdown::new(self.notify_shutdown.subscribe())));
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