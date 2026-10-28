//! Ordinary linked Source planning and affine actual-owner selection. No Step
//! is polled here; successful construction remains held for Root admission.
use super::*;
use n::{NetworkOperationLease, ReadyNetworkPair};

/// Issuance is irreversible at this waist. A terminal refusal retains every
/// actual bearer beside the original Ready slots until explicit owner recovery.
#[derive(Default)]
#[allow(
    dead_code,
    reason = "terminal refusal retains affine bearers for explicit owner recovery"
)]
pub(super) enum IssuedCustody {
    #[default]
    Empty,
    Lease(NetworkOperationLease),
    First(c::BoundBaseOperation),
    FirstLease(c::BoundBaseOperation, NetworkOperationLease),
    Both(c::BoundBaseOperation, c::BoundBaseOperation),
}
impl IssuedCustody {
    pub(super) fn is_empty(&self) -> bool {
        matches!(self, Self::Empty)
    }
}
impl NativePlay {
    /// Replace rich construction only for the independently pinned local
    /// appliance. Descriptions never authorize an operation: ordinary planning,
    /// opaque verification, actual Root issuance and actual-owner checks do.
    pub fn install_linked_held(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        generation: c::OfferGeneration,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
    ) -> Result<(), ()> {
        if self.attempted || !self.flat_refusal.is_empty() || source.is_none() || join.is_none() {
            return Err(());
        }
        self.attempted = true;
        self.configuration_revision = crate::native_credentials::committed_revision();
        self.phase = "installation-refused";
        // Inline terminal custody is established before either Ready owner moves.
        // No fallible allocation is needed to retain an irreversibly issued bearer.
        self.flat_refusal = IssuedCustody::Empty;
        let custody = &mut self.flat_refusal;
        let pair = ReadyNetworkPair::new(
            source.take().expect("preflighted actual source"),
            join.take().expect("preflighted actual join"),
        );
        let bases = [c::BaseImplementationId::from(
            c::LOCAL_BASE_IMPLEMENTATION_ID,
        )];
        let pins = (
            expected.source_document_id.as_str(),
            expected.checked_plot_id.as_str(),
            expected.expanded_plot_id.as_str(),
        );
        let mut active = None;
        let result = pair.prepare(|owners| {
            let grants = [
                issuer.grant_ref(owners.credential_identity(), Operation::Credentials)?,
                issuer.grant_ref(owners.join_identity(), Operation::Join)?,
            ];
            let attachment = conduit_net::PreparedCanonicalNetworkOffer::attachment_local(
                CAP,
                IMPLEMENTATION,
                ARTIFACT,
            );
            let header = c::HostAdvertisementRef {
                protocol_version: c::PROTOCOL_VERSION,
                host_id: owners.join_identity().host.as_str(),
                boot_id: owners.join_identity().boot.as_str(),
                offer_generation: generation,
                profile: "environment/esp32-native-root@1",
                bases: c::BaseProviderAdvertisementRefs::Borrowed(&[]),
                resources: c::ResourceOfferRefs::Borrowed(&[]),
                capabilities: c::CapabilityOfferRefs::Borrowed(&[]),
                planner_capabilities: &[],
            };
            // Keep the affine checked owners in the Ready-pair scope, while both
            // temporary-description callbacks return only unit. No Plan borrow
            // enters this slot, and neither callback can publish the owners.
            let mut selected = None;
            let mut checked = Err(());
            let mut planned = Err(());
            owners
                .with_advertisement(header, &attachment, |owners, host| {
                    planned = conduit_planner::borrowed_local::with_linked_local_plan(
                        image,
                        host,
                        &bases,
                        c::AuthorityGrantRefs::Borrowed(&grants),
                        1,
                        conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                        |scope| {
                            checked = check_linked_scope(
                                owners,
                                scope,
                                issuer,
                                pins,
                                &mut active,
                                custody,
                                &mut selected,
                            );
                        },
                    )
                    .map_err(|_| ());
                })
                .map_err(|_| ())?;
            planned?;
            checked?;
            let (checked, parts) = selected.ok_or(())?;
            // Temporary planner descriptions have gone. Allocate final backing
            // now to preserve the measured non-overlapping construction shape.
            let backing = match reserve_kernel_backing() {
                Ok(backing) => backing,
                Err(()) => {
                    let (first, second) = checked.into_operations();
                    *custody = IssuedCustody::Both(first, second);
                    return Err(());
                }
            };
            Ok((checked, (parts, backing)))
        });
        // No Plan, owner projection, grants, or borrowed lowering survives this
        // point. Final backing was allocated only after temporary descriptions
        // retired; initialization transfers it without a further allocation.
        drop(bases);
        self.active = active;
        match result {
            Ok((credentials, join, (parts, backing))) => {
                self.source = Some(credentials);
                self.join = Some(join);
                self.flat_refusal = IssuedCustody::Empty;
                self.finish_linked_held(parts, backing)
            }
            Err((pair, ())) => {
                let (credentials, provider) = pair.into_ready();
                *source = Some(credentials);
                *join = Some(provider);
                Err(())
            }
        }
    }
    /// Materialization follows completed checked-owner selection. Its scheduler
    /// construction temporaries must not share a frame with nested planning,
    /// capability verification or SHA preparation.
    #[inline(never)]
    fn finish_linked_held(
        &mut self,
        parts: n::PreparedNetworkKernelParts,
        backing: alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>,
    ) -> Result<(), ()> {
        self.prepared = Some(alloc::boxed::Box::write(
            backing,
            parts.finish().map_err(|_| ())?,
        ));
        self.phase = "installed-held";
        Ok(())
    }
}

