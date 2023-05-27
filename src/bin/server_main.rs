use bader_db::run_server;
use anyhow::Result;
use std::time::Duration;

#[tokio::main]
pub async fn main() -> Result<()> {
    env_logger::init();
    let port = std::env::var("PORT").unwrap_or("6379".to_string());
    let is_leader = std::env::var("IS_LEADER").unwrap_or("0".to_string());
    let sample = 10;
    let threshold = 0.5;
    let frequency = Duration::from_millis(100);
    run_server(format!("0.0.0.0:{}", port).as_str(), sample, threshold, frequency, &is_leader).await;
    Ok(())
}
