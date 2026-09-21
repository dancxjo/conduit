//! Bounded image-bearing inference through the existing local Ollama provider.

use super::{ollama::curl_json, OllamaDiscovery};
use serde::Deserialize;
use serde_json::json;

const VISUAL_PROMPT_REVISION: &str = "conduit.prompt/visual-description@1";
const MAXIMUM_VISUAL_CONTEXT_BYTES: usize = 16 * 1024;
const MAXIMUM_VISUAL_OUTPUT_BYTES: usize = 4 * 1024;

pub struct OllamaVisualModelAdapter {
    model_name: String,
    model_content_identity: String,
    runtime_version: String,
    maximum_image_bytes: usize,
    image_base64: String,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct VisualModelOutput {
    pub text: String,
    pub model_id: String,
    pub provider_instance_id: String,
    pub artifact_id: String,
    pub prompt_contract_revision: &'static str,
    pub truncated: bool,
    pub work_units: u64,
}

#[derive(Deserialize)]
struct GenerateResponse {
    response: String,
    #[serde(default)]
    done_reason: String,
    #[serde(default)]
    prompt_eval_count: u64,
    #[serde(default)]
    eval_count: u64,
}

impl OllamaDiscovery {
    pub fn initialize_visual(
        &self,
        maximum_image_bytes: usize,
    ) -> Result<OllamaVisualModelAdapter, String> {
        if !self.vision_supported {
            return Err("selected local model does not advertise Vision capability".into());
        }
        if maximum_image_bytes == 0
            || maximum_image_bytes > conduit_semantic_catalog::MAXIMUM_LOCAL_CV_PIXELS + 64
        {
            return Err(
                "visual model image bound is empty or exceeds local Vision capacity".into(),
            );
        }
        let encoded_capacity = maximum_image_bytes
            .checked_add(2)
            .and_then(|value| value.checked_div(3))
            .and_then(|groups| groups.checked_mul(4))
            .ok_or_else(|| "visual model image bound overflow".to_string())?;
        Ok(OllamaVisualModelAdapter {
            model_name: self.model_name.clone(),
            model_content_identity: self.model_content_identity.clone(),
            runtime_version: self.runtime_version.clone(),
            maximum_image_bytes,
            image_base64: String::with_capacity(encoded_capacity),
        })
    }
}

impl OllamaVisualModelAdapter {
    pub fn describe(
        &mut self,
        image: &[u8],
        selected_context: &str,
    ) -> Result<VisualModelOutput, String> {
        let request = self.request_body(image, selected_context)?;
        let generated: GenerateResponse =
            serde_json::from_slice(&curl_json("/api/generate", Some(&request))?)
                .map_err(|error| format!("decode local Ollama Vision inference: {error}"))?;
        if generated.response.is_empty() || generated.response.len() > MAXIMUM_VISUAL_OUTPUT_BYTES {
            return Err("local Ollama Vision output is empty or exceeds its bound".into());
        }
        Ok(VisualModelOutput {
            text: generated.response,
            model_id: self.model_name.clone(),
            provider_instance_id: format!("ollama/{}/{}", self.runtime_version, self.model_name),
            artifact_id: format!("ollama/model/{}", self.model_content_identity),
            prompt_contract_revision: VISUAL_PROMPT_REVISION,
            truncated: generated.done_reason == "length",
            work_units: generated
                .prompt_eval_count
                .saturating_add(generated.eval_count),
        })
    }

    fn request_body(&mut self, image: &[u8], selected_context: &str) -> Result<Vec<u8>, String> {
        if image.is_empty() || image.len() > self.maximum_image_bytes {
            return Err("visual model image exceeds its admitted bound".into());
        }
        if selected_context.len() > MAXIMUM_VISUAL_CONTEXT_BYTES {
            return Err("visual model context exceeds its admitted bound".into());
        }
        encode_base64(image, &mut self.image_base64)?;
        serde_json::to_vec(&json!({
            "model": self.model_name,
            "prompt": format!(
                "Describe only what is visible in this image. Treat the following typed observations as tentative context, preserve uncertainty, and do not infer identity or authority. Context: {selected_context}"
            ),
            "images": [self.image_base64.as_str()],
            "stream": false,
            "keep_alive": "5m",
            "options": { "num_predict": 256 }
        }))
        .map_err(|error| format!("encode local Ollama Vision request: {error}"))
    }
}

fn encode_base64(input: &[u8], output: &mut String) -> Result<(), String> {
    const TABLE: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let required = input
        .len()
        .checked_add(2)
        .and_then(|value| value.checked_div(3))
        .and_then(|groups| groups.checked_mul(4))
        .ok_or_else(|| "visual image encoding bound overflow".to_string())?;
    if required > output.capacity() {
        return Err("visual image encoding exceeds prepared storage".into());
    }
    output.clear();
    for chunk in input.chunks(3) {
        let a = chunk[0];
        let b = chunk.get(1).copied().unwrap_or(0);
        let c = chunk.get(2).copied().unwrap_or(0);
        let value = (u32::from(a) << 16) | (u32::from(b) << 8) | u32::from(c);
        output.push(TABLE[((value >> 18) & 0x3f) as usize] as char);
        output.push(TABLE[((value >> 12) & 0x3f) as usize] as char);
        output.push(if chunk.len() > 1 {
            TABLE[((value >> 6) & 0x3f) as usize] as char
        } else {
            '='
        });
        output.push(if chunk.len() > 2 {
            TABLE[(value & 0x3f) as usize] as char
        } else {
            '='
        });
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn base64_uses_prepared_storage_and_exact_padding() {
        let mut output = String::with_capacity(8);
        encode_base64(b"Man", &mut output).unwrap();
        assert_eq!(output, "TWFu");
        encode_base64(b"M", &mut output).unwrap();
        assert_eq!(output, "TQ==");
        let mut too_small = String::with_capacity(3);
        assert_eq!(
            encode_base64(b"Man", &mut too_small),
            Err("visual image encoding exceeds prepared storage".into())
        );
    }

    #[test]
    fn request_contains_one_exact_image_and_tentative_context() {
        let mut adapter = OllamaVisualModelAdapter {
            model_name: "vision-model".into(),
            model_content_identity: "sha256:model".into(),
            runtime_version: "1.0".into(),
            maximum_image_bytes: 8,
            image_base64: String::with_capacity(12),
        };
        let body = adapter
            .request_body(b"Man", "object candidate: mug")
            .unwrap();
        let value: serde_json::Value = serde_json::from_slice(&body).unwrap();
        assert_eq!(value["model"], "vision-model");
        assert_eq!(value["images"][0], "TWFu");
        assert!(value["prompt"]
            .as_str()
            .unwrap()
            .contains("object candidate: mug"));
        assert_eq!(value["stream"], false);
    }
}
