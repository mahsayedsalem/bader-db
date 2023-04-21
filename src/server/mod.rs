mod handler;
pub mod shutdown;
mod connection;

use std::net::SocketAddr;
use std::sync::Arc;
use std::str;
use std::time::Duration;
use std::future::Future;
use bytes::BytesMut;
use anyhow::Result;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpStream, net::TcpListener, sync::broadcast, sync::mpsc};
use crate::resp::value::Value;
use crate::cache::Cache;
use crate::server::{connection::Connection, handler::Handler, shutdown::Shutdown};


#[derive(Debug)]
pub struct Server<'a> {
    socket_addr: &'a str,
    main_cache: Arc<Cache>,
    listener: TcpListener,
    pub notify_shutdown: broadcast::Sender<()>,
    pub shutdown_complete_tx: mpsc::Sender<()>,
}

impl<'a> Server<'a> {
    pub fn new(socket_addr: &'a str,
               main_cache: Arc<Cache>,
               listener: TcpListener,
               notify_shutdown: broadcast::Sender<()>,
               shutdown_complete_tx: mpsc::Sender<()>) -> Self {

        Server {
            socket_addr,
            main_cache,
            listener,
            notify_shutdown,
            shutdown_complete_tx,
        }

    }

    pub async fn run(&self) -> Result<()> {
        log::info!("{:?}", "Server is running");

        log::info!("{:?} {:?}", "Server is running on", self.socket_addr);

        // Create a a cache clone to spawn the cache monitor_for_expiry
        let clone = self.main_cache.clone();

        tokio::spawn(async move {
            clone.monitor_for_expiry().await
        });

        loop {
            let incoming = self.listener.accept().await;
            match incoming {
                Ok((s, addr)) => {
                    let client_cache = self.main_cache.clone();
                    let mut handler = Handler::new(client_cache,
                                           Connection::new(s),
                                           Shutdown::new(self.notify_shutdown.subscribe()),
                                           self.shutdown_complete_tx.clone());
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
