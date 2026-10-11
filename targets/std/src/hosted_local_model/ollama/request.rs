//! Existing finite inference request shapes and configuration helpers.
use super::*;
use conduit_presentation::GenerativePresenterInput;

impl OllamaLocalModelAdapter {
    pub(super) fn generate(
        &self,
        input: &str,
        maximum_tokens: u64,
        structured: bool,
    ) -> Result<GenerateResponse, LocalModelAdapterTerminal> {
        let mut request = json!({
            "model": self.model_name,
            "prompt": input,
            "think": false,
            "stream": false,
            "keep_alive": "5m",
            "options": { "num_predict": maximum_tokens }
        });
        if structured {
            request["format"] = json!("json");
        }
        let body = serde_json::to_vec(&request)
            .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)?;
        let response: GenerateResponse = serde_json::from_slice(
            &curl_json(&self.endpoint, "/api/generate", Some(&body))
                .map_err(|_| LocalModelAdapterTerminal::ProviderLost)?,
        )
        .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)?;
        if response.done != Some(true) {
            return Err(LocalModelAdapterTerminal::InvalidStructuredResult);
        }
        Ok(response)
    }

    pub(super) fn embed(&self, input: &str) -> Result<EmbedResponse, LocalModelAdapterTerminal> {
        let body = serde_json::to_vec(&json!({
            "model": self.model_name,
            "input": input,
            "truncate": false,
            "keep_alive": "5m"
        }))
        .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)?;
        serde_json::from_slice(
            &curl_json(&self.endpoint, "/api/embed", Some(&body))
                .map_err(|_| LocalModelAdapterTerminal::ProviderLost)?,
        )
        .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)
    }

    pub(super) fn chat_present(
        &self,
        system: &str,
        semantic_data: &str,
        maximum_tokens: u64,
    ) -> Result<ChatResponse, LocalModelAdapterTerminal> {
        let input: GenerativePresenterInput = serde_json::from_str(semantic_data)
            .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)?;
        let available_actions = input
            .presentation
            .actions
            .iter()
            .filter(|action| action.availability.is_available())
            .map(|action| action.identity.clone())
            .collect::<Vec<_>>();
        let action_items = if available_actions.is_empty() {
            json!({ "type": "string" })
        } else {
            json!({ "type": "string", "enum": available_actions })
        };
        // The provider grammar guides bounded structure. The Host still
        // validates every Face identity, value, and available action after
        // inference; schema conformance alone grants no semantic authority.
        let format = if system == super::super::ollama_present::WORDING_SYSTEM_POLICY {
            super::super::ollama_present::wording_format(&input, &available_actions)
                .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)?
        } else {
            json!({
                "type": "object",
                "properties": {
                    "speech_text_index": { "type": "integer", "minimum": 0 },
                    "presented_thought_text_index": { "type": ["integer", "null"], "minimum": 0 },
                    "suggested_action_identities": {
                        "type": "array",
                        "items": action_items,
                        "maxItems": available_actions.len()
                    }
                },
                "required": ["speech_text_index", "presented_thought_text_index", "suggested_action_identities"],
                "additionalProperties": false
            })
        };
        let body = serde_json::to_vec(&json!({
            "model": self.model_name,
            "messages": [
                { "role": "system", "content": system },
                { "role": "user", "content": semantic_data }
            ],
            "stream": false,
            "think": false,
            "format": format,
            "keep_alive": "5m",
            "options": { "num_predict": maximum_tokens, "temperature": 0 }
        }))
        .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)?;
        let response: ChatResponse = serde_json::from_slice(
            &curl_json(&self.endpoint, "/api/chat", Some(&body))
                .map_err(|_| LocalModelAdapterTerminal::ProviderLost)?,
        )
        .map_err(|_| LocalModelAdapterTerminal::InvalidStructuredResult)?;
        if response.done != Some(true) {
            return Err(LocalModelAdapterTerminal::InvalidStructuredResult);
        }
        Ok(response)
    }
}

pub(super) fn proof_presenter_diagnostic(stage: &str, error: &str) {
    #[cfg(feature = "local-model-proof")]
    eprintln!("local-model Presenter {stage} failure: {error}");
    #[cfg(not(feature = "local-model-proof"))]
    let _ = (stage, error);
}

pub(super) fn configuration_count(placement: &PlannedGear, key: &str) -> Option<u64> {
    placement.configuration.iter().find_map(|entry| {
        (entry.key == key)
            .then_some(&entry.value)
            .and_then(|value| match value {
                conduit_core::ConfigurationValue::U64(value) => Some(*value),
                _ => None,
            })
    })
}
