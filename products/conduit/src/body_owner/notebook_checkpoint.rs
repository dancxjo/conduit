//! Product payload carried by the existing authorized immutable Text generation.
//! This is document meaning, not a filesystem record or a second storage engine.
use conduit_body::BodyId;
use conduit_semantic_catalog::{
    TextDocument, MAXIMUM_EDITED_TEXT_BYTES, MAXIMUM_TEXT_DOCUMENT_BYTES,
};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Checkpoint {
    schema: String,
    body_id: BodyId,
    name: String,
    revision: u64,
    text: String,
}

pub(super) fn encode(body: &BodyId, name: &str, document: &[u8]) -> Result<Vec<u8>, String> {
    validate_name(name)?;
    let document = TextDocument::decode(document).map_err(super::debug)?;
    let text = std::str::from_utf8(document.text).map_err(|error| error.to_string())?;
    let bytes = serde_json::to_vec(&Checkpoint {
        schema: "conduit.notebook/document@1".into(),
        body_id: body.clone(),
        name: name.into(),
        revision: document.revision,
        text: text.into(),
    })
    .map_err(|error| error.to_string())?;
    if bytes.len() > conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize {
        return Err("notebook checkpoint exceeds its selected Text profile".into());
    }
    Ok(bytes)
}

/// Accept only the selected generation's validated payload for this continuing
/// Body and document name. The caller must obtain bytes through the read Play.
pub(super) fn decode(body: &BodyId, name: &str, bytes: &[u8]) -> Result<Vec<u8>, String> {
    validate_name(name)?;
    if bytes.len() > conduit_data::MAXIMUM_DATA_TEXT_BYTES as usize {
        return Err("notebook checkpoint exceeds its selected Text profile".into());
    }
    let checkpoint: Checkpoint =
        serde_json::from_slice(bytes).map_err(|error| error.to_string())?;
    if checkpoint.schema != "conduit.notebook/document@1"
        || &checkpoint.body_id != body
        || checkpoint.name != name
        || checkpoint.text.len() > MAXIMUM_EDITED_TEXT_BYTES as usize
    {
        return Err("selected checkpoint differs from this notebook Body or document".into());
    }
    let mut current = vec![0; MAXIMUM_TEXT_DOCUMENT_BYTES as usize];
    let length = TextDocument {
        revision: checkpoint.revision,
        text: checkpoint.text.as_bytes(),
    }
    .encode_into(&mut current)
    .map_err(super::debug)?;
    current.truncate(length);
    Ok(current)
}

fn validate_name(name: &str) -> Result<(), String> {
    if name.is_empty() || name.len() > 128 || name.chars().any(char::is_control) {
        return Err("notebook name is outside its finite profile".into());
    }
    Ok(())
}
