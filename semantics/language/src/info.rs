//! Finite host-neutral linguistic Info.

use conduit_core::{StructuredInfoRefusal, StructuredInfoType};
use conduit_plot::rust_binding::record_field_type;

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
    NativeBinding(conduit_plot::rust_binding::NativeBindingRefusal),
}

impl From<StructuredInfoRefusal> for LinguisticRefusal {
    fn from(value: StructuredInfoRefusal) -> Self {
        Self::Structured(value)
    }
}

impl From<conduit_plot::rust_binding::NativeBindingRefusal> for LinguisticRefusal {
    fn from(value: conduit_plot::rust_binding::NativeBindingRefusal) -> Self {
        Self::NativeBinding(value)
    }
}

macro_rules! semantic_type {
    ($function:ident, $native:ty, $description:literal) => {
        pub fn $function() -> StructuredInfoType {
            <$native>::semantic_type().expect($description)
        }
    };
}

semantic_type!(
    offset_basis_type,
    crate::LinguisticOffsetBasis,
    "checked native linguistic offset-basis Type"
);
semantic_type!(
    text_span_type,
    crate::TextSpan,
    "checked native linguistic text-span Type"
);
semantic_type!(
    token_identity_type,
    crate::LinguisticTokenIdentity,
    "checked native linguistic token-identity Type"
);
semantic_type!(
    optional_text_type,
    crate::LinguisticOptionalText,
    "checked native linguistic optional-text Type"
);
semantic_type!(
    token_category_type,
    crate::LinguisticTokenCategory,
    "checked native linguistic token-category Type"
);
semantic_type!(
    feature_slot_type,
    crate::LinguisticTokenFeatureSlot,
    "checked native linguistic feature-slot Type"
);
semantic_type!(
    linguistic_token_type,
    crate::LinguisticToken,
    "checked native linguistic token Type"
);
semantic_type!(
    segment_kind_type,
    crate::LinguisticSegmentKind,
    "checked native linguistic segment-kind Type"
);
semantic_type!(
    linguistic_segment_type,
    crate::LinguisticSegment,
    "checked native linguistic segment Type"
);
semantic_type!(
    provenance_type,
    crate::LinguisticDerivationProvenance,
    "checked native linguistic provenance Type"
);
semantic_type!(
    linguistic_tokens_four_type,
    crate::LinguisticTokensFour,
    "checked native linguistic token bundle Type"
);
semantic_type!(
    linguistic_annotation_type,
    crate::LinguisticAnnotation,
    "checked native linguistic annotation Type"
);
semantic_type!(
    dependency_relation_type,
    crate::LinguisticDependencyRelation,
    "checked native linguistic dependency-relation Type"
);
semantic_type!(
    dependency_edge_type,
    crate::LinguisticDependencyEdge,
    "checked native linguistic dependency-edge Type"
);
semantic_type!(
    annotation_bundle_four_type,
    crate::AnnotationBundleFour,
    "checked native linguistic annotation bundle Type"
);

pub fn linguistic_label_type() -> StructuredInfoType {
    record_field_type(&linguistic_annotation_type(), "label")
        .expect("checked linguistic label field")
}

pub fn linguistic_annotations_four_type() -> StructuredInfoType {
    record_field_type(&annotation_bundle_four_type(), "annotations")
        .expect("checked annotation collection field")
}
