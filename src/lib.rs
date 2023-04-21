mod server;
mod cache;
mod resp;

use std::time::Duration;
use std::sync::Arc;
use tokio::{io::AsyncReadExt, io::AsyncWriteExt, net::TcpStream, net::TcpListener, sync::broadcast, sync::mpsc, signal};

use crate::server::Server;
use crate::cache::Cache;
use crate::server::shutdown::Shutdown;

pub async fn run_server(socket_addr: &str,
                        sample: usize,
                        threshold: f64,
                        frequency: Duration) {

    // Bind a tcp listener
    let listener = TcpListener::bind(socket_addr).await.unwrap();

    // Create the shutdown signal which will shutdown when we hit ctrl_c
    let shutdown = signal::ctrl_c();

    // Channels used for a graceful shutdown
    let (notify_shutdown, _) = broadcast::channel(1);
    let (shutdown_complete_tx, mut shutdown_complete_rx) = mpsc::channel(1);

    // Create the main_cache arc that we clone in every connection. We only clone a ref to the store
    // which makes it inexpensive
    let main_cache = Arc::new(Cache::new(
        sample,
        threshold,
        frequency,
        Some(Shutdown::new(notify_shutdown.subscribe())),
    ));

    let mut server = Server::new(socket_addr,
                             main_cache,
                             listener,
                             notify_shutdown,
                             shutdown_complete_tx);

    log::info!("{:?}", "Server is created");

    tokio::select! {
        res = server.run() => {
            if let Err(err) = res {
                log::error!("failed to run the server, error {:?}", err);
            }
        }
        _ = shutdown => {
            log::info!("shutting down");
        }
    }

    let Server {
        shutdown_complete_tx,
        notify_shutdown,
        ..
    } = server;

    drop(notify_shutdown);
    drop(shutdown_complete_tx);


    let _ = shutdown_complete_rx.recv().await;
}
