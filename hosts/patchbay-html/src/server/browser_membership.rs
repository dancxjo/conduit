//! Browser renderer delivery for the truthful zero-Body public entrance.

use super::{PatchbayHtmlServer, ServerError, MAX_BROWSER_WASM_BYTES};
use std::path::PathBuf;

impl PatchbayHtmlServer {
    pub fn bind_browser_front_door_ephemeral() -> Result<Self, ServerError> {
        let mut server = Self::bind_front_door_ephemeral()?;
        let wasm_path = std::env::var_os("CONDUIT_BROWSER_RUNTIME_WASM")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm")
            });
        let metadata = std::fs::metadata(&wasm_path).map_err(|error| {
            ServerError::Interaction(format!(
                "browser Host runtime {} is unavailable ({error}); run through `cargo xtask demo patchbay --on browser`",
                wasm_path.display()
            ))
        })?;
        if !metadata.is_file() || metadata.len() > MAX_BROWSER_WASM_BYTES as u64 {
            return Err(ServerError::Interaction(
                "browser Host runtime is not one bounded regular WASM artifact".into(),
            ));
        }
        server.browser_wasm = Some(std::fs::read(&wasm_path).map_err(|error| {
            ServerError::Interaction(format!("cannot read browser Host runtime: {error}"))
        })?);
        // A browser renderer is not silently admitted into a Body. The
        // admission endpoint remains absent until an explicit JOIN or BE BORN
        // transition establishes one exact current Body.
        server.body_admission = None;
        Ok(server)
    }
}
