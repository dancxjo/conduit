//! Same browser actor, exact current carrier and acknowledged Show.
use super::*;
pub(crate) fn history_mask(
    state_dir: &Path,
    window_id: &str,
    binding: LinkBindingId,
    request: OwnerFaceSnapshotRequest,
    show: Box<MaskShow>,
    look: Vec<u8>,
) -> Result<(), String> {
    if look.is_empty() || look.len() > conduit_face::MAX_LOOK_BYTES {
        return Err("browser-history-look-size".into());
    }
    match call(state_dir, |token| Request::BodyBrowserHistoryMask {
        protocol: PROTOCOL,
        token,
        window_id: window_id.into(),
        binding,
        request,
        show,
        look,
    })? {
        Response::BodyBrowserComponentMaskAccepted { protocol: PROTOCOL } => Ok(()),
        Response::Refused { code, .. } => Err(code),
        _ => Err("Body owner returned the wrong history selection response".into()),
    }
}
