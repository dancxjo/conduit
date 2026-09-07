//! Lowers Crèche-owned graduation meaning into the shared application protocol.

use conduit_creche_model::{GraduationControls, GraduationEvidenceView};
use conduit_presentation::StatusKind;
use serde::Deserialize;

#[derive(Deserialize)]
#[serde(tag = "mode", rename_all = "kebab-case", deny_unknown_fields)]
enum GraduationViewRequest {
    Controls {
        revision: u32,
        durable_identity: bool,
        birth_evidence: bool,
        current_admitted_part: bool,
        ready: bool,
        graduated: bool,
        status: String,
        status_kind: String,
    },
    Evidence {
        revision: u32,
        body_id: String,
        choice: String,
        sign_id: String,
        patchbay_plan_id: Option<String>,
        patchbay_implementation_id: Option<String>,
        creche_required: bool,
        canonical_json: String,
    },
}

fn presentation_view(request: GraduationViewRequest) -> Result<Vec<u8>, String> {
    let semantic = match request {
        GraduationViewRequest::Controls {
            revision,
            durable_identity,
            birth_evidence,
            current_admitted_part,
            ready,
            graduated,
            status,
            status_kind,
        } => GraduationControls {
            revision,
            durable_identity,
            birth_evidence,
            current_admitted_part,
            ready,
            graduated,
            status,
            status_kind: match status_kind.as_str() {
                "ordinary" => StatusKind::Ordinary,
                "failure" => StatusKind::Failure,
                "success" => StatusKind::Success,
                _ => return Err("UnknownGraduationStatusKind".into()),
            },
        }
        .presentation(),
        GraduationViewRequest::Evidence {
            revision,
            body_id,
            choice,
            sign_id,
            patchbay_plan_id,
            patchbay_implementation_id,
            creche_required,
            canonical_json,
        } => GraduationEvidenceView {
            revision,
            body_id,
            choice,
            sign_id,
            patchbay_plan_id,
            patchbay_implementation_id,
            creche_required,
            canonical_json,
        }
        .presentation(),
    }
    .map_err(|error| format!("describe Crèche graduation: {error:?}"))?;
    semantic
        .lower()
        .map_err(|error| format!("lower Crèche graduation: {error:?}"))?
        .encode()
        .map_err(|error| format!("encode Crèche graduation: {error:?}"))
}

#[no_mangle]
pub extern "C" fn conduit_creche_graduation_view(length: usize) -> i32 {
    super::abi::clear_output();
    let bytes = match super::abi::take_input(length) {
        Ok(bytes) => bytes,
        Err(code) => return code,
    };
    match serde_json::from_slice::<GraduationViewRequest>(&bytes)
        .map_err(|error| format!("InvalidGraduationPresentation: {error}"))
        .and_then(presentation_view)
        .and_then(|encoded| {
            super::abi::write_output_bytes(&encoded)
                .map_err(|_| "GraduationPresentationOutputBound".to_string())
        }) {
        Ok(()) => 0,
        Err(message) => super::abi::refuse(message, super::abi::ERROR_GRADUATION),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn graduation_views_lower_through_shared_semantics() {
        let controls = presentation_view(GraduationViewRequest::Controls {
            revision: 1,
            durable_identity: true,
            birth_evidence: true,
            current_admitted_part: true,
            ready: true,
            graduated: false,
            status: "Ready".into(),
            status_kind: "ordinary".into(),
        })
        .unwrap();
        let controls = conduit_presentation::ApplicationView::decode(&controls).unwrap();
        assert_eq!(controls.actions.len(), 2);
        assert!(controls
            .nodes
            .iter()
            .any(|node| node.key == "graduation-status"));

        let evidence = presentation_view(GraduationViewRequest::Evidence {
            revision: 2,
            body_id: "body/1".into(),
            choice: "without-patchbay".into(),
            sign_id: "sign/1".into(),
            patchbay_plan_id: None,
            patchbay_implementation_id: None,
            creche_required: false,
            canonical_json: "{}".into(),
        })
        .unwrap();
        let evidence = conduit_presentation::ApplicationView::decode(&evidence).unwrap();
        assert!(evidence
            .nodes
            .iter()
            .any(|node| node.key == "graduation-raw-json"));
    }
}
