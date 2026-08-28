//! Canonical Form catalog for linguistic Info.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, ConfigurationValue, KindContractRevision, PortDescriptor, PortDirection,
    PortTemporal, StructuredInfoType,
};
use conduit_form::{
    ConfigurationField, ConfigurationRule, KindDefinition, KindSignature, StartupParameterSignature,
};

use crate::{
    annotation_bundle_four_type, dependency_edge_type, linguistic_annotation_type,
    linguistic_annotations_four_type, linguistic_label_type, linguistic_segment_type,
    linguistic_token_type, linguistic_tokens_four_type, text_span_type,
    ANNOTATION_BUNDLE_FOUR_TYPE, DEPENDENCY_EDGE_TYPE, LINGUISTIC_ANNOTATIONS_FOUR_TYPE,
    LINGUISTIC_ANNOTATION_TYPE, LINGUISTIC_LABEL_TYPE, LINGUISTIC_SEGMENT_TYPE,
    LINGUISTIC_TOKENS_FOUR_TYPE, LINGUISTIC_TOKEN_TYPE, MAXIMUM_LINGUISTIC_TEXT_BYTES,
    TEXT_SPAN_TYPE,
};

pub const TOKENIZE_FOUR_KIND: &str = "language/tokenize-four";
pub const ANNOTATE_FOUR_KIND: &str = "language/annotate-four";
pub const LINGUISTICS_REVISION: &str = "conduit.std/linguistics@1";

pub fn install_linguistics_catalogs(
    startup: &mut conduit_form::StartupCatalog,
    profile: &mut conduit_form::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in linguistic_types() {
        startup
            .insert_structured_type(name, value_type)
            .map_err(|error| error.to_string())?;
    }
    startup
        .insert(KindSignature {
            kind: TOKENIZE_FOUR_KIND.into(),
            startup_parameters: vec![StartupParameterSignature {
                name: "text".into(),
                value_type: "Text".into(),
                default: None,
            }],
        })
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: ANNOTATE_FOUR_KIND.into(),
            startup_parameters: vec![],
        })
        .map_err(|error| error.to_string())?;

    profile
        .insert(tokenize_four_definition())
        .map_err(|error| error.to_string())?;
    profile
        .insert(annotate_four_definition())
        .map_err(|error| error.to_string())
}

pub fn tokenize_four_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(TOKENIZE_FOUR_KIND),
        kind_contract_revision: KindContractRevision::from(LINGUISTICS_REVISION),
        inputs: vec![],
        outputs: vec![port(
            "tokens",
            &linguistic_tokens_four_type(),
            PortDirection::Output,
        )],
        configuration: vec![ConfigurationField {
            key: "text".into(),
            default_value: ConfigurationValue::Text(String::new()),
            validation: ConfigurationRule::TextBytes {
                maximum: MAXIMUM_LINGUISTIC_TEXT_BYTES,
            },
        }],
    }
}

pub fn annotate_four_definition() -> KindDefinition {
    KindDefinition {
        kind_id: kind_id(ANNOTATE_FOUR_KIND),
        kind_contract_revision: KindContractRevision::from(LINGUISTICS_REVISION),
        inputs: vec![port(
            "tokens",
            &linguistic_tokens_four_type(),
            PortDirection::Input,
        )],
        outputs: vec![port(
            "annotations",
            &annotation_bundle_four_type(),
            PortDirection::Output,
        )],
        configuration: vec![],
    }
}

fn linguistic_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (TEXT_SPAN_TYPE, text_span_type()),
        (LINGUISTIC_TOKEN_TYPE, linguistic_token_type()),
        (LINGUISTIC_TOKENS_FOUR_TYPE, linguistic_tokens_four_type()),
        (LINGUISTIC_SEGMENT_TYPE, linguistic_segment_type()),
        (LINGUISTIC_ANNOTATION_TYPE, linguistic_annotation_type()),
        (LINGUISTIC_LABEL_TYPE, linguistic_label_type()),
        (
            LINGUISTIC_ANNOTATIONS_FOUR_TYPE,
            linguistic_annotations_four_type(),
        ),
        (DEPENDENCY_EDGE_TYPE, dependency_edge_type()),
        (ANNOTATION_BUNDLE_FOUR_TYPE, annotation_bundle_four_type()),
    ]
}

fn port(name: &str, value_type: &StructuredInfoType, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: value_type
            .profile()
            .expect("bounded linguistic type")
            .value_kind()
            .clone(),
        direction,
        temporal: PortTemporal::Value,
    }
}
