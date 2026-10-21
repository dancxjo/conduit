//! Root semantic custody and transient raster source for a protected renderer.
//! No retained pixels, damage ordering, composition or route selection live here.
mod lifecycle;
mod raster;
mod routing;
mod scanout;
use super::{
    CompositionReceipt, CompositorAdmission, NativeCompositorError, ScanoutAcknowledgement,
};
use crate::protected_compositor::{
    TransportError,
    display_owner::SharedDisplayOwner,
    session_owner::{OwnerError, SessionId},
};
use alloc::{rc::Rc, string::String, vec::Vec};
use conduit_core::{PlacementId, PlanId};
use conduit_face::{GraphicsScene, LayoutRect, ManifestationId};
use conduitos_compositor_core::wire::{Request, Response};
use conduitos_compositor_core::{Error, Handle};
use core::cell::Cell;

#[derive(Clone)]
struct Binding {
    manifestation: ManifestationId,
    plan: PlanId,
    placement: PlacementId,
    front: String,
    face_revision: u64,
}
struct Surface {
    id: String,
    handle: Handle,
    // Accepted resource declarations, used for admission/raster extents and
    // placement receipts only. Never used to select surfaces or compute damage.
    bounds: LayoutRect,
    z: u8,
    binding: Option<Binding>,
    receipt: Option<CompositionReceipt>,
    expression: Option<conduit_face::AdmittedVisualExpression>,
    upload_sequence: u64,
}
struct Scanout {
    sequence: u64,
    coverage: [u32; 8],
    session: SessionId,
}
pub struct NativeCompositor {
    admission: CompositorAdmission,
    owner: Option<SharedDisplayOwner>,
    session: Option<SessionId>,
    surfaces: Vec<Surface>,
    scanout: Option<Scanout>,
    frame_sequence: u64,
    admitted_pixels: usize,
    failed: bool,
    // Last submitted input samples only; never renderer/readiness authority.
    pointer_sample: Cell<Option<(u32, u32)>>,
    hover_sample: Cell<bool>,
}
impl NativeCompositor {
    pub fn admitted(admission: CompositorAdmission) -> Self {
        Self {
            admission,
            owner: None,
            session: None,
            surfaces: Vec::with_capacity(8),
            scanout: None,
            frame_sequence: 0,
            admitted_pixels: 0,
            failed: false,
            pointer_sample: Cell::new(None),
            hover_sample: Cell::new(false),
        }
    }
    /// Attachment never installs or activates another service. First admission
    /// acquires a namespace; a stale namespace cannot automatically reactivate.
    pub(crate) fn attach_display_owner(
        &mut self,
        owner: SharedDisplayOwner,
    ) -> Result<(), NativeCompositorError> {
        if !owner
            .try_borrow()
            .map_err(|_| NativeCompositorError::ServiceBusy)?
            .matches_scope(
                &self.admission.host_id,
                &self.admission.boot_id,
                self.admission.offer_generation,
                &self.admission.display_base_id,
            )
        {
            return Err(NativeCompositorError::StaleIdentity);
        }
        if let Some(current) = &self.owner {
            if !Rc::ptr_eq(current, &owner) {
                return Err(NativeCompositorError::ServiceBusy);
            }
        } else {
            self.owner = Some(owner);
        }
        Ok(())
    }
    pub fn admission(&self) -> &CompositorAdmission {
        &self.admission
    }
    pub const fn frame_sequence(&self) -> u64 {
        self.frame_sequence
    }
    /// Resident retained pixels owned by this Root facade. The protected guest
    /// owns its fixed pixel arenas; transient raster staging is leased separately.
    pub const fn allocated_pixels(&self) -> usize {
        0
    }
    pub fn receipts(&self) -> impl Iterator<Item = &CompositionReceipt> {
        self.surfaces
            .iter()
            .filter_map(|surface| surface.receipt.as_ref())
    }
    fn index(&self, id: &str) -> Result<usize, NativeCompositorError> {
        self.validate_id(id)?;
        self.surfaces
            .iter()
            .position(|s| s.id == id)
            .ok_or(NativeCompositorError::SurfaceNotAdmitted)
    }
    fn validate_id(&self, id: &str) -> Result<(), NativeCompositorError> {
        if self
            .admission
            .surface_ids
            .iter()
            .any(|candidate| candidate == id)
        {
            Ok(())
        } else {
            Err(NativeCompositorError::UnadmittedSurface)
        }
    }
    fn owner(&self) -> Result<&SharedDisplayOwner, NativeCompositorError> {
        self.owner
            .as_ref()
            .ok_or(NativeCompositorError::ServiceUnavailable)
    }
    pub(crate) fn validate_active_session(&self) -> Result<(), NativeCompositorError> {
        self.scope().map(|_| ())
    }
    fn scope(&self) -> Result<SessionId, NativeCompositorError> {
        if self.failed {
            return Err(NativeCompositorError::ServiceLost);
        }
        let session = self.session.ok_or(NativeCompositorError::StaleIdentity)?;
        let owner = self
            .owner()?
            .try_borrow()
            .map_err(|_| NativeCompositorError::ServiceBusy)?;
        if !owner.sessions.is_active(session) {
            return Err(NativeCompositorError::StaleIdentity);
        }
        Ok(session)
    }
    fn request(&self, request: Request) -> Result<Response, NativeCompositorError> {
        let session = self.scope()?;
        self.owner()?
            .try_borrow_mut()
            .map_err(|_| NativeCompositorError::ServiceBusy)?
            .sessions
            .request(session, request)
            .map_err(owner_error)
            .and_then(response)
    }
    fn unit(&self, request: Request) -> Result<(), NativeCompositorError> {
        match self.request(request)? {
            Response::Unit => Ok(()),
            _ => Err(NativeCompositorError::ServiceProtocol),
        }
    }
    fn invalidate(&mut self) {
        self.scanout = None;
    }
    fn fail(&mut self) {
        self.scanout = None;
        self.failed = true;
        if let (Some(owner), Some(session)) = (&self.owner, self.session) {
            if let Ok(mut owner) = owner.try_borrow_mut() {
                if owner.sessions.is_active(session) {
                    owner.quarantine();
                }
            }
        }
    }
}
fn response(response: Response) -> Result<Response, NativeCompositorError> {
    if let Response::Refused(error) = response {
        return Err(core_error(error));
    }
    Ok(response)
}
fn core_error(error: Error) -> NativeCompositorError {
    use NativeCompositorError as E;
    match error {
        Error::Bounds => E::InvalidBounds,
        Error::Capacity => E::SurfaceCapacityExceeded,
        Error::Stale => E::StaleSurfaceBinding,
        Error::Busy => E::ServiceBusy,
        Error::Incomplete => E::SurfaceNotAdmitted,
        Error::Order => E::ServiceProtocol,
        Error::NoDamage => E::SurfaceNotAdmitted,
        Error::Exhausted => E::ServiceLost,
    }
}
fn owner_error(error: OwnerError<TransportError>) -> NativeCompositorError {
    use NativeCompositorError as E;
    match error {
        OwnerError::InactiveSession => E::StaleIdentity,
        OwnerError::ForeignHandle => E::StaleSurfaceBinding,
        OwnerError::Capacity => E::SurfaceCapacityExceeded,
        OwnerError::Protocol => E::ServiceProtocol,
        OwnerError::Terminated | OwnerError::Exhausted | OwnerError::Transport(_) => E::ServiceLost,
    }
}
fn rect(bounds: LayoutRect) -> conduitos_compositor_core::Rect {
    conduitos_compositor_core::Rect {
        x: bounds.x,
        y: bounds.y,
        width: bounds.width,
        height: bounds.height,
    }
}
fn pixels(bounds: LayoutRect) -> Result<usize, NativeCompositorError> {
    if bounds.width == 0 || bounds.height == 0 {
        return Err(NativeCompositorError::InvalidBounds);
    }
    usize::from(bounds.width)
        .checked_mul(usize::from(bounds.height))
        .filter(|&n| n <= super::MAX_COMPOSITOR_PIXELS)
        .ok_or(NativeCompositorError::SurfaceCapacityExceeded)
}
