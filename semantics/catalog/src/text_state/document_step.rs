//! One pure application of the existing revisioned editing law. Retention and
//! adoption belong to its calling Plot, never to a Mask or storage provider.
use super::*;
use conduit_core::{CheckedValueContract, FrontValueContract, FrontValueLocation, KindSemanticLaw};

pub const TEXT_APPLY_EDIT_KIND: &str = "text/apply-edit";
pub const TEXT_APPLY_EDIT_REVISION: &str = "conduit.text/apply-edit@1";

/// Derive a candidate document without mutating either input. The caller adopts
/// it only after its checked Step commits; refusal leaves output untouched.
pub fn apply_document_edit(
    document: &[u8],
    edit: &[u8],
    output: &mut [u8],
) -> Result<usize, TextEditRefusal> {
    let document = TextDocument::decode(document)?;
    let edit = TextEdit::decode(edit)?;
    let mut storage = [0; MAXIMUM_EDITED_TEXT_BYTES as usize];
    let mut owner = retained::RetainedText::from_snapshot(
        storage.len(),
        &mut storage,
        document.revision,
        document.text,
    )?;
    owner.replace(&mut storage, edit)?;
    TextDocument {
        revision: owner.revision(),
        text: owner.current(&storage)?,
    }
    .encode_into(output)
}

pub fn text_apply_edit_semantic_contract() -> Kind {
    let port = |name: &str, kind: &str, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
        abnormal_kind: None,
    };
    let value = |name: &str, kind: &str, bytes, input| FrontValueContract {
        location: if input {
            FrontValueLocation::Input(port_id(name))
        } else {
            FrontValueLocation::Output(port_id(name))
        },
        contract: CheckedValueContract::new(kind_id(kind), bytes, vec![])
            .expect("finite document value"),
    };
    Kind {
        kind_id: kind_id(TEXT_APPLY_EDIT_KIND),
        kind_contract_revision: TEXT_APPLY_EDIT_REVISION.into(),
        startup_parameters: vec![],
        shorthand: None,
        configuration: vec![],
        inputs: vec![
            port("document", TEXT_DOCUMENT_VALUE_KIND, PortDirection::Input),
            port("edit", TEXT_EDIT_VALUE_KIND, PortDirection::Input),
        ],
        outputs: vec![port(
            "current",
            TEXT_DOCUMENT_VALUE_KIND,
            PortDirection::Output,
        )],
        semantic_laws: vec![
            KindSemanticLaw::ValueContracts(vec![
                value(
                    "document",
                    TEXT_DOCUMENT_VALUE_KIND,
                    MAXIMUM_TEXT_DOCUMENT_BYTES,
                    true,
                ),
                value("edit", TEXT_EDIT_VALUE_KIND, MAXIMUM_TEXT_EDIT_BYTES, true),
                value(
                    "current",
                    TEXT_DOCUMENT_VALUE_KIND,
                    MAXIMUM_TEXT_DOCUMENT_BYTES,
                    false,
                ),
            ]),
            KindSemanticLaw::Terminal(KindTerminalBehavior::CompletesWhenInputsClose),
        ],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 3,
            max_queue_bytes: 2 * MAXIMUM_TEXT_DOCUMENT_BYTES + MAXIMUM_TEXT_EDIT_BYTES,
        },
    }
}

#[cfg(feature = "plot-catalog")]
pub(super) fn install(
    startup: &mut conduit_plot::StartupCatalog,
    profile: &mut conduit_plot::ProfileCatalog,
) -> Result<(), alloc::string::String> {
    startup.insert(conduit_plot::KindSignature {
        kind: TEXT_APPLY_EDIT_KIND.into(),
        startup_parameters: vec![],
    })?;
    profile
        .insert_kind(text_apply_edit_semantic_contract())
        .map_err(|error| error.to_string())
}
