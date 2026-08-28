//! Host-neutral discovery projection wire operation (D72).
//!
//! A non-Rust host supplies the exact schemas its registry is about to expose.
//! The compiled world selects which names exist; the host remains the owner of
//! operational schemas. The response binds the exact visible schema array to a
//! deterministic hash and the same manifest identity returned by `gate`.
//!
//! This module is the **wire skin only** — stdin/stdout plus manifest loading.
//! The projection itself is `harness_preview::project`, beside `gate`, so an
//! in-process Rust host and the WASM build cannot answer discovery differently
//! (the same split `gate` already has).

use harness_preview::project;
use serde_json::Value;
use std::io::Read;
use std::path::Path;

pub fn run(world_path: &Path) -> i32 {
    let world = match crate::hostkit::load_compiled_world(world_path) {
        Ok(world) => world,
        Err(e) => {
            eprintln!("project: {e}");
            return 2;
        }
    };
    let mut input = String::new();
    if let Err(e) = std::io::stdin().read_to_string(&mut input) {
        eprintln!("project: cannot read stdin: {e}");
        return 1;
    }
    let request: Value = match serde_json::from_str(&input) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("project: malformed request: {e}");
            return 2;
        }
    };
    let response = match project(&world, &request) {
        Ok(value) => value,
        Err(e) => {
            eprintln!("project: malformed request: {e}");
            return 2;
        }
    };
    match serde_json::to_string(&response) {
        Ok(value) => {
            println!("{value}");
            0
        }
        Err(e) => {
            eprintln!("project: cannot serialize response: {e}");
            1
        }
    }
}
