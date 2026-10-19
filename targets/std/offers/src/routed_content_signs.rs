//! Optional shared execution-evidence requirement; independent of numeric types.
use conduit_core::*;
pub const ROUTED_CONTENT_DIGEST_DOMAIN: &str = "conduit/routed-value-content@1";
pub const ROUTED_CONTENT_DIGEST_SCHEME: &str = "core/semantic-sha256@1";
/// Hash input extents are the already-admitted value-store extents. Storage is
/// sized from the prepared event ceiling, not a lifetime event-count promise.
pub fn routed_content_sign_requirement(
) -> Result<StructuredConfigurationValue, StructuredInfoRefusal> {
    let text = StructuredInfoType::leaf(kind_id(TEXT_INFO_ID))?;
    let fields = [
        ("digest-domain", ROUTED_CONTENT_DIGEST_DOMAIN),
        ("digest-scheme", ROUTED_CONTENT_DIGEST_SCHEME),
        ("storage-rule", "kernel/retained-event-ceiling@1"),
        (
            "work-rule",
            "one-digest-per-committed-route-with-admitted-value-extent@1",
        ),
    ];
    let ty = StructuredInfoType::record(
        kind_id("std/routed-content-signs@1"),
        fields
            .iter()
            .map(|(name, _)| StructuredFieldType::new(*name, text.clone()))
            .collect::<Result<_, _>>()?,
    )?;
    let value = StructuredInfoValue::record(
        ty,
        fields
            .iter()
            .map(|(name, value)| {
                StructuredFieldValue::new(
                    *name,
                    StructuredInfoValue::leaf(text.clone(), value.as_bytes().to_vec())?,
                )
            })
            .collect::<Result<_, _>>()?,
    )?;
    StructuredConfigurationValue::new(
        value.value_type().profile()?.value_kind().clone(),
        value.canonical_bytes()?,
    )
    .ok_or(StructuredInfoRefusal::MalformedCanonicalEncoding)
}