// Keep the concrete affine issuance/checking waist out of the generic planner
// callback. Its owner brand is preserved; no Source/Plan borrow escapes.
#[inline(never)]
fn check_linked_scope<'owner>(
    owners: n::ReadyNetworkPairView<'owner, Provider, Source>,
    scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
    issuer: &mut RootIssuer,
    pins: (&str, &str, &str),
    active: &mut Option<c::ActivePlayIdentity>,
    custody: &mut IssuedCustody,
    selected: &mut Option<(
        n::PreparedNetworkPairBindings<'owner>,
        n::PreparedNetworkKernelParts,
    )>,
) -> Result<(), ()> {
    let tables = n::NetworkControlFlatDirectTables::prepare(scope.verified()).map_err(|_| ())?;
    let roles = n::PreparedNetworkKernelParts::check_flat_direct(&tables).map_err(|_| ())?;
    let view = scope.verified().view();
    // The pair checks Credentials and Join only. Keep this
    // independent actual attachment implementation preflight.
    let sign = view
        .placements
        .iter()
        .nth(usize::from(roles.attachment_sign.0))
        .ok_or(())?;
    if sign.capability_id != CAP
        || sign.implementation_id != IMPLEMENTATION
        || sign.artifact_id != ARTIFACT
        || sign.execution_profile_id != conduit_net::NETWORK_ATTACHMENT_LOCAL_PROFILE
        || sign.host_calls.len() != 1
        || sign.host_calls.iter().next().ok_or(())?.contract_id
            != conduit_net::NETWORK_ATTACHMENT_SIGN_HOST_CALL
    {
        return Err(());
    }
    let credential = view
        .placements
        .iter()
        .nth(usize::from(roles.credentials.0))
        .ok_or(())?
        .placement_id;
    let join = view
        .placements
        .iter()
        .nth(usize::from(roles.join.0))
        .ok_or(())?
        .placement_id;
    *active = Some(issuer.reserve_linked_network_play(scope, pins)?);
    let current = active.as_ref().expect("reserved exact parent");
    let (table, handle, claim) = issuer.issue_linked_network(
        owners.credential_identity(),
        Operation::Credentials,
        scope,
        pins,
        current,
    )?;
    let first = match owners.check_flat_credentials(
        NetworkOperationLease {
            table,
            handle,
            claim,
        },
        &tables,
        current,
        credential,
    ) {
        Ok(first) => first,
        Err((lease, _)) => {
            *custody = IssuedCustody::Lease(lease);
            return Err(());
        }
    };
    let issued =
        issuer.issue_linked_network(first.join_identity(), Operation::Join, scope, pins, current);
    let (table, handle, claim) = match issued {
        Ok(issued) => issued,
        Err(()) => {
            *custody = IssuedCustody::First(first.into_operation());
            return Err(());
        }
    };
    let checked = match first.check_flat_join(
        NetworkOperationLease {
            table,
            handle,
            claim,
        },
        &tables,
        current,
        join,
    ) {
        Ok(checked) => checked,
        Err((first, lease, _)) => {
            *custody = IssuedCustody::FirstLease(first.into_operation(), lease);
            return Err(());
        }
    };
    match n::PreparedNetworkKernelParts::from_flat_direct(tables) {
        Ok(parts) => {
            *selected = Some((checked, parts));
            Ok(())
        }
        Err(_) => {
            let (first, second) = checked.into_operations();
            *custody = IssuedCustody::Both(first, second);
            Err(())
        }
    }
}

#[cfg(test)]
std::thread_local! {
    static REFUSE_NEXT_BACKING: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(super) fn refuse_next_backing() {
    REFUSE_NEXT_BACKING.with(|flag| flag.set(true));
}
fn reserve_kernel_backing(
) -> Result<alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>, ()> {
    reserve_kernel_backing_using(|layout| {
        #[cfg(test)]
        if REFUSE_NEXT_BACKING.with(|flag| flag.replace(false)) {
            return core::ptr::null_mut();
        }
        // SAFETY: allocator receives the exact requested nonzero layout.
        unsafe { alloc::alloc::alloc(layout) }
    })
}
fn reserve_kernel_backing_using(
    allocate: impl FnOnce(core::alloc::Layout) -> *mut u8,
) -> Result<alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>, ()> {
    let layout = core::alloc::Layout::new::<PreparedNetworkKernel>();
    if layout.size() == 0 {
        return Ok(alloc::boxed::Box::<PreparedNetworkKernel>::new_uninit());
    }
    // SAFETY: exact layout, unique ownership, initialized only by Box::write.
    let raw = allocate(layout).cast::<core::mem::MaybeUninit<PreparedNetworkKernel>>();
    if raw.is_null() {
        return Err(());
    }
    Ok(unsafe { alloc::boxed::Box::from_raw(raw) })
}

#[cfg(test)]
mod backing_tests {
    use super::*;
    #[test]
    fn actual_kernel_layout_refusal_returns_without_initialized_owner() {
        let mut requested = None;
        let result = reserve_kernel_backing_using(|layout| {
            requested = Some(layout);
            core::ptr::null_mut()
        });
        assert!(result.is_err());
        assert_eq!(
            requested,
            Some(core::alloc::Layout::new::<PreparedNetworkKernel>())
        );
    }
}
