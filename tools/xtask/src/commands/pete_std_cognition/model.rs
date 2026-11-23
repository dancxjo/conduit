//! Explicit local model selection and initialization receipts for both entrances.
use clap::Args;
use conduit_std_host::hosted_local_model::{
    HostedLocalModelAdapter, OllamaDiscovery, OllamaLocalModelAdapter, DEFAULT_OLLAMA_ENDPOINT,
};
use std::{io::Write, path::PathBuf};

#[derive(Args, Debug)]
pub(super) struct ModelArgs {
    /// Already installed Ollama model; no download is performed.
    #[arg(long, default_value = "gemma3:latest")]
    model: String,
    /// Exact HTTP loopback origin; IPv6 requires an explicit port.
    #[arg(long, default_value = DEFAULT_OLLAMA_ENDPOINT)]
    model_endpoint: String,
    /// Finite memory admitted to the selected model realization, in MiB.
    #[arg(long, default_value_t = 6144)]
    model_memory_mib: u32,
    /// Require this 64-character lowercase SHA-256 in Ollama's local inventory.
    #[arg(long)]
    expected_model_digest: Option<String>,
    /// Write a new resolved model/configuration receipt before opening Create serial.
    #[arg(long)]
    model_receipt_out: Option<PathBuf>,
}

impl ModelArgs {
    pub(super) fn validate(&self) -> Result<(), String> {
        if self.model.is_empty()
            || self.model.trim() != self.model
            || self.model.len() > conduit_ai::MAXIMUM_LOCAL_MODEL_IDENTITY_BYTES
            || self.model.chars().any(char::is_control)
        {
            return Err("--model must name one nonblank bounded local Ollama model".into());
        }
        if self.model_memory_mib == 0 {
            return Err("--model-memory-mib must be nonzero".into());
        }
        validate_endpoint(&self.model_endpoint)?;
        if let Some(digest) = &self.expected_model_digest {
            if digest.len() != 64
                || !digest
                    .bytes()
                    .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
            {
                return Err(
                    "--expected-model-digest must be 64 lowercase hexadecimal characters".into(),
                );
            }
        }
        Ok(())
    }

    pub(super) fn describe(&self) {
        println!(
            "Pete model: {} at {}; admitted memory: {} MiB; expected digest: {}",
            self.model,
            self.model_endpoint,
            self.model_memory_mib,
            self.expected_model_digest
                .as_deref()
                .unwrap_or("not specified"),
        );
    }

    fn verify_discovery(&self, discovery: &OllamaDiscovery) -> Result<(), String> {
        if self
            .expected_model_digest
            .as_ref()
            .is_some_and(|expected| expected != &discovery.model_content_identity)
        {
            return Err(format!(
                "selected Ollama model digest mismatch: expected {}, observed {}",
                self.expected_model_digest.as_deref().unwrap(),
                discovery.model_content_identity,
            ));
        }
        Ok(())
    }

    pub(super) fn prepare(&self) -> Result<OllamaLocalModelAdapter, String> {
        self.validate()?;
        if self
            .model_receipt_out
            .as_ref()
            .is_some_and(|path| path.exists())
        {
            return Err("model receipt destination already exists".into());
        }
        let discovery = OllamaDiscovery::discover_at(&self.model_endpoint, &self.model)
            .map_err(|error| format!("discover Ollama: {error}"))?;
        self.verify_discovery(&discovery)?;
        let selected = discovery.clone();
        let adapter = discovery
            .initialize(
                self.model_memory_mib,
                vec![conduit_ai::LocalModelKindProfile::GenerateFlow],
            )
            .map_err(|error| format!("initialize Ollama: {error}"))?;
        let receipt = serde_json::json!({
            "schema": "conduit.pete/model-initialization@1",
            "proof_class": "local_model_initialization",
            "endpoint": self.model_endpoint,
            "requested_model": self.model,
            "expected_model_digest": self.expected_model_digest,
            "artifact_identity_basis": "ollama_inventory_before_initialization",
            "discovery": selected,
            "initialized_offer": adapter.offer(),
        });
        let bytes = serde_json::to_vec(&receipt)
            .map_err(|error| format!("encode Pete model receipt: {error}"))?;
        if let Some(path) = &self.model_receipt_out {
            let mut file = std::fs::OpenOptions::new()
                .write(true)
                .create_new(true)
                .open(path)
                .map_err(|error| format!("open Pete model receipt {}: {error}", path.display()))?;
            file.write_all(&bytes)
                .and_then(|()| file.write_all(b"\n"))
                .and_then(|()| file.sync_all())
                .map_err(|error| format!("commit Pete model receipt: {error}"))?;
        }
        eprintln!(
            "CONDUIT_PETE_MODEL_EVIDENCE={}",
            String::from_utf8_lossy(&bytes)
        );
        Ok(adapter)
    }
}

fn validate_endpoint(endpoint: &str) -> Result<(), String> {
    let origin = endpoint
        .strip_prefix("http://")
        .filter(|_| endpoint.len() <= 256);
    let valid = origin.is_some_and(|origin| {
        ["127.0.0.1", "localhost", "[::1]"].iter().any(|host| {
            origin.strip_prefix(host).is_some_and(|suffix| {
                (suffix.is_empty() && *host != "[::1]")
                    || suffix.strip_prefix(':').is_some_and(|port| {
                        !port.is_empty()
                            && port.bytes().all(|byte| byte.is_ascii_digit())
                            && port.parse::<u16>().is_ok_and(|port| port != 0)
                    })
            })
        })
    });
    if !valid {
        return Err("--model-endpoint must be an HTTP loopback origin without a path; IPv6 requires an explicit nonzero port".into());
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exact_loopback_origin_excludes_remote_authority_and_url_suffixes() {
        for endpoint in [
            "http://127.0.0.1:11434",
            "http://localhost",
            "http://[::1]:1234",
        ] {
            assert!(validate_endpoint(endpoint).is_ok(), "{endpoint}");
        }
        for endpoint in [
            "https://127.0.0.1:11434",
            "http://remote:11434",
            "http://localhost.evil:11434",
            "http://localhost:0",
            "http://localhost:65536",
            "http://localhost:word",
            "http://[::1]",
            "http://localhost:11434/api",
            "http://localhost?query",
            "http://user@localhost:11434",
        ] {
            assert!(validate_endpoint(endpoint).is_err(), "{endpoint}");
        }
    }

    #[test]
    fn mismatched_artifact_refuses_before_initialization() {
        let selected = ModelArgs {
            model: "selected:fixture".into(),
            model_endpoint: DEFAULT_OLLAMA_ENDPOINT.into(),
            model_memory_mib: 6144,
            expected_model_digest: Some("a".repeat(64)),
            model_receipt_out: None,
        };
        let mut discovery = OllamaDiscovery {
            endpoint: DEFAULT_OLLAMA_ENDPOINT.into(),
            runtime_version: "fixture".into(),
            model_name: selected.model.clone(),
            model_content_identity: "b".repeat(64),
            model_bytes: 1,
            architecture: "fixture".into(),
            parameter_profile: "fixture".into(),
            quantization: "fixture".into(),
            context_length: 4096,
            completion_supported: true,
            embedding_supported: false,
            vision_supported: false,
        };
        assert!(selected
            .verify_discovery(&discovery)
            .unwrap_err()
            .contains("digest mismatch"));
        discovery.model_content_identity = "a".repeat(64);
        assert!(selected.verify_discovery(&discovery).is_ok());
    }
}
