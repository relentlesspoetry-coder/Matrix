//! Copied out of the application resources before shutdown. Headless by design.
#![allow(dead_code)]
#[path = "../update_lifecycle.rs"]
mod update_lifecycle;
#[path = "../update_worker.rs"]
mod update_worker;
fn main() {
    if std::env::args_os().len() != 1 {
        std::process::exit(1);
    }
    if let Err(error) = update_worker::run() {
        eprintln!("Matrix Play update: {error}");
        std::process::exit(1);
    }
}
