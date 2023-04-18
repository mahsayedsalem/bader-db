use bader_db::run_server;
use anyhow::Result;
use std::time::Duration;

#[tokio::main]
pub async fn main() -> Result<()> {
    env_logger::init();
    let host = format!("127.0.0.1:{}", port).as_str();
    let sample = 10;
    let threshold = 0.5;
    let frequency = Duration::from_millis(100);
    let port = std::env::var("PORT").unwrap_or("6379".to_string());
    run_server(host, sample, threshold, frequency).await;
    Ok(())
}