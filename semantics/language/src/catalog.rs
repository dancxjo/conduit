//! Canonical Form catalog for linguistic Info.

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
use conduit_form::{
    KindConfigurationField, KindConfigurationRule, KindProjection, KindSignature,
    StartupParameterSignature,
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
        configuration: vec![KindConfigurationField {
            key: "text".into(),
            default_value: ConfigurationValue::Text(String::new()),
            rule: KindConfigurationRule::TextBytes {
                maximum: MAXIMUM_LINGUISTIC_TEXT_BYTES,
            },
        }],
    }
}

pub fn tokenize_four_semantic_contract() -> Kind {
    let definition = tokenize_four_definition();
    Kind {
        startup_parameters: vec![FrontStartupParameter {
            name: "text".into(),
            value_type: kind_id("value/text"),
            has_default: false,
        }],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
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
        configuration: vec![],
    }
}

pub fn annotate_four_semantic_contract() -> Kind {
    let definition = annotate_four_definition();
    Kind {
        startup_parameters: vec![],
        shorthand: None,
        kind_id: definition.kind_id,
        kind_contract_revision: definition.kind_contract_revision,
        inputs: definition.inputs,
        outputs: definition.outputs,
        configuration: Default::default(),
        semantic_laws: Default::default(),
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
