//! Browser presence transport for explicit owner wardrobe inspection and Apply.
use super::{transport::Socket, Out, PROTOCOL};
use conduit_core::{LinkBindingId, PlanId};
use conduit_presentation::{MaskWardrobeAction, OwnerFaceSnapshotRequest};
use std::path::Path;

#[allow(clippy::too_many_arguments)]
pub(super) fn respond(
    socket: &mut Socket,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
    binding: &LinkBindingId,
    request_id: String,
    request: OwnerFaceSnapshotRequest,
    owner_plan_id: Option<PlanId>,
    basis_revision: u64,
    action: Option<MaskWardrobeAction>,
) -> Result<(), String> {
    #[cfg(unix)]
    let result = state_dir
        .zip(window_id)
        .ok_or_else(|| "owner wardrobe requires the installed service actor".to_string())
        .and_then(|(dir, window)| {
            crate::durable_host_control::browser::wardrobe(
                dir,
                window,
                binding.clone(),
                request,
                owner_plan_id,
                basis_revision,
                action,
            )
        });
    #[cfg(not(unix))]
    let result = {
        let _ = (
            state_dir,
            window_id,
            binding,
            request,
            owner_plan_id,
            basis_revision,
            action,
        );
        Err::<serde_json::Value, String>("owner-unavailable".into())
    };
    let (accepted, code, report) = match result {
        Ok(report) => (true, String::new(), Some(Box::new(report))),
        Err(error) => {
            let code = if error == "owner-wardrobe-plan-stale" {
                "owner-wardrobe-plan-stale"
            } else if error.contains("StaleRevision") {
                "owner-wardrobe-revision-stale"
            } else if error.contains("carrier-mismatch") {
                "owner-wardrobe-carrier-mismatch"
            } else {
                "owner-wardrobe-unavailable"
            };
            (false, code.into(), None)
        }
    };
    socket.send(&Out::FaceWardrobeResponse {
        protocol: PROTOCOL,
        request_id,
        accepted,
        code,
        report,
    })
}
