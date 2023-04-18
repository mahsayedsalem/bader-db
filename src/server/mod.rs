mod handler;

use std::net::SocketAddr;
use tokio::net::{TcpListener, TcpStream};
use std::sync::Arc;
use std::str;
use std::time::Duration;
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
            clone.remove_garbage().await
        });

        loop {
            let incoming = listener.accept().await;
            let client_cache = self.main_cache.clone();
            match incoming {
                Ok((s, addr)) => {
                    log::info!("{:?}", "Accepted new connection");
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

    pub async fn handle_connection(stream: TcpStream, addr: SocketAddr, client_store: Arc<Cache>) {
        log::info!("{:?} {:?}", "Connection established from", addr);

        let mut handler = Handler::new(stream);
        loop {
            if let Ok(value) = handler.read_value().await{
                if let Some(v) = value {
                    let response: Value = Handler::get_response(v, &client_store).await.unwrap();
                    log::info!("response: {:?}", response);
                    handler.write_value(response).await.unwrap();
                } else {
                    break;
                }
            } else {
                break;
            }
        }
    }
}
