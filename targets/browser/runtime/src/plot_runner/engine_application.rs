//! Retained application Host Call completion over the prepared source instance.
//! This module neither advances the scheduler nor owns another application model.
use super::{debug_error, TourScheduler};
use conduit_core::HostCallRequirement;
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};

pub(super) fn complete_call(
    scheduler: &mut TourScheduler,
    operation: &HostCallRequirement,
    request: HostCallRequest,
) -> Result<bool, String> {
    if operation.contract_id.as_str() == crate::installed_browser::application::STATE_OPERATION {
        let input = scheduler
            .kernel
            .host_value(request.input.value)
            .map_err(debug_error)?;
        let index = usize::from(request.node.0);
        let result = scheduler.applications[index]
            .as_mut()
            .ok_or("browser application state was not prepared before Play")?
            .execute(input);
        let outcome = match result {
            Ok(bytes) => HostCallOutcome {
                disposition: HostCallDisposition::Completed,
                output: Some(
                    BoundedValueRef::new(
                        scheduler
                            .kernel
                            .store_host_value(&bytes)
                            .map_err(debug_error)?,
                        operation.maximum_output_bytes,
                    )
                    .map_err(debug_error)?,
                ),
                failure: None,
            },
            Err(_) => HostCallOutcome {
                disposition: HostCallDisposition::Failed,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::InvalidInput,
                    detail: 70,
                }),
            },
        };
        scheduler
            .kernel
            .complete_host_call(request.node, request.request, outcome)
            .map_err(debug_error)?;
        return Ok(true);
    }
    Ok(false)
}

/// Read the same instance used by the admitted application Host Call. Never
/// clone retained domain state or search for a substitute owner by type.
#[cfg(feature = "creche-surface")]
pub(in crate::plot_runner) fn observe_patchbay(
    scheduler: &TourScheduler,
    owner: conduit_kernel::NodeId,
    body: &conduit_body::BodyId,
    basis: conduit_face::FaceContributionBasis,
) -> Result<PatchbaySourceObservation, String> {
    let port = match scheduler
        .applications
        .get(usize::from(owner.0))
        .and_then(|app| app.as_deref())
    {
        Some(super::super::application_state::PreparedApplication::Patchbay(port)) => port,
        _ => return Err("exact resident Patchbay owner is not prepared".into()),
    };
    let fragment = port
        .port
        .face_fragment(basis, body, false)
        .map_err(|error| format!("Patchbay observation refused: {error:?}"))?;
    Ok(PatchbaySourceObservation {
        revision: port.port.revision(),
        fragment,
        pending_request: port.pending_observation(),
        last_disposition: port.last_disposition(),
        input_refusal: port.input_refusal(),
    })
}

#[cfg(all(test, feature = "creche-surface"))]
#[path = "prepared_patchbay_tests.rs"]
mod retention_tests;

#[cfg(feature = "creche-surface")]
pub(in crate::plot_runner) struct PatchbaySourceObservation {
    pub revision: u32,
    pub fragment: conduit_face::FaceFragment,
    pub pending_request: Option<serde_json::Value>,
    pub last_disposition: Option<serde_json::Value>,
    pub input_refusal: Option<serde_json::Value>,
}
#[cfg(feature = "creche-surface")]
pub(in crate::plot_runner) fn record_patchbay_disposition(
    scheduler: &mut TourScheduler,
    owner: conduit_kernel::NodeId,
    disposition: super::super::patchbay_disposition::RequestDisposition,
) -> Result<(), String> {
    match scheduler
        .applications
        .get_mut(usize::from(owner.0))
        .and_then(|app| app.as_deref_mut())
    {
        Some(super::super::application_state::PreparedApplication::Patchbay(app)) => {
            app.record_disposition(disposition)
        }
        _ => Err("exact resident Patchbay owner is not prepared".into()),
    }
}
