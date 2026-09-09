//! Inspection of the Tour's fixed specimen from its portable Kind contracts.
use alloc::{format, vec::Vec};
use conduit_presentation::{PresentationRole, PresentationSubject, PresentationText};
use conduit_semantic_catalog::{
    text_literal_contract, text_presentation_contract, text_upper_contract,
};

use crate::TourWorkspaceState;

pub(crate) fn fields(
    state: &TourWorkspaceState,
    gear: &str,
    subjects: &mut Vec<PresentationSubject>,
) -> Vec<PresentationText> {
    let contract = match gear {
        "meet-one-gear/words" => text_literal_contract(),
        "meet-one-gear/change" => text_upper_contract(),
        _ => text_presentation_contract(),
    };
    let mut text = Vec::new();
    let mut field = |key: &str, label: &str, value: alloc::string::String| {
        let identity = format!("{gear}/inspection/{key}");
        subjects.push(PresentationSubject {
            identity: identity.clone(),
            role: PresentationRole::Info,
            label: label.into(),
            accessibility_name: label.into(),
        });
        text.push(PresentationText {
            subject: identity,
            text: value,
        });
    };
    field("kind", "Kind", contract.kind_id.as_str().into());
    field("face", "Face", contract.plain_name);
    field(
        "implementation",
        "Implementation",
        "Not supplied by workspace".into(),
    );
    field("placement", "Placement", "Not supplied by workspace".into());
    let mut ports = alloc::string::String::new();
    for (direction, descriptors) in [("in", &contract.inputs), ("out", &contract.outputs)] {
        for port in descriptors {
            if !ports.is_empty() {
                ports.push('\n');
            }
            ports.push_str(&format!(
                "{direction} {}  {}",
                port.port_id.as_str(),
                port.value_kind.as_str()
            ));
        }
    }
    field("ports", "Ports", ports);
    field(
        "state",
        "Current state",
        "Per-Port observations unavailable".into(),
    );
    field(
        "play",
        "Plan / Play / Signs",
        if state.run_pending {
            "Run requested".into()
        } else if let Some(result) = &state.result {
            format!("Tour result: {result}\nPer-Gear Signs unavailable")
        } else {
            "No Tour result recorded".into()
        },
    );
    field("documentation", "Documentation", contract.summary);
    text
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{CANONICAL_PATCHBAY_GEARS, TourWorkspacePhase};

    #[test]
    fn every_specimen_gear_uses_its_own_contract_and_never_invents_port_values() {
        let mut state = TourWorkspaceState::canonical(1, TourWorkspacePhase::PatchbayOpen);
        for (gear, kind) in CANONICAL_PATCHBAY_GEARS.into_iter().zip([
            "text/literal",
            "text/upper",
            "presentation/text",
        ]) {
            state.selected_patchbay_subject = Some(gear.into());
            let presentation = state.inspector_presentation().unwrap().unwrap();
            assert_eq!(presentation.text[0].text, kind);
            assert!(
                presentation
                    .text
                    .iter()
                    .any(|field| field.text == "Per-Port observations unavailable")
            );
        }
        state.result = Some("observed result".into());
        let presentation = state.inspector_presentation().unwrap().unwrap();
        assert!(
            presentation
                .text
                .iter()
                .any(|field| field.text.contains("Tour result: observed result"))
        );
    }
}
