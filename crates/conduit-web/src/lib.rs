//! Safe browser bindings to the production `.panel` parser.

use wasm_bindgen::prelude::*;

/// Returns a small JSON summary produced from `conduit_panel::parse` itself.
#[wasm_bindgen]
pub fn parse_panel(source: String) -> String {
    match conduit_panel::parse(&source) {
        Ok(panel) => format!(
            "{{\"ok\":true,\"nodes\":{},\"cords\":{}}}",
            panel.nodes.len(),
            panel.cords.len()
        ),
        Err(error) => format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    }
}

/// Executes the existing finite hosted proof runtime with bounded in-memory
/// streams, returning only public stdout/stderr and the exact runtime result.
#[wasm_bindgen]
pub fn run_panel(source: String) -> String {
    let panel = match conduit_panel::parse(&source) {
        Ok(panel) => panel,
        Err(error) => return format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    };
    let registry = conduit_runtime::Registry::default();
    let resolved = match registry.resolve(&panel) {
        Ok(resolved) => resolved,
        Err(error) => return format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    };
    let mut input = std::io::empty();
    let mut output = Vec::new();
    let mut error = Vec::new();
    let mut io = conduit_runtime::RunIo {
        input: &mut input,
        output: &mut output,
        error: &mut error,
    };
    match resolved.run(&mut io) {
        Ok(summary) => format!(
            "{{\"ok\":true,\"completed_nodes\":{},\"stdout\":{:?},\"stderr\":{:?}}}",
            summary.nodes_completed,
            String::from_utf8_lossy(&output),
            String::from_utf8_lossy(&error)
        ),
        Err(error) => format!("{{\"ok\":false,\"diagnostic\":{:?}}}", error.to_string()),
    }
}
