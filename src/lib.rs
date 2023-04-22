mod server;
mod cache;
mod resp;

use std::time::Duration;
use std::sync::Arc;
use tokio::{net::TcpListener, sync::broadcast, signal};

use crate::server::Server;
use crate::cache::Cache;

pub async fn run_server(socket_addr: &str,
                        sample: usize,
                        threshold: f64,
                        frequency: Duration) {

    // Bind a tcp listener
    let listener = TcpListener::bind(socket_addr).await.unwrap();

    // Create the shutdown signal which will shutdown when we hit ctrl_c
    let shutdown = signal::ctrl_c();

    // Create the main_cache arc that we clone in every connection. We only clone a ref to the store
    // which makes it inexpensive
    let main_cache = Arc::new(Cache::new(
        sample,
        threshold,
        frequency,
    ));

    // Create a a cache clone to spawn the cache monitor_for_expiry
    let clone = main_cache.clone();

    let task = tokio::spawn(async move {
        clone.monitor_for_expiry().await
    });

    // Create the server instance
    let server = Server::new(socket_addr,
                                 main_cache,
                                 listener);

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

    // closing background monitor
    task.abort();


    log::info!("{:?}", "Server is closed");
}