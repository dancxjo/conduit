//! Exact adopted English scorer selection. Numerical execution remains generic;
//! feature meaning, legal actions and commitment remain checked language Source.
use alloc::{format, string::String, sync::Arc, vec};
use conduit_ai::{integer_categorical::*, integer_categorical_step::PreparedCategoricalStep, *};
use conduit_core::*;
use conduit_data::{TensorAxisRole, TensorElement};

pub const ENGLISH_PARSER_WEIGHTS: &[u8] = include_bytes!("../training/ewt_joint_v2/ewt_joint.i16");
pub const ENGLISH_PARSER_SCHEMA: &[u8] = include_bytes!("../training/ewt_joint_v2/schema.json");
pub const ENGLISH_PARSER_LEXICAL_PROFILE: &str =
    include_str!("../training/ewt_joint_v2/lexical_profile.json");
pub const ENGLISH_PARSER_MODEL_BYTES: u64 = 62_796;
pub const ENGLISH_PARSER_DIMENSIONS: (usize, usize, usize) = (413, 76, 25);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ParserScorerRefusal {
    Absent,
    ForeignModel,
    ForeignSignature,
    ForeignDescriptor,
    ResourceBounds,
    NumericalContract,
}

fn hex(bytes: [u8; 32]) -> String {
    use core::fmt::Write;
    let mut output = String::new();
    for byte in bytes {
        write!(output, "{byte:02x}").expect("String formatting");
    }
    output
}

/// Descriptor used by the accepted #5722 artifact, including its exact schema.
/// This constructs metadata only; it does not adopt content or grant access.
pub fn english_parser_scorer_descriptor() -> (ModelArtifact, ModelSignature) {
    let schema = hex(semantic_digest(
        "language/parser-action-schema@1",
        ENGLISH_PARSER_SCHEMA,
    ));
    let port = |name: &str, element, count| {
        ModelPortConstraint::new(
            ModelPortIdentity::new(name.into()).expect("pinned port"),
            ModelPortPresence::Required,
            ModelSemanticKind::new(format!("language/parser-{name}/{schema}@1"))
                .expect("pinned semantic kind"),
            ModelValueConstraint::tensor(
                ModelTensorConstraint::from_parts(
                    vec![element],
                    vec![ModelAxisConstraint::new(
                        ModelDimensionConstraint::fixed(count).expect("pinned count"),
                        TensorAxisRole::Feature,
                    )
                    .expect("pinned axis")],
                    count * 8,
                )
                .expect("pinned tensor"),
            )
            .expect("pinned constraint"),
        )
        .expect("pinned port constraint")
    };
    let signature = ModelSignature::from_parts(
        format!("language/parser-actions/{schema}@1"),
        1,
        vec![ModelOperation::Infer],
        vec![port("features", TensorElement::U64, 25)],
        vec![port("scores", TensorElement::I64, 76)],
    )
    .expect("pinned signature");
    let digest = model_content_digest(ENGLISH_PARSER_WEIGHTS);
    let artifact = ModelArtifact {
        architecture_profile: CATEGORICAL_I16_ARCHITECTURE.into(),
        format_profile: CATEGORICAL_I16_FORMAT.into(),
        precision_profile: CATEGORICAL_I16_PRECISION.into(),
        state_schema_version: 1,
        signature_identity: signature
            .semantic_digest()
            .expect("pinned signature digest"),
        content: BoundedResourceRef {
            identity: ResourceSemanticIdentity::from_digest(digest),
            content_profile: CATEGORICAL_I16_FORMAT.into(),
            access_class: "language/parser-model/read@1".into(),
            extent: ResourceExtent {
                bytes: ENGLISH_PARSER_MODEL_BYTES,
                items: Some(1),
            },
            lifetime: ResourceLifetime {
                version: ResourceVersionIdentity::from_digest(digest),
                expires_at: None,
            },
        },
    };
    (artifact, signature)
}

/// Select only an already adopted exact artifact. Callers retain the named
/// refusal and choose the deterministic language fallback explicitly.
pub fn prepare_english_parser_scorer(
    resource: Option<Arc<AdmittedModelResource>>,
    pool: ResourcePoolId,
    residence: ResourceContentOffer,
) -> Result<PreparedCategoricalStep, ParserScorerRefusal> {
    let resource = resource.ok_or(ParserScorerRefusal::Absent)?;
    let (artifact, signature) = english_parser_scorer_descriptor();
    if resource.artifact().content_identity() != artifact.content_identity()
        || resource.bytes() != ENGLISH_PARSER_WEIGHTS
    {
        return Err(ParserScorerRefusal::ForeignModel);
    }
    if resource.signature().semantic_digest().ok() != signature.semantic_digest().ok() {
        return Err(ParserScorerRefusal::ForeignSignature);
    }
    // Generic model descriptor identity intentionally omits reference custody.
    // This selected language profile also pins the complete resource reference.
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
