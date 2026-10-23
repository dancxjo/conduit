//! One explicitly prepared display-local renderer owner. Rc shares ownership;
//! it grants neither Host/Boot capability nor scanout/semantic Show authority.
//! Wire this module only for the actual x86_64 freestanding service backend.
use super::{
    CompositorService, TransportError,
    display_grant::{DisplayGrant, SharedDisplayGrant},
    raster_staging::{RasterLease, RasterStaging, STAGING_PIXELS},
    session_owner::{OwnerError, SessionOwner},
};
use crate::{display::DisplayError, protected_region::DomainCost};
use alloc::{rc::Rc, vec::Vec};
use core::{cell::RefCell, mem::size_of};
use sha2::{Digest, Sha256};
#[cfg(feature = "native-compositor")]
#[path = "lifecycle_emission.rs"]
mod lifecycle_emission;

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
/// memory budget. Owner Rc/RefCell metadata, shared grant Rc metadata/identities
/// and caller-owned presenter state are extra.
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
    grant: SharedDisplayGrant,
    guest_image_sha256: [u8; 32],
    #[cfg(feature = "native-compositor")]
    observation_make: Option<crate::make::MakeRecord>,
    #[cfg(feature = "native-compositor")]
    observation_seen: u8,
    staging_preparation_raw_ticks: Option<u64>,
    preparation_raw_ticks: Option<u64>,
    disposal_raw_ticks: Option<u64>,
}

impl DisplayOwner {
    /// Prepare once before play, then clone the returned Rc for presenters on
    /// this display. There is no automatic restart, replacement, or retry.
    /// The large arena reservation is fallible. Stable Rc::new follows the
    /// allocator's ordinary failure policy for its small metadata allocation.
    pub(crate) fn prepare(grant: DisplayGrant) -> Result<SharedDisplayOwner, PreparationError> {
        Self::prepare_shared(Rc::new(grant))
    }

    /// Explicit fresh preparation under Root's still-current display grant.
    /// Root must retain and validate the same Host, Boot, offer generation, Base
    /// and concrete target format; sharing does not renew a replaced provider.
    /// Keep this immutable handle before releasing an old staging arena: cloning
    /// it allocates nothing before the new contiguous 8 MiB reservation. Other
    /// fragmentation can still refuse preparation; there is no retry or larger
    /// memory profile here. Every successful preparation installs a fresh guest
    /// instance and starts without presenter namespaces or semantic Show/input.
    pub(crate) fn prepare_shared(
        grant: SharedDisplayGrant,
    ) -> Result<SharedDisplayOwner, PreparationError> {
        let preparation_started = super::lifecycle::start();
        let (width, height) = grant.extent();
        let staging_started = super::lifecycle::start();
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
        let staging_finished = super::lifecycle::start();
        let service = CompositorService::install_embedded(width, height)
            .map_err(PreparationError::Transport)?;
        let service_cost_at_preparation = service.cost();
        let sessions = SessionOwner::new(service).map_err(PreparationError::Owner)?;
        // Hash exactly the immutable bytes used by install_embedded, once per
        // preparation. Its work remains inside the preparation tick envelope.
        let guest_image_sha256: [u8; 32] =
            Sha256::digest(CompositorService::embedded_image_bytes()).into();
        let report = PreparationReport {
            staging_logical_bytes: STAGING_BYTES,
            staging_reserved_bytes: staging.capacity() * size_of::<u32>(),
            service_reserved_bytes: SERVICE_RESERVED_BYTES,
            service_cost_at_preparation,
        };
        let owner = Rc::new(RefCell::new(Self {
            sessions,
            staging,
            report,
            grant,
            guest_image_sha256,
            #[cfg(feature = "native-compositor")]
            observation_make: None,
            #[cfg(feature = "native-compositor")]
            observation_seen: 0,
            staging_preparation_raw_ticks: super::lifecycle::elapsed(
                staging_started,
                staging_finished,
            ),
            preparation_raw_ticks: None,
            disposal_raw_ticks: None,
        }));
        let finished = super::lifecycle::start();
        owner.borrow_mut().preparation_raw_ticks =
            super::lifecycle::elapsed(preparation_started, finished);
        Ok(owner)
    }

    /// Immutable digest custody; observing it never reads or rehashes guest memory.
    pub(crate) fn guest_image_sha256(&self) -> &[u8; 32] {
        &self.guest_image_sha256
    }

    /// Immutable embedded image for preparation-time source binding only.
    pub(crate) fn embedded_image_bytes(&self) -> &'static [u8] {
        CompositorService::embedded_image_bytes()
    }

    /// Borrowed exact scope only; observing it cannot renew or replace a grant.
    pub(crate) fn display_identity(&self) -> super::display_grant::DisplayIdentity<'_> {
        self.grant.identity()
    }

    pub(crate) fn matches_scope(
        &self,
        host: &conduit_core::HostId,
        boot: &conduit_core::BootId,
        generation: conduit_core::OfferGeneration,
        base: &conduit_core::HostBaseId,
    ) -> bool {
        self.grant.matches(host, boot, generation, base)
    }

    pub(crate) fn format_matches(&self, format: crate::display::DisplayFormat) -> bool {
        self.grant.format_matches(format)
    }

    /// Terminally fence clients and return the cleared transient arena to the
    /// admitted allocator. The exclusive owner borrow excludes every RasterLease.
    /// An inert Rc can outlive this release without retaining pixel storage.
    pub(crate) fn quarantine(&mut self) {
        let started = super::lifecycle::start();
        let live_staging = !self.staging.is_empty();
        self.sessions.quarantine();
        self.staging.fill(0);
        drop(core::mem::take(&mut self.staging));
        if live_staging {
            self.disposal_raw_ticks = super::lifecycle::elapsed(started, super::lifecycle::start());
        }
    }

    /// Fixed read-only lifecycle snapshot; no pixel, grant or transport escape.
    pub(crate) fn lifecycle_observation(&self) -> super::lifecycle::DisplayObservation {
        super::lifecycle::DisplayObservation {
            service: self.sessions.service_observation(),
            guest_image_sha256: self.guest_image_sha256,
            activations: self.sessions.activation_observation(),
            current_cost: self.service_cost(),
            staging_prepared_bytes: self.report.staging_logical_bytes,
            staging_live_bytes: self.staging.len() * size_of::<u32>(),
            staging_preparation_raw_ticks: self.staging_preparation_raw_ticks,
            preparation_raw_ticks: self.preparation_raw_ticks,
            disposal_raw_ticks: self.disposal_raw_ticks,
        }
    }

    pub(crate) fn service_cost(&self) -> DomainCost {
        self.sessions.service_cost()
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
        let result = {
            let mut staging = RasterStaging::admit(self.staging.as_mut_slice())?;
            let mut lease = staging.lease(width, height)?;
            render_and_upload(&mut self.sessions, &mut lease)
        };
        // A transport failure may have already cleared the active namespace.
        // End the lease before releasing its arena; cleanup cannot be deferred
        // until the next compositor entry or until stale Rc clients disappear.
        if self.sessions.is_terminated() {
            self.quarantine();
        }
        Ok(result)
    }
}
