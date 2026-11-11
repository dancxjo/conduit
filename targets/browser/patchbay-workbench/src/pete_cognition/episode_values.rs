//! Read-only episode values from the retained native/actual-model artifacts.
use conduit_ai::{llm_contract, ModelDerivedResult, LLM_GENERATE_RECOVERABLE_FLOW_KIND};
use conduit_core::{
    semantic_digest, validate_canonical_structured_value, BorrowedTemporalInstant,
    ValidatedCanonicalStructuredValue,
};
use conduit_face::{
    FaceProperty, FacePropertyValue, FaceRelationship, FaceRelationshipKind, FaceRole, FaceSubject,
    FaceText,
};
use conduit_human::{ExperienceAvailability, ExperienceEvidenceRequest};
use conduit_plot::rust_binding::NativeRustBinding;
use patchbay_application::graph_face;
use serde_json::{json, Value};
use std::{
    error::Error,
    fs::File,
    io::{ErrorKind, Read},
    path::Path,
};
type Result<T> = std::result::Result<T, Box<dyn Error>>;
const SNIPPET_BYTES: usize = 900;
const MAX_SITUATION_BYTES: usize = 65_536;
const MAX_RESULT_BYTES: usize = 16_384;

pub(super) fn append(
    directory: &Path,
    body_subject: &str,
    content: &mut graph_face::Content<'_>,
) -> Result<()> {
    for episode in 1..=2 {
        let name = format!("situation-{episode}.canonical");
        match read(directory, &name, MAX_SITUATION_BYTES)? {
            Some(bytes) => evidence(episode, &name, &bytes, body_subject, content)?,
            None => missing(
                &format!("historical-episode/{episode}/evidence"),
                &format!("Episode {episode}: Situation not recorded"),
                &name,
                body_subject,
                content,
            )?,
        }
        let name = format!("response-{episode}.json");
        match read(directory, &name, MAX_RESULT_BYTES)? {
            Some(bytes) => result(episode, &name, &bytes, body_subject, content)?,
            None => missing(
                &format!("historical-episode/{episode}/model-result"),
                &format!("Episode {episode}: model result not recorded"),
                &name,
                body_subject,
                content,
            )?,
        }
    }
    Ok(())
}
fn evidence(
    episode: usize,
    artifact: &str,
    bytes: &[u8],
    body: &str,
    content: &mut graph_face::Content<'_>,
) -> Result<()> {
    let request = ExperienceEvidenceRequest::decode(bytes)
        .map_err(|error| format!("decode {artifact}: {error:?}"))?;
    // Decode establishes native shape; projection separately checks selection,
    // source bounds and epistemic combinations without resolving any identity.
    let mut validator = conduit_semantic_catalog::BoundedExperienceEvidenceProjection::prepare();
    validator
        .execute(bytes)
        .map_err(|error| format!("validate {artifact}: {error:?}"))?;
    for (index, item) in request.evidence().get().as_slice().iter().enumerate() {
        let id = format!("historical-episode/{episode}/evidence/{index}");
        subject(
            content,
            &id,
            &format!(
                "Episode {episode}: supplied evidence {} ({:?})",
                index + 1,
                item.origin()
            ),
            body,
        )?;
        artifact_properties(content, &id, artifact, bytes);
        for (key, value) in [
            ("identity", item.identity().get().as_str().to_owned()),
            ("origin", format!("{:?}", item.origin())),
            ("domain", format!("{:?}", item.domain())),
            ("availability", format!("{:?}", item.availability())),
            ("certainty", format!("{:?}", item.certainty())),
            ("temporal-role", format!("{:?}", item.temporal_role())),
            ("content-kind", item.content_kind().get().clone()),
            (
                "provenance-revision",
                item.provenance_revision().get().clone(),
            ),
        ] {
            text_property(content, &id, key, &value);
        }
        prop(
            content,
            &id,
            "revision",
            FacePropertyValue::Count(*item.revision()),
        );
        let encoded = item.encoded_content().get().as_slice();
        let (encoding, wording) = readable(encoded);
        text_property(content, &id, "content-encoding", encoding);
        text_property(content, &id, "content", &wording);
        prop(
            content,
            &id,
            "content-bytes",
            FacePropertyValue::Count(encoded.len() as u64),
        );
        prop(
            content,
            &id,
            "content-digest",
            FacePropertyValue::Identity(digest(encoded)),
        );
        let availability = match item.availability() {
            ExperienceAvailability::Present => "Supplied evidence",
            ExperienceAvailability::NotObserved => {
                "Unknown / not observed; this does not establish absence"
            }
            ExperienceAvailability::SourceUnavailable => {
                "Source unavailable / unknown; this does not establish absence"
            }
        };
        content.texts.push(FaceText {
            subject: id.clone(),
            text: format!("{availability}: {}", snippet(&wording).0),
        });
        for (source_index, source) in item.sources().get().as_slice().iter().enumerate() {
            let encoded = source
                .clone()
                .encode()
                .map_err(|error| format!("encode actual source: {error:?}"))?;
            let view = validate_canonical_structured_value(&encoded)
                .map_err(|error| format!("actual source shape: {error:?}"))?;
            let value = source_value(view)?;
            text_property(
                content,
                &id,
                &format!("source-{source_index}"),
                &serde_json::to_string(&value)?,
            );
            prop(
                content,
                &id,
                &format!("source-{source_index}-native-bytes"),
                FacePropertyValue::Count(encoded.len() as u64),
            );
            prop(
                content,
                &id,
                &format!("source-{source_index}-native-digest"),
                FacePropertyValue::Identity(digest(&encoded)),
            );
        }
    }
    if request.evidence().get().as_slice().is_empty() {
        let id = format!("historical-episode/{episode}/evidence");
        subject(
            content,
            &id,
            &format!("Episode {episode}: empty supplied evidence selection"),
            body,
        )?;
        artifact_properties(content, &id, artifact, bytes);
        content.texts.push(FaceText {subject:id,text:"No evidence items were selected; this is not an observation that the world or body was absent.".into()});
    }
    Ok(())
}
fn result(
    episode: usize,
    artifact: &str,
    bytes: &[u8],
    body: &str,
    content: &mut graph_face::Content<'_>,
) -> Result<()> {
    let result: ModelDerivedResult = serde_json::from_slice(bytes)?;
    let contract = llm_contract(LLM_GENERATE_RECOVERABLE_FLOW_KIND)
        .ok_or("recoverable model contract unavailable")?;
    result
        .validate(&contract)
        .map_err(|error| format!("validate {artifact}: {error:?}"))?;
    let id = format!("historical-episode/{episode}/model-result");
    subject(
        content,
        &id,
        &format!("Episode {episode}: ModelDerived result (historical)"),
        body,
    )?;
    // Put the recorded words and epistemic origin before audit details.
    let (encoding, wording) = readable(&result.payload);
    text_property(content, &id, "provenance", "ModelDerived");
    text_property(content, &id, "payload", &wording);
    text_property(content, &id, "payload-encoding", encoding);
    for (key, value) in [
        ("payload-kind", result.payload_kind),
        ("request-identity", result.request_identity),
        ("run-identity", result.run_identity),
        ("implementation-identity", result.implementation_identity),
        ("disposition", format!("{:?}", result.disposition)),
        ("determinism", format!("{:?}", result.determinism)),
    ] {
        text_property(content, &id, key, &value);
    }
    for (key, value) in [
        ("accounting-input-bytes", result.accounting.input_bytes),
        ("accounting-context-items", result.accounting.context_items),
        ("accounting-output-bytes", result.accounting.output_bytes),
        ("accounting-work-units", result.accounting.work_units),
        ("accounting-history-items", result.accounting.history_items),
    ] {
        prop(content, &id, key, FacePropertyValue::Count(value));
    }
    artifact_properties(content, &id, artifact, bytes);
    prop(
        content,
        &id,
        "payload-digest",
        FacePropertyValue::Identity(digest(&result.payload)),
    );
    content.texts.push(FaceText {
        subject: id,
        text: format!(
            "ModelDerived words, not an observation or confirmed effect: {}",
            snippet(&wording).0
        ),
    });
    Ok(())
}
fn missing(
    id: &str,
    name: &str,
    artifact: &str,
    body: &str,
    content: &mut graph_face::Content<'_>,
) -> Result<()> {
    subject(content, id, name, body)?;
    text_property(
        content,
        id,
        "record-status",
        "not recorded in this partial run",
    );
    text_property(content, id, "expected-artifact", artifact);
    content.texts.push(FaceText {subject:id.into(),text:"This artifact was not recorded. No response, observation, effect, or absence is inferred.".into()});
    Ok(())
}
fn subject(content: &mut graph_face::Content<'_>, id: &str, name: &str, body: &str) -> Result<()> {
    if content
        .subjects
        .iter()
        .any(|subject| subject.identity == id)
    {
        return Err("duplicate historical episode subject".into());
    }
    content.subjects.push(FaceSubject {
        identity: id.into(),
        role: FaceRole::Info,
        name: name.into(),
    });
    content.relationships.push(FaceRelationship {
        source: body.into(),
        target: id.into(),
        kind: FaceRelationshipKind::Contains,
    });
    prop(
        content,
        id,
        "inspection-mode",
        FacePropertyValue::Text("read-only historical artifact".into()),
    );
    Ok(())
}
fn artifact_properties(content: &mut graph_face::Content<'_>, id: &str, name: &str, bytes: &[u8]) {
    text_property(content, id, "artifact", name);
    prop(
        content,
        id,
        "artifact-bytes",
        FacePropertyValue::Count(bytes.len() as u64),
    );
    prop(
        content,
        id,
        "artifact-digest",
        FacePropertyValue::Identity(digest(bytes)),
    );
}
fn text_property(content: &mut graph_face::Content<'_>, id: &str, name: &str, value: &str) {
    // Empty native content is valid for an unobserved source. Face Text must
    // be nonempty; preserve emptiness as a fact rather than inventing wording.
    if value.is_empty() {
        prop(
            content,
            id,
            &format!("{name}-empty"),
            FacePropertyValue::Flag(true),
        );
        return;
    }
    let (preview, truncated) = snippet(value);
    prop(content, id, name, FacePropertyValue::Text(preview));
    if truncated {
        prop(
            content,
            id,
            &format!("{name}-truncated"),
            FacePropertyValue::Flag(true),
        );
        prop(
            content,
            id,
            &format!("{name}-full-bytes"),
            FacePropertyValue::Count(value.len() as u64),
        );
        prop(
            content,
            id,
            &format!("{name}-full-digest"),
            FacePropertyValue::Identity(digest(value.as_bytes())),
        );
    }
}
fn prop(content: &mut graph_face::Content<'_>, id: &str, name: &str, value: FacePropertyValue) {
    content.properties.push(FaceProperty {
        subject: id.into(),
        name: name.into(),
        value,
    });
}
fn snippet(value: &str) -> (String, bool) {
    const MARKER: &str = " … [truncated; exact artifact retained]";
    if value.len() <= SNIPPET_BYTES {
        return (value.into(), false);
    }
    let mut end = SNIPPET_BYTES - MARKER.len();
    while !value.is_char_boundary(end) {
        end -= 1;
    }
    (format!("{}{MARKER}", &value[..end]), true)
}
fn readable(bytes: &[u8]) -> (&'static str, String) {
    match std::str::from_utf8(bytes) {
        Ok(value) => {
            if let Ok(Value::Object(fields)) = serde_json::from_str::<Value>(value) {
                if fields.len() == 1 {
                    if let Some(Value::String(text)) = fields.get("text") {
                        return ("utf8/native-json-text-field", text.clone());
                    }
                }
            }
            ("utf8", value.into())
        }
        Err(_) => (
            "hex/non-UTF8",
            bytes.iter().map(|byte| format!("{byte:02x}")).collect(),
        ),
    }
}
fn digest(bytes: &[u8]) -> String {
    semantic_digest("conduit.pete/historical-episode-value@1", bytes)
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect()
}
fn read(directory: &Path, name: &str, maximum: usize) -> Result<Option<Vec<u8>>> {
    let file = match File::open(directory.join(name)) {
        Ok(file) => file,
        Err(error) if error.kind() == ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(error.into()),
    };
    let mut bytes = Vec::with_capacity(maximum);
    file.take(maximum as u64 + 1).read_to_end(&mut bytes)?;
    if bytes.len() > maximum {
        return Err(format!("{name} exceeds its bounded historical artifact extent").into());
    }
    Ok(Some(bytes))
}
fn source_value(view: ValidatedCanonicalStructuredValue<'_>) -> Result<Value> {
    for tag in ["source", "sign", "human_statement", "memory_record"] {
        if let Some(value) = view
            .variant_payload(tag)
            .map_err(|error| format!("source variant: {error:?}"))?
        {
            return Ok(json!({"role":tag,"identity":std::str::from_utf8(leaf(value)?)?}));
        }
    }
    if let Some(value) = view
        .variant_payload("implementation_run")
        .map_err(|error| format!("source variant: {error:?}"))?
    {
        let mut object = serde_json::Map::new();
        object.insert("role".into(), json!("implementation_run"));
        for key in ["implementation", "provider_instance", "artifact", "run"] {
            object.insert(
                key.into(),
                json!(std::str::from_utf8(leaf(field(value, key)?)?)?),
            );
        }
        return Ok(Value::Object(object));
    }
    if let Some(value) = view
        .variant_payload("temporal_context")
        .map_err(|error| format!("source variant: {error:?}"))?
    {
        let policy = field(value, "policy")?;
        return Ok(
            json!({"role":"temporal_context","observed_at":instant(field(value,"observed_at")?)?,
            "reference_at":instant(field(value,"reference_at")?)?,"policy":{
                "maximum_current_age_ticks":number(field(policy,"maximum_current_age_ticks")?)?,
                "maximum_recent_age_ticks":number(field(policy,"maximum_recent_age_ticks")?)?}}),
        );
    }
    if let Some(value) = view
        .variant_payload("resource")
        .map_err(|error| format!("source variant: {error:?}"))?
    {
        let bytes = leaf(value)?;
        return Ok(
            json!({"role":"resource","meaning":"inert provenance; no resource access",
            "canonical_payload_hex":bytes.iter().map(|byte|format!("{byte:02x}")).collect::<String>()}),
        );
    }
    Err("unsupported actual native evidence source".into())
}
fn instant(value: ValidatedCanonicalStructuredValue<'_>) -> Result<Value> {
    let value = BorrowedTemporalInstant::from_canonical(value)
        .map_err(|error| format!("source time: {error:?}"))?;
    Ok(
        json!({"clock_basis":value.clock_basis,"ticks":value.ticks,"scale":format!("{:?}",value.scale),
        "resolution_ticks":value.resolution_ticks,"uncertainty_ticks":value.uncertainty_ticks}),
    )
}
fn field<'a>(
    value: ValidatedCanonicalStructuredValue<'a>,
    name: &str,
) -> Result<ValidatedCanonicalStructuredValue<'a>> {
    value
        .record_field(name)
        .map_err(|error| format!("source field: {error:?}"))?
        .ok_or_else(|| "source field missing".into())
}
fn leaf(value: ValidatedCanonicalStructuredValue<'_>) -> Result<&[u8]> {
    let node = value.value_node();
    if node.len() < 5 || node[0] != 0 {
        return Err("source scalar malformed".into());
    }
    Ok(&node[5..])
}
fn number(value: ValidatedCanonicalStructuredValue<'_>) -> Result<u64> {
    Ok(u64::from_le_bytes(leaf(value)?.try_into()?))
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn snippets_preserve_utf8_boundaries_and_mark_exact_artifact_custody() {
        let input = "é".repeat(900);
        let (preview, truncated) = snippet(&input);
        assert!(truncated);
        assert!(preview.len() <= 900);
        assert!(preview.ends_with("[truncated; exact artifact retained]"));
        assert_eq!(snippet("complete"), ("complete".into(), false));
    }
}
