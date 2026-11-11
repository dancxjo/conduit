//! Reuse the installed experience formatter over the exact captured native input.
use super::*;
use conduit_core::validate_canonical_structured_value;
use conduit_semantic_catalog::{
    BoundedExperienceEvidenceProjection, EXPERIENCE_EVIDENCE_MAX_BYTES,
};
pub(super) fn prepare() -> Projector {
    let mut projection = BoundedExperienceEvidenceProjection::prepare();
    let evidence_type = conduit_human::ExperienceEvidenceRequest::semantic_type()
        .unwrap()
        .canonical_bytes()
        .unwrap();
    let experience_type = conduit_chat::episode_experience_type()
        .canonical_bytes()
        .unwrap();
    let mut evidence = Vec::with_capacity(EXPERIENCE_EVIDENCE_MAX_BYTES);
    Box::new(move |input, output| {
        use EpisodeCurrentProjectionRefusal as R;
        if input.len() > conduit_chat::MAXIMUM_EPISODE_EXPERIENCE_BYTES {
            return Err(R::ByteBound);
        }
        let value = validate_canonical_structured_value(input).map_err(|_| R::Malformed)?;
        if value.type_bytes() != experience_type {
            return Err(R::Malformed);
        }
        let node = value
            .record_field("evidence")
            .map_err(|_| R::Malformed)?
            .ok_or(R::Malformed)?
            .value_node();
        if evidence_type.len().saturating_add(node.len()) > EXPERIENCE_EVIDENCE_MAX_BYTES {
            return Err(R::ByteBound);
        }
        evidence.clear();
        evidence.extend_from_slice(&evidence_type);
        evidence.extend_from_slice(node);
        let context = projection.execute(&evidence).map_err(|_| R::Malformed)?;
        super::super::experience_model_request::render_context(context, output, None)
            .map_err(|_| R::ByteBound)
    })
}
