//! One explicitly prepared display-local renderer owner. Rc shares ownership;
//! it grants neither Host/Boot capability nor scanout/semantic Show authority.
//! Wire this module only for the actual x86_64 freestanding service backend.
use super::{
    CompositorService, TransportError,
    raster_staging::{RasterLease, RasterStaging, STAGING_PIXELS},
    session_owner::{OwnerError, SessionOwner},
};
use crate::{
    display::DisplayError,
    protected_region::{DomainBackend, DomainCost},
};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::RefCell, mem::size_of};

pub(crate) const STAGING_BYTES: usize = STAGING_PIXELS * size_of::<u32>();
pub(crate) const SERVICE_RESERVED_BYTES: u32 = crate::arch::ServiceDomain::RESERVED_BYTES;

pub(crate) type SharedDisplayOwner = Rc<RefCell<DisplayOwner>>;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum PreparationError {
    Allocation,
    Transport(TransportError),
    Owner(OwnerError<TransportError>),
}

/// A preparation snapshot, not a live execution-cost counter or a complete boot
/// memory budget. Rc/RefCell metadata and caller-owned presenter state are extra.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PreparationReport {
    pub staging_logical_bytes: usize,
    pub staging_reserved_bytes: usize,
    pub service_reserved_bytes: u32,
    /// Includes the initialization entry performed by install_embedded.
    pub service_cost_at_preparation: DomainCost,
}

pub(crate) struct DisplayOwner {
    // The session manager exposes only fenced session operations. Its underlying
    // transport is private: no consumer can take a raw mutable service here.
    pub(crate) sessions: SessionOwner<CompositorService>,
    staging: Vec<u32>,
    report: PreparationReport,
}

impl DisplayOwner {
    /// Prepare once before play, then clone the returned Rc for presenters on
    /// this display. There is no automatic restart, replacement, or retry.
    /// The large arena reservation is fallible. Stable Rc::new follows the
    /// allocator's ordinary failure policy for its small metadata allocation.
    pub(crate) fn prepare(width: u16, height: u16) -> Result<SharedDisplayOwner, PreparationError> {
        if width == 0 || height == 0 {
            return Err(PreparationError::Transport(TransportError::Bounds));
        }
        let mut staging = Vec::new();
        staging
            .try_reserve_exact(STAGING_PIXELS)
            .map_err(|_| PreparationError::Allocation)?;
        // Reservation precedes initialization. resize cannot allocate because
        // capacity already covers this exact logical length; never shrink it.
        staging.resize(STAGING_PIXELS, 0);
        // Admission is short-lived, not a self-reference retained in this owner.
        // A failure here is a local storage-contract mismatch, not guest retry.
        let admitted = RasterStaging::admit(staging.as_mut_slice())
            .map_err(|_| PreparationError::Owner(OwnerError::Capacity))?;
        drop(admitted);
        let service = CompositorService::install_embedded(width, height)
            .map_err(PreparationError::Transport)?;
        let service_cost_at_preparation = service.domain.cost();
        let sessions = SessionOwner::new(service).map_err(PreparationError::Owner)?;
        let report = PreparationReport {
            staging_logical_bytes: STAGING_BYTES,
            staging_reserved_bytes: staging.capacity() * size_of::<u32>(),
            service_reserved_bytes: SERVICE_RESERVED_BYTES,
            service_cost_at_preparation,
        };
        Ok(Rc::new(RefCell::new(Self {
            sessions,
            staging,
            report,
        })))
    }

    pub(crate) fn preparation_report(&self) -> PreparationReport {
        self.report
    }

    /// Split private staging from the fenced session manager. A caller renders,
    /// uploads bounded chunks through its exact session, and returns; the lease
    /// clears on both successful and error-valued callback returns. No borrow of
    /// the raster or service may escape, and no second raster is allocated.
    /// A shared owner must be exclusively borrowed for the complete callback.
    pub(crate) fn with_raster<R>(
        &mut self,
        width: u16,
        height: u16,
        render_and_upload: impl FnOnce(&mut SessionOwner<CompositorService>, &mut RasterLease<'_>) -> R,
    ) -> Result<R, DisplayError> {
        let mut staging = RasterStaging::admit(self.staging.as_mut_slice())?;
        let mut lease = staging.lease(width, height)?;
        Ok(render_and_upload(&mut self.sessions, &mut lease))
    }
}
