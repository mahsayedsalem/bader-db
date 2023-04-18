mod server;
mod cache;
mod resp;

use std::time::Duration;
use crate::server::Server;

pub async fn run_server(socket_addr: &str, sample: usize, threshold: f64, frequency: Duration) {
    let server = Server::new(socket_addr, sample, threshold, frequency);
    log::info!("{:?}", "Server is created");
    server.run().await;
}
