//! Validated native Text Lab loss ingestion for one explanatory Patchbay session.

use super::{navigation_state, PatchbayHtmlServer, ServerError};
use conduit_core::SignId;
use std::path::PathBuf;

impl PatchbayHtmlServer {
    pub fn with_text_lab_loss_updates(mut self, base: String) -> Result<Self, ServerError> {
        self.text_lab_base = Some(base);
        let wasm_path = std::env::var_os("CONDUIT_BROWSER_RUNTIME_WASM")
            .map(PathBuf::from)
            .unwrap_or_else(|| {
                PathBuf::from("target/wasm32-unknown-unknown/release/conduit_browser_runtime.wasm")
            });
        let metadata = std::fs::metadata(&wasm_path).map_err(|error| {
            ServerError::Interaction(format!(
                "Text Lab browser runtime {} is unavailable ({error})",
                wasm_path.display()
            ))
        })?;
        if !metadata.is_file() || metadata.len() > super::MAX_BROWSER_WASM_BYTES as u64 {
            return Err(ServerError::Interaction(
                "Text Lab browser runtime is not one bounded regular WASM artifact".into(),
            ));
        }
        self.browser_wasm = Some(std::fs::read(wasm_path)?);
        Ok(self)
    }

    pub(super) fn apply_text_lab_loss(&mut self, body: &[u8]) -> Result<Vec<u8>, ServerError> {
        let base = self
            .text_lab_base
            .as_deref()
            .ok_or(ServerError::InvalidRequest)?;
        let receipt = serde_json::from_slice(body).map_err(|_| ServerError::InvalidRequest)?;
        let mut snapshot = crate::text_lab_split_loss_snapshot(base, &receipt)
            .map_err(ServerError::Interaction)?;
        snapshot.mark_available(SignId::from("patchbay-html/text-lab/loss-presented"))?;
        self.navigation = navigation_state(&snapshot)?;
        self.encoded_snapshot = snapshot.encode()?;
        self.snapshot = snapshot;
        Ok(self.encoded_snapshot.clone())
    }
}
