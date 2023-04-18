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