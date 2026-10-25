//! Checked history Look selection through the existing authenticated owner service.
use super::*;
use conduit_plot::rust_binding::NativeRustBinding;

impl DurableHostRuntime {
    pub(super) fn owned_body_native_history_mask(
        &mut self,
        request: &OwnerFaceSnapshotRequest,
        show: &MaskShow,
        receipt: &PortableAdmissionReceipt,
        look: &[u8],
    ) -> Result<RemoteOwnerMaskRouteSeal, String> {
        let HostSource::Body { owner, .. } = &mut self.host else {
            return Err("installed Host does not own a live Body session".into());
        };
        if look.len() > conduit_face::MAX_LOOK_BYTES {
            return Err("native-history-look-too-large".into());
        }
        let look =
            conduit_face::LookInfo::decode(look).map_err(|_| "native-history-look-invalid")?;
        owner.select_native_history_mask(request, show, receipt, look)
    }
}

#[cfg(unix)]
pub(crate) fn select_native_history_mask(
    state_dir: &Path,
    receipt: PortableAdmissionReceipt,
    request: OwnerFaceSnapshotRequest,
    show: MaskShow,
    look: conduit_face::LookInfo,
) -> Result<RemoteOwnerMaskRouteSeal, String> {
    let look = look.encode().map_err(|_| "native-history-look-invalid")?;
    if look.len() > conduit_face::MAX_LOOK_BYTES {
        return Err("native-history-look-too-large".into());
    }
    match body::call(
        state_dir,
        Request::BodyNativeHistoryMask {
            protocol: PROTOCOL,
            token: body::token(state_dir)?,
            receipt: Box::new(receipt),
            request,
            show: Box::new(show),
            look,
        },
    )? {
        Response::BodyNativeMaskRoute {
            protocol: PROTOCOL,
            route,
        } => Ok(*route),
        Response::Refused { code, .. } => Err(code),
        _ => Err("Body owner returned the wrong native history route response".into()),
    }
}

#[cfg(not(unix))]
pub(crate) fn select_native_history_mask(
    _state_dir: &Path,
    _receipt: PortableAdmissionReceipt,
    _request: OwnerFaceSnapshotRequest,
    _show: MaskShow,
    _look: conduit_face::LookInfo,
) -> Result<RemoteOwnerMaskRouteSeal, String> {
    Err("no reviewed local durable host control carrier exists on this platform".into())
}
