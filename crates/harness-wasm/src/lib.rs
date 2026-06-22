//! WebAssembly engine (E14): the **real** compiler + kernel, callable from JS.
//!
//! This is a thin `wasm-bindgen` shim over [`harness_preview::preview`] — the
//! *same* pure preview the native authoring tool (`harness serve`, E11) uses, so
//! the in-browser visualizations (E15) run the actual governance logic with no
//! reimplementation and no possibility of drift (DECISIONS D22).
//!
//! Functions return JSON **strings** (the caller `JSON.parse`s them) and never
//! throw for ordinary bad input: a parse/compile failure comes back as
//! `{"ok":false,"error":...}` from [`preview`].

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
