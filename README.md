[![Build Status](https://img.shields.io/github/actions/workflow/status/mahsayedsalem/bader-db/quickstart.yml?branch=main)](https://github.com/mahsayedsalem/bader-db/actions)
[![Crates.io](https://img.shields.io/crates/v/bader-db.svg)](https://crates.io/crates/bader-io)

<h1 align="center">
  BADER-DB (بادِر)
</h1>

<h4 align="center">Key-value cache RESP server with support for key expirations ⌛</h4>
<p align="center">
  <a href="#supported-features">Supported Features</a> •
  <a href="#getting-started">Getting Started</a> •
  <a href="#basic-usage">Basic Usage</a> •
  <a href="#cache-eviction">Cache Eviction</a> •
  <a href="#testing">Testing</a>
</p>

This is a rust work-in-progress learning project and shouldn't be used in production.

## Supported Features

* SET 🏪 — Set with or without an expiry date.
* GET ⚡ — Get value by key, we store our values in a BTree.
* Key Eviction ⌛ — A memory-efficient probabilistic eviction algorithm similar to [Redis](https://redis.io/commands/expire).
* Memory Safe 🛡️ — Ensures the latest value is always retrieved, handles race conditions.

## Getting Started

This crate is available on [crates.io](https://crates.io/crates/bader-db). The
easiest way to use it is to add an entry to your `Cargo.toml` defining the dependency:

```toml
[dependencies]
bader-db = "0.1.2"
```

## Basic Usage
The TCP server is built on Tokio to handle async connections, make sure you include it in your dependencies.

```rust
use bader_db::run_server;
use anyhow::Result;
use std::time::Duration;

#[tokio::main]
pub async fn main() -> Result<()> {
    env_logger::init();
    let port = std::env::var("PORT").unwrap_or("6379".to_string());
    let sample = 10;
    let threshold = 0.5;
    let frequency = Duration::from_millis(100);
    run_server(format!("127.0.0.1:{}", port).as_str(), sample, threshold, frequency).await;
    Ok(())
}
```
The `run_server` method will take the host and the eviction algorithm parameters. 

## Cache Eviction

The eviction algorithm parameters: 
* sample
* frequency
* threshold

Expiration of keys in the cache is performed at intervals, triggered every `frequency` and is done in the background. The approach for performing this task is based on the implementation in Redis, which is straightforward yet effective.

The following is a summary of the eviction process, explained in a clear way:

    Wait for the next frequency tick.
    Randomly select a batch of sample entries from the cache.
    Check for and remove any expired entries found in the batch.
    If the percentage of removed entries is greater than the threshold, go back to step 2; otherwise, go back to step 1.

This allows the user to control the aggressiveness of eviction by adjusting the values of threshold and frequency. It's important to keep in mind that the higher the threshold value, the more memory the cache will use on average.

## Testing 
using `redis-cli` you can run this simple script to set some values with different expirations and watch the logs as the eviction algorithm takes place.

```rust
use std::process::Command;
use std::{thread, time};

fn main() {
    println!("Started filling");

    for i in 10..2000 {
        let expiry = 10 * i;
        let sleep_time = time::Duration::from_nanos(100);
        thread::sleep(sleep_time);
        let i = format!("nx{}", i.to_string());
        Command::new("redis-cli")
            .arg("set")
            .arg(i.to_string())
            .arg(i.to_string())
            .arg("EXP")
            .arg(expiry.to_string())
            .spawn()
            .expect("ls command failed to start");
    }

    for i in 10..5000 {
        let expiry = 10 * i;
        let sleep_time = time::Duration::from_nanos(100);
        thread::sleep(sleep_time);
        Command::new("redis-cli")
            .arg("set")
            .arg(i.to_string())
            .arg(i.to_string())
            .arg("EXP")
            .arg(expiry.to_string())
            .spawn()
            .expect("ls command failed to start");
    }
    println!("Terminated.");
}
```