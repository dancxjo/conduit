//! An inspectable preparation result, not a numerical correctness certificate.
use conduit_core::{
    kind_id, StructuredConfigurationValue, StructuredFieldType, StructuredFieldValue,
    StructuredInfoRefusal, StructuredInfoType, StructuredInfoValue,
};

pub(super) fn property(tape_refusal: &str) -> Result<StructuredConfigurationValue, String> {
    if tape_refusal.len() > 1024 {
        return Err("expression eligibility evidence exceeds 1024 bytes".into());
    }
    let build = || -> Result<_, StructuredInfoRefusal> {
        let text = StructuredInfoType::leaf(kind_id("value/text"))?;
        let ty = StructuredInfoType::record(
            kind_id("std/source-expression-selection@1"),
            vec![StructuredFieldType::new(
                "tape-profile-refusal",
                text.clone(),
            )?],
        )?;
        let value = StructuredInfoValue::record(
            ty,
            vec![StructuredFieldValue::new(
                "tape-profile-refusal",
                StructuredInfoValue::leaf(text, tape_refusal.as_bytes().to_vec())?,
            )?],
        )?;
        Ok((
            value.value_type().profile()?.value_kind().clone(),
            value.canonical_bytes()?,
        ))
    };
    let (profile, bytes) =
        build().map_err(|why| format!("expression eligibility evidence: {why:?}"))?;
    StructuredConfigurationValue::new(profile, bytes)
        .ok_or("invalid expression eligibility evidence".into())
}
