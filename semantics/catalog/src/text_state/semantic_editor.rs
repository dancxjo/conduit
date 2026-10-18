//! Semantic text encounters with committed replacement input; no Host machinery.
use super::*;
use conduit_core::{KindSemanticLaw, KindTerminalBehavior};

pub const TEXT_EDIT_DOCUMENT_KIND: &str = "text/edit-document";
pub const TEXT_EDIT_DOCUMENT_REVISION: &str = "conduit.text/edit-document@1";
pub const FACE_EDITOR_KIND: &str = "face/editor";
pub const FACE_EDITOR_REVISION: &str = "conduit.face/editor@1";

fn port(name: &str, kind: &str, direction: PortDirection) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Flow { closes: false },
        abnormal_kind: None,
    }
}

pub fn text_edit_document_semantic_contract() -> Kind {
    let mut contract = text_edit_semantic_contract();
    contract.kind_id = kind_id(TEXT_EDIT_DOCUMENT_KIND);
    contract.kind_contract_revision = TEXT_EDIT_DOCUMENT_REVISION.into();
    contract.inputs = vec![port("edit", TEXT_EDIT_VALUE_KIND, PortDirection::Input)];
    contract.outputs = vec![port(
        "current",
        TEXT_DOCUMENT_VALUE_KIND,
        PortDirection::Output,
    )];
    contract.limits.max_queue_bytes = MAXIMUM_TEXT_EDIT_BYTES;
    contract.semantic_laws.push(KindSemanticLaw::TemporalState(
        conduit_core::TemporalStateBehavior::Retained,
    ));
    contract
        .semantic_laws
        .push(KindSemanticLaw::InitializedState(
            conduit_core::InitializedStateLaw {
                update_port_id: port_id("edit"),
                current_port_id: port_id("current"),
                current: conduit_core::CheckedValueContract::new(
                    kind_id(TEXT_DOCUMENT_VALUE_KIND),
                    MAXIMUM_TEXT_DOCUMENT_BYTES,
                    vec![],
                )
                .expect("finite document contract"),
                initial: vec![0; 8],
            },
        ));
    contract
}

pub fn face_editor_semantic_contract() -> Kind {
    let configuration = crate::data_label_configuration();
    Kind {
        kind_id: kind_id(FACE_EDITOR_KIND),
        kind_contract_revision: FACE_EDITOR_REVISION.into(),
        startup_parameters: crate::startup_front(&configuration),
        shorthand: None,
        inputs: vec![port(
            "document",
            TEXT_DOCUMENT_VALUE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![port("edit", TEXT_EDIT_VALUE_KIND, PortDirection::Output)],
        configuration,
        semantic_laws: vec![KindSemanticLaw::Terminal(
            KindTerminalBehavior::MirrorsInputTerminal,
        )],
        limits: CapabilityLimits {
            max_active_instances: 8,
            max_queue_items: 1,
            max_queue_bytes: MAXIMUM_TEXT_EDIT_BYTES,
        },
    }
}

#[cfg(feature = "plot-catalog")]
pub(super) fn install(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    use conduit_plot::{KindSignature, StartupParameterSignature};
    startup.insert(KindSignature {
        kind: TEXT_EDIT_DOCUMENT_KIND.into(),
        startup_parameters: vec![StartupParameterSignature {
            name: "maximum-bytes".into(),
            value_type: "Count".into(),
            default: Some("256".into()),
        }],
    })?;
    startup.insert(KindSignature {
        kind: FACE_EDITOR_KIND.into(),
        startup_parameters: crate::data_label_startup_signature(),
    })?;
    for contract in [
        text_edit_document_semantic_contract(),
        face_editor_semantic_contract(),
    ] {
        profile
            .insert_kind(contract)
            .map_err(|error| error.to_string())?;
    }
    Ok(())
}
