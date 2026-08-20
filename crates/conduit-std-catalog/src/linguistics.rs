//! Finite host-neutral linguistic Info and deterministic reference realizations.

use alloc::{vec, vec::Vec};
use conduit_core::{
    kind_id, StructuredFieldType, StructuredInfoRefusal, StructuredInfoType, StructuredVariantCase,
};

pub const TEXT_SPAN_TYPE: &str = "TextSpan";
pub const LINGUISTIC_TOKEN_TYPE: &str = "LinguisticToken";
pub const LINGUISTIC_TOKENS_FOUR_TYPE: &str = "LinguisticTokensFour";
pub const LINGUISTIC_SEGMENT_TYPE: &str = "LinguisticSegment";
pub const LINGUISTIC_ANNOTATION_TYPE: &str = "LinguisticAnnotation";
pub const LINGUISTIC_ANNOTATIONS_FOUR_TYPE: &str = "LinguisticAnnotationsFour";
pub const LINGUISTIC_LABEL_TYPE: &str = "LinguisticLabel";
pub const DEPENDENCY_EDGE_TYPE: &str = "LinguisticDependencyEdge";
pub const ANNOTATION_BUNDLE_FOUR_TYPE: &str = "AnnotationBundleFour";
pub const LINGUISTIC_TOKEN_COUNT: u16 = 4;
pub const LINGUISTIC_FEATURE_SLOTS: u16 = 2;
pub const LINGUISTIC_DEPENDENCY_COUNT: u16 = 3;
pub const MAXIMUM_LINGUISTIC_TEXT_BYTES: u32 = 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LinguisticRefusal {
    TextTooLarge,
    WrongTokenCount { expected: u16, actual: usize },
    MalformedInfo,
    Structured(StructuredInfoRefusal),
}

impl From<StructuredInfoRefusal> for LinguisticRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

pub(crate) fn leaf(kind: &str) -> StructuredInfoType {
    StructuredInfoType::leaf(kind_id(kind)).expect("reviewed linguistic leaf")
}

fn field(name: &str, value_type: StructuredInfoType) -> StructuredFieldType {
    StructuredFieldType::new(name, value_type).expect("reviewed linguistic field")
}

fn case(name: &str, payload_type: StructuredInfoType) -> StructuredVariantCase {
    StructuredVariantCase::new(name, payload_type).expect("reviewed linguistic case")
}

fn record(kind: &str, fields: Vec<StructuredFieldType>) -> StructuredInfoType {
    StructuredInfoType::record(kind_id(kind), fields).expect("reviewed linguistic record")
}

pub(crate) fn bounded(value_type: StructuredInfoType, length: u16) -> StructuredInfoType {
    StructuredInfoType::collection(value_type, Some(length))
        .expect("reviewed linguistic collection")
}

fn unit_type() -> StructuredInfoType {
    leaf("value/unit@1")
}

fn text_type() -> StructuredInfoType {
    leaf("value/text@1")
}

fn count_type() -> StructuredInfoType {
    leaf("value/count@1")
}

pub fn offset_basis_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("language/offset-basis@1"),
        vec![
            case("unicode_scalar", unit_type()),
            case("utf8_byte", unit_type()),
        ],
    )
    .expect("reviewed offset bases")
}

pub fn text_span_type() -> StructuredInfoType {
    record(
        "language/text-span@1",
        vec![
            field("basis", offset_basis_type()),
            field("end", count_type()),
            field("start", count_type()),
            field("text_identity", text_type()),
        ],
    )
}

pub fn token_identity_type() -> StructuredInfoType {
    record(
        "language/token-identity@1",
        vec![
            field("ordinal", count_type()),
            field("text_identity", text_type()),
        ],
    )
}

pub(crate) fn optional_text_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("language/optional-text@1"),
        vec![case("absent", unit_type()), case("present", text_type())],
    )
    .expect("reviewed optional text")
}

pub(crate) fn token_category_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("language/token-category@1"),
        vec![case("punctuation", unit_type()), case("word", unit_type())],
    )
    .expect("reviewed token categories")
}

fn feature_type() -> StructuredInfoType {
    record(
        "language/token-feature@1",
        vec![field("name", text_type()), field("value", text_type())],
    )
}

pub(crate) fn feature_slot_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("language/token-feature-slot@1"),
        vec![case("feature", feature_type()), case("unused", unit_type())],
    )
    .expect("reviewed feature slot")
}

pub fn linguistic_token_type() -> StructuredInfoType {
    record(
        "language/token@1",
        vec![
            field("category", token_category_type()),
            field(
                "features",
                bounded(feature_slot_type(), LINGUISTIC_FEATURE_SLOTS),
            ),
            field("identity", token_identity_type()),
            field("lemma", optional_text_type()),
            field("span", text_span_type()),
            field("surface", text_type()),
        ],
    )
}

pub(crate) fn segment_kind_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("language/segment-kind@1"),
        vec![case("sentence", unit_type())],
    )
    .expect("reviewed segment kinds")
}

pub fn linguistic_segment_type() -> StructuredInfoType {
    record(
        "language/segment@1",
        vec![
            field("identity", text_type()),
            field("kind", segment_kind_type()),
            field("span", text_span_type()),
        ],
    )
}

pub fn provenance_type() -> StructuredInfoType {
    let evidence = |kind| {
        record(
            kind,
            vec![
                field("implementation", text_type()),
                field("revision", text_type()),
            ],
        )
    };
    StructuredInfoType::variant(
        kind_id("language/derivation-provenance@1"),
        vec![
            case("deterministic_rule", evidence("language/rule-evidence@1")),
            case("library", evidence("language/library-evidence@1")),
            case("model", evidence("language/model-evidence@1")),
        ],
    )
    .expect("reviewed provenance cases")
}

pub fn linguistic_tokens_four_type() -> StructuredInfoType {
    record(
        "language/token-sequence-four@1",
        vec![
            field("provenance", provenance_type()),
            field("segments", bounded(linguistic_segment_type(), 1)),
            field(
                "tokens",
                bounded(linguistic_token_type(), LINGUISTIC_TOKEN_COUNT),
            ),
        ],
    )
}

pub fn linguistic_annotation_type() -> StructuredInfoType {
    record(
        "language/span-annotation@1",
        vec![field("label", text_type()), field("span", text_span_type())],
    )
}

pub fn linguistic_label_type() -> StructuredInfoType {
    text_type()
}

pub(crate) fn dependency_relation_type() -> StructuredInfoType {
    StructuredInfoType::variant(
        kind_id("language/dependency-relation@1"),
        vec![
            case("modifier", unit_type()),
            case("punctuation", unit_type()),
            case("subject", unit_type()),
        ],
    )
    .expect("reviewed dependency relations")
}

pub fn dependency_edge_type() -> StructuredInfoType {
    record(
        "language/dependency-edge@1",
        vec![
            field("dependent", token_identity_type()),
            field("governor", token_identity_type()),
            field("relation", dependency_relation_type()),
        ],
    )
}

pub fn linguistic_annotations_four_type() -> StructuredInfoType {
    bounded(linguistic_annotation_type(), LINGUISTIC_TOKEN_COUNT)
}

pub fn annotation_bundle_four_type() -> StructuredInfoType {
    record(
        "language/annotation-bundle-four@1",
        vec![
            field("annotations", linguistic_annotations_four_type()),
            field(
                "dependencies",
                bounded(dependency_edge_type(), LINGUISTIC_DEPENDENCY_COUNT),
            ),
            field("provenance", provenance_type()),
        ],
    )
}
