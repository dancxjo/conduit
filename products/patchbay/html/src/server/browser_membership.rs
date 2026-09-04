//! Browser renderer delivery for the truthful zero-Body public entrance.

use super::{PatchbayHtmlServer, ServerError, MAX_BROWSER_WASM_BYTES};
use patchbay_model::FormCandidate;
use std::path::PathBuf;

impl PatchbayHtmlServer {
    pub fn bind_browser_front_door_ephemeral() -> Result<Self, ServerError> {
        Self::bind_browser_front_door_with_forms_ephemeral(Vec::new())
    }

    pub fn bind_browser_front_door_with_forms_ephemeral(
        forms: Vec<FormCandidate>,
    ) -> Result<Self, ServerError> {
        let mut server = Self::bind_front_door_with_forms_ephemeral(forms)?;
        server.browser_wasm = Some(read_browser_runtime()?);
        // A browser renderer is not silently admitted into a Body. The
        // admission endpoint remains absent until an explicit JOIN or BIRTH
        // transition establishes one exact current Body.
        server.body_admission = None;
        Ok(server)
    }

    pub fn with_body_invitation(mut self, url: &str) -> Result<Self, ServerError> {
        validate_local_invitation(url)?;
        self.body_admission = Some(
            serde_json::to_vec(&serde_json::json!({ "url": url }))
                .map_err(|error| ServerError::Interaction(error.to_string()))?,
        );
        self.browser_wasm = Some(read_browser_runtime()?);
        Ok(self)
    }
}

fn read_browser_runtime() -> Result<Vec<u8>, ServerError> {
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
    std::fs::read(&wasm_path).map_err(|error| {
        ServerError::Interaction(format!("cannot read browser Host runtime: {error}"))
    })
}

fn validate_local_invitation(url: &str) -> Result<(), ServerError> {
    if url.is_empty() || url.len() > 2048 || url.chars().any(char::is_whitespace) {
        return Err(ServerError::Interaction(
            "Body invitation is empty or exceeds its finite bound".into(),
        ));
    }
    let remainder = url.strip_prefix("ws://127.0.0.1:").ok_or_else(|| {
        ServerError::Interaction("Body invitation must use a loopback WebSocket URL".into())
    })?;
    let authority = remainder.split('/').next().unwrap_or_default();
    let port = authority.parse::<u16>().map_err(|_| {
        ServerError::Interaction("Body invitation has an invalid loopback port".into())
    })?;
    if port < 1024 {
        return Err(ServerError::Interaction(
            "Body invitation uses a reserved loopback port".into(),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn body_invitation_is_bounded_and_loopback_only() {
        assert!(validate_local_invitation("ws://127.0.0.1:4173/body").is_ok());
        assert!(validate_local_invitation("wss://example.com/body").is_err());
        assert!(validate_local_invitation("ws://127.0.0.1:80/body").is_err());
        assert!(
            validate_local_invitation(&format!("ws://127.0.0.1:4173/{}", "x".repeat(2048)))
                .is_err()
        );
    }
}
