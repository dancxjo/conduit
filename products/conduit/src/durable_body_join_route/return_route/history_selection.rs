//! Complete typed Look input on the same finite authenticated return bearer.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

pub(super) const SCHEMA: &str = "conduit.body/native-owner-history-selection@1";

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Selection {
    schema: String,
    token: [u8; 32],
    sequence: u8,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    look: Vec<u8>,
}

pub(super) fn serve(
    line: &mut conduit_std_host::secure_websocket::SecureWebSocketLine,
    listener: &SecureWebSocketListener,
    state_dir: &Path,
    old: &Grant,
    next_sequence: u8,
    document: serde_json::Value,
) -> Result<show_ack::ShowGate, String> {
    let selection: Selection =
        serde_json::from_value(document).map_err(|_| "native history selection frame invalid")?;
    if selection.schema != SCHEMA
        || selection.sequence != 0
        || current_time_millis()? >= old.expires_at_millis
        || !old.matches_request(selection.token, &selection.request)
    {
        return Err("native history selection grant or basis invalid".into());
    }
    if selection.look.len() > conduit_face::MAX_LOOK_BYTES {
        return Err("native-history-look-too-large".into());
    }
    let look = conduit_face::LookInfo::decode(&selection.look)
        .map_err(|_| "native-history-look-invalid")?;
    line.complete_bounded_input();
    crate::durable_host_control::select_native_history_mask(
        state_dir,
        old.receipt.clone(),
        selection.request.clone(),
        selection.show,
        look,
    )?;
    refresh::serve(
        line,
        listener,
        state_dir,
        old,
        next_sequence,
        serde_json::json!({
            "schema": refresh::REFRESH_SCHEMA, "token": selection.token,
            "sequence": 0, "request": selection.request,
        }),
    )
}
