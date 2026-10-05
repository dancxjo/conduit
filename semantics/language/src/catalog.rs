//! Canonical Plot catalog for linguistic Info.

use alloc::{
    string::{String, ToString},
    vec,
    vec::Vec,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, ConfigurationValue, FrontStartupParameter, Kind,
    KindIdentity, PortDescriptor, PortDirection, PortTemporal, StructuredInfoType,
    MAXIMUM_STRUCTURED_CANONICAL_BYTES,
};
use conduit_plot::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature,
    StartupParameterSignature,
};

use crate::{
    analysis_revision_type, analysis_token_ref_type, annotation_bundle_four_type,
    dependency_arc_type, dependency_edge_type, dependency_head_type, dependency_subtype_type,
    language_dependency_relation_type, language_request_field, language_request_parameter,
    language_request_signature, language_requirement_laws, linguistic_annotation_type,
    linguistic_annotations_four_type, linguistic_label_type, linguistic_segment_type,
    linguistic_token_type, linguistic_tokens_four_type, text_span_type,
    universal_dependency_relation_type, ANALYSIS_REVISION_TYPE, ANALYSIS_TOKEN_REF_TYPE,
    ANNOTATION_BUNDLE_FOUR_TYPE, DEPENDENCY_ARC_TYPE, DEPENDENCY_EDGE_TYPE, DEPENDENCY_HEAD_TYPE,
    DEPENDENCY_SUBTYPE_TYPE, LANGUAGE_DEPENDENCY_RELATION_TYPE, LINGUISTIC_ANNOTATIONS_FOUR_TYPE,
    LINGUISTIC_ANNOTATION_TYPE, LINGUISTIC_LABEL_TYPE, LINGUISTIC_SEGMENT_TYPE,
    LINGUISTIC_TOKENS_FOUR_TYPE, LINGUISTIC_TOKEN_TYPE, MAXIMUM_LINGUISTIC_TEXT_BYTES,
    TEXT_SPAN_TYPE, UNIVERSAL_DEPENDENCY_RELATION_TYPE,
};

pub const TOKENIZE_FOUR_KIND: &str = "language/tokenize-four";
pub const ANNOTATE_FOUR_KIND: &str = "language/annotate-four";
pub const LINGUISTICS_REVISION: &str = "conduit.language/linguistics@2";

pub fn install_linguistics_catalogs(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), String> {
    for (name, value_type) in crate::identity_types()
        .into_iter()
        .chain(crate::realization_types())
        .chain(linguistic_types())
    {
        if let Some(existing) = startup.structured_type(name) {
            if existing != &value_type {
                return Err(alloc::format!(
                    "{name} differs from the Language-owned native schema"
                ));
            }
        } else {
            startup
                .insert_structured_type(name, value_type)
                .map_err(|error| error.to_string())?;
        }
    }
    startup
        .insert(KindSignature {
            kind: TOKENIZE_FOUR_KIND.into(),
            startup_parameters: vec![
                StartupParameterSignature {
                    name: "text".into(),
                    value_type: "Text".into(),
                    default: None,
                },
                language_request_signature(),
            ],
        })
        .map_err(|error| error.to_string())?;
    startup
        .insert(KindSignature {
            kind: ANNOTATE_FOUR_KIND.into(),
            startup_parameters: vec![language_request_signature()],
        })
        .map_err(|error| error.to_string())?;

    profile
        .insert_kind(tokenize_four_semantic_contract())
        .map_err(|error| error.to_string())?;
    profile
        .insert_kind(annotate_four_semantic_contract())
        .map_err(|error| error.to_string())
}

pub fn tokenize_four_definition() -> KindProjection {
    KindProjection {
        kind_id: kind_id(TOKENIZE_FOUR_KIND),
        kind_contract_revision: KindIdentity::from(LINGUISTICS_REVISION),
        inputs: vec![],
        outputs: vec![port(
            "tokens",
            &linguistic_tokens_four_type(),
            PortDirection::Output,
        )],
        configuration: vec![
            language_request_field(),
            KindConfigurationField {
                key: "text".into(),
                default_value: ConfigurationValue::Text(String::new()),
                rule: KindConfigurationRule::TextBytes {
                    maximum: MAXIMUM_LINGUISTIC_TEXT_BYTES,
                },
            },
        ],
    }
}

pub fn tokenize_four_semantic_contract() -> Kind {
    let definition = tokenize_four_definition();
    Kind {
        startup_parameters: vec![
            FrontStartupParameter {
                name: "text".into(),
                value_type: kind_id("value/text"),
                has_default: false,
            },
            language_request_parameter(),
        ],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: definition.configuration,
        semantic_laws: language_requirement_laws(),
        limits: linguistic_limits(),
    }
}

pub fn annotate_four_definition() -> KindProjection {
    KindProjection {
        kind_id: kind_id(ANNOTATE_FOUR_KIND),
        kind_contract_revision: KindIdentity::from(LINGUISTICS_REVISION),
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
        configuration: vec![language_request_field()],
    }
}

pub fn annotate_four_semantic_contract() -> Kind {
    let definition = annotate_four_definition();
    Kind {
        startup_parameters: vec![language_request_parameter()],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: definition.configuration,
        semantic_laws: language_requirement_laws(),
        limits: linguistic_limits(),
    }
}

fn linguistic_limits() -> CapabilityLimits {
    CapabilityLimits {
        max_active_instances: 8,
        max_queue_items: 4,
        max_queue_bytes: (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32,
    }
}

fn linguistic_types() -> Vec<(&'static str, StructuredInfoType)> {
    vec![
        (
            UNIVERSAL_DEPENDENCY_RELATION_TYPE,
            universal_dependency_relation_type(),
        ),
        (
            LANGUAGE_DEPENDENCY_RELATION_TYPE,
            language_dependency_relation_type(),
        ),
        (DEPENDENCY_SUBTYPE_TYPE, dependency_subtype_type()),
        (ANALYSIS_REVISION_TYPE, analysis_revision_type()),
        (ANALYSIS_TOKEN_REF_TYPE, analysis_token_ref_type()),
        (DEPENDENCY_HEAD_TYPE, dependency_head_type()),
        (DEPENDENCY_ARC_TYPE, dependency_arc_type()),
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
        abnormal_kind: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn four_item_linguistics_contracts_own_their_required_queue_capacity() {
        for contract in [
            tokenize_four_semantic_contract(),
            annotate_four_semantic_contract(),
        ] {
            assert_eq!(contract.limits.max_active_instances, 8);
            assert_eq!(contract.limits.max_queue_items, 4);
            assert_eq!(
                contract.limits.max_queue_bytes,
                (MAXIMUM_STRUCTURED_CANONICAL_BYTES * 4) as u32
            );
        }
    }
}
