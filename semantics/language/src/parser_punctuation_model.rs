//! Explicit opt-in, unaccepted punctuation-supervised candidate. The accepted
//! V2 descriptor/default remains unchanged; this selects no grammar or beam law.
use crate::parser_model::*;
use alloc::sync::Arc;
use conduit_ai::{integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::*;
pub const PUNCTUATION_PARSER_WEIGHTS: &[u8] =
    include_bytes!("../training/ewt_punctuation_v3/ewt_joint.i16");
/// Same checked feature/action ABI; distinct exact content and resource custody.
pub fn punctuation_parser_scorer_descriptor() -> (ModelArtifact, ModelSignature) {
    let (mut artifact, signature) = english_parser_scorer_descriptor();
    let digest = model_content_digest(PUNCTUATION_PARSER_WEIGHTS);
    artifact.content.identity = ResourceSemanticIdentity::from_digest(digest);
    artifact.content.lifetime.version = ResourceVersionIdentity::from_digest(digest);
    artifact.content.access_class = "language/parser-punctuation-candidate/read@1".into();
    (artifact, signature)
}
pub fn prepare_punctuation_parser_scorer(
    resource: Option<Arc<AdmittedModelResource>>,
    pool: ResourcePoolId,
    residence: ResourceContentOffer,
) -> Result<PreparedCategoricalStep, ParserScorerRefusal> {
    let resource = resource.ok_or(ParserScorerRefusal::Absent)?;
    let (artifact, signature) = punctuation_parser_scorer_descriptor();
    if resource.artifact().content_identity() != artifact.content_identity()
        || resource.bytes() != PUNCTUATION_PARSER_WEIGHTS
    {
        return Err(ParserScorerRefusal::ForeignModel);
    }
    if resource.signature().semantic_digest().ok() != signature.semantic_digest().ok() {
        return Err(ParserScorerRefusal::ForeignSignature);
    }
    if resource.artifact().content != artifact.content
        || resource.descriptor_identity()
            != artifact
                .descriptor_digest(&signature)
                .map_err(|_| ParserScorerRefusal::ForeignDescriptor)?
    {
        return Err(ParserScorerRefusal::ForeignDescriptor);
    }
    let bounds = &residence.contract;
    if bounds.maximum_bytes != ENGLISH_PARSER_MODEL_BYTES as u32
        || bounds.maximum_items != 1
        || bounds.reader_leases == 0
        || bounds.generation_slots != 1
        || bounds.publication_slots != 0
    {
        return Err(ParserScorerRefusal::ResourceBounds);
    }
    let prepared = PreparedCategoricalStep::prepare(resource, pool, residence)
        .map_err(|_| ParserScorerRefusal::ResourceBounds)?;
    if prepared.dimensions() != ENGLISH_PARSER_DIMENSIONS {
        return Err(ParserScorerRefusal::NumericalContract);
    }
    Ok(prepared)
}
