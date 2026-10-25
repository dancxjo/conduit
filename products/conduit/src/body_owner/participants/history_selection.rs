//! Carrier worker forwards history intent; the installed actor owns selection.
use super::*;
#[allow(clippy::too_many_arguments)]
pub(super) fn respond(
    socket: &mut transport::Socket,
    state_dir: Option<&Path>,
    window_id: Option<&str>,
    binding: &LinkBindingId,
    request_id: String,
    request: conduit_face::OwnerFaceSnapshotRequest,
    show: Box<conduit_face::MaskShow>,
    look: Vec<u8>,
) -> Result<(), String> {
    #[cfg(unix)]
    let result = state_dir
        .zip(window_id)
        .ok_or_else(|| "history selection requires the installed service actor".to_string())
        .and_then(|(dir, window)| {
            crate::durable_host_control::browser::history_mask(
                dir,
                window,
                binding.clone(),
                request,
                show,
                look,
            )
        });
    #[cfg(not(unix))]
    let result = {
        let _ = (state_dir, window_id, binding, request, show, look);
        Err::<(), String>("owner-unavailable".into())
    };
    let refusal = result.as_ref().err().map_or_else(String::new, |error| {
        if error.contains("stale") || error.contains("Stale") {
            "stale-face-or-show".into()
        } else {
            "history-mask-selection-unavailable".into()
        }
    });
    socket.send(&Out::FaceHistoryMaskResponse {
        protocol: PROTOCOL,
        request_id,
        accepted: result.is_ok(),
        refusal,
    })
}
