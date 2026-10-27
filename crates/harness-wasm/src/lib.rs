//! WebAssembly engine (E14): the **real** compiler + kernel, callable from JS.
//!
//! This is a thin `wasm-bindgen` shim over [`harness_preview`] — the *same* pure
//! functions the native front ends use, so the in-browser visualizations (E15)
//! and the WebMCP browser extension run the actual governance logic with no
//! reimplementation and no possibility of drift (DECISIONS D22):
//! - [`preview`] is `harness serve`'s design-time preview (E11);
//! - [`gate`] is the `harness gate` wire operation (D24), one proposed call;
//! - [`project`] is the `harness project` wire operation (D72), one tool surface.
//!
//! Functions return JSON **strings** (the caller `JSON.parse`s them) and never
//! throw for ordinary bad input: a parse/compile failure, or a malformed
//! request, comes back as `{"ok":false,"error":...}`.
//!
//! `gate` and `project` take the world as manifest YAML and compile it on every
//! call, as `harness gate --world` reads it on every invocation. The one step
//! the native adapter adds before compiling — resolving and canonicalizing
//! `roots` paths against the filesystem — has no meaning in a browser and is
//! not done here: a world whose roots are relative or symlinked answers
//! differently, which is why the native/WASM parity suite covers the pathless
//! case sets only (`tests/parity.rs`).

use compiler::{compile, loader::load_yaml};
use harness_types::CompiledWorld;
use serde_json::{json, Value};
use wasm_bindgen::prelude::*;

/// Compile a draft `WorldManifest` (YAML) and return, as a JSON string, the
/// projected tool surface plus a clean-vs-tainted decision matrix per action.
///
/// Shape: `{ok:true, world_id, manifest_hash, surface[], decisions[]}` on
/// success, or `{ok:false, error}` on a parse/compile error.
#[wasm_bindgen]
pub fn preview(yaml: &str) -> String {
    harness_preview::preview(yaml).to_string()
}

/// Govern one proposed call: the `harness gate` wire operation (gate ABI, D24).
///
/// `request` is a `GateRequest` as JSON; the result is the `GateResponse` JSON
/// the native `harness gate --world <world>` prints for the same input, or
/// `{ok:false, error}` where the native command exits non-zero (a world that
/// does not compile, a malformed request).
#[wasm_bindgen]
pub fn gate(world_yaml: &str, request: &str) -> String {
    let world = match compile_world(world_yaml) {
        Ok(w) => w,
        Err(e) => return error(e),
    };
    let req: harness_preview::GateRequest = match serde_json::from_str(request) {
        Ok(r) => r,
        Err(e) => return error(format!("malformed GateRequest: {e}")),
    };
    match serde_json::to_string(&harness_preview::gate(&world, &req)) {
        Ok(json) => json,
        Err(e) => error(format!("cannot serialize response: {e}")),
    }
}

/// Shape one offered tool surface: the `harness project` wire operation (D72).
///
/// `request` is the projection request as JSON; the result is the response the
/// native `harness project --world <world>` prints, or `{ok:false, error}`.
#[wasm_bindgen]
pub fn project(world_yaml: &str, request: &str) -> String {
    let world = match compile_world(world_yaml) {
        Ok(w) => w,
        Err(e) => return error(e),
    };
    let req: Value = match serde_json::from_str(request) {
        Ok(r) => r,
        Err(e) => return error(format!("malformed request: {e}")),
    };
    match harness_preview::project(&world, &req) {
        Ok(res) => res.to_string(),
        Err(e) => error(format!("malformed request: {e}")),
    }
}

/// The bundled default world manifest (YAML) — seeds editors and playgrounds.
#[wasm_bindgen]
pub fn default_world() -> String {
    compiler::default_world_yaml().to_string()
}

/// The engine (harness) version, so a page can show which kernel build it runs.
#[wasm_bindgen]
pub fn version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

fn compile_world(yaml: &str) -> Result<CompiledWorld, String> {
    let manifest = load_yaml(yaml).map_err(|e| format!("cannot parse world: {e}"))?;
    compile(&manifest).map_err(|e| format!("cannot compile world: {e}"))
}

fn error(message: impl Into<String>) -> String {
    json!({ "ok": false, "error": message.into() }).to_string()
}
