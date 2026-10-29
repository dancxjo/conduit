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
        self.install_linked_mode(image, expected, generation, source, join, issuer, None)
    }
    /// Separate immutable checked preparation from admitted execution backing.
    /// Before `admit` succeeds no attempt or Root sequence/bearer is changed.
    pub(crate) fn install_linked_phased_held(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        generation: c::OfferGeneration,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
        admit: impl FnOnce() -> Result<(), ()>,
    ) -> Result<(), ()> {
        if self.attempted || !self.flat_refusal.is_empty() || source.is_none() || join.is_none() {
            trace_phased_refusal(
                100,
                &self.flat_refusal,
                self.active.is_some(),
                self.attempted,
            );
            return Err(());
        }
        let pair = ReadyNetworkPair::new(source.take().ok_or(())?, join.take().ok_or(())?);
        let pins = (
            expected.source_document_id.as_str(),
            expected.checked_plot_id.as_str(),
            expected.expanded_plot_id.as_str(),
        );
        let custody = &mut self.flat_refusal;
        let mut reserved = false;
        let mut prepared_active = None;
        let mut refusal_stage = 101;
        let result = pair.prepare(|owners| {
            let (selected, parts, mut root) = {
                let credential_grant =
                    issuer.grant_ref(owners.credential_identity(), Operation::Credentials)?;
                refusal_stage = 102;
                let join_grant = issuer.grant_ref(owners.join_identity(), Operation::Join)?;
                let grants = [credential_grant, join_grant];
                let bases = [c::BaseImplementationId::from(
                    c::LOCAL_BASE_IMPLEMENTATION_ID,
                )];
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
                refusal_stage = 103;
                owners
                    .with_advertisement(header, &attachment, |owners, host| {
                        refusal_stage = 104;
                        conduit_planner::borrowed_local::with_linked_local_plan(
                            image,
                            host,
                            &bases,
                            c::AuthorityGrantRefs::Borrowed(&grants),
                            1,
                            conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                            |scope| {
                                refusal_stage = 105;
                                prepare_linked_selection(owners, scope, issuer, pins)
                            },
                        )
                        .map_err(|_| ())
                    })
                    .map_err(|_| ())???
            };
            // Both native contracts, borrowed Source/Plan projections, grants
            // and lowering tables have retired. Only exact compact proofs and
            // numeric parts overlap final backing and actual issuance.
            refusal_stage = 110;
            let memory = selected.prepare_claim_storage()?;
            let mut operation = Operation::Credentials;
            selected.prepare_issuance_storage(|native, prepared| {
                refusal_stage = if matches!(operation, Operation::Credentials) {
                    111
                } else {
                    112
                };
                root.prepare_issuance_storage(native, operation, prepared)?;
                operation = Operation::Join;
                Ok(())
            })?;
            refusal_stage = 113;
            prepared_active = Some(root.prepare_reservation()?);
            #[cfg(test)]
            record_phased(0, None);
            refusal_stage = 114;
            let storage = Self::reserve_linked_backing()?;
            #[cfg(test)]
            record_phased(1, Some(storage.as_ptr() as usize));
            refusal_stage = 115;
            admit()?;
            self.attempted = true;
            self.configuration_revision = crate::native_credentials::committed_revision();
            self.phase = "installation-refused";
            refusal_stage = 120;
            root.reserve()?;
            reserved = true;
            #[cfg(test)]
            record_phased(4, None);
            let first =
                match selected.issue_credentials_reusing(memory, |native, prepared, memory| {
                    refusal_stage = 121;
                    let issued = root.issue_prepared(
                        native,
                        Operation::Credentials,
                        prepared,
                        prepared_active.as_ref().ok_or(())?,
                        memory,
                    );
                    #[cfg(test)]
                    record_phased(6, None);
                    if issued.is_ok() {
                        refusal_stage = 122;
                    }
                    issued
                }) {
                    Ok(first) => first,
                    Err((lease, _)) => {
                        if let Some(lease) = lease {
                            *custody = IssuedCustody::Lease(lease);
                        }
                        return Err(());
                    }
                };
            #[cfg(test)]
            record_phased(5, None);
            let checked = match first.issue_join_reusing(|native, prepared, memory| {
                refusal_stage = 123;
                let issued = root.issue_prepared(
                    native,
                    Operation::Join,
                    prepared,
                    prepared_active.as_ref().ok_or(())?,
                    memory,
                );
                #[cfg(test)]
                record_phased(7, None);
                if issued.is_ok() {
                    refusal_stage = 124;
                }
                issued
            }) {
                Ok(checked) => checked,
                Err((first, lease, _)) => {
                    *custody = match lease {
                        Some(lease) => IssuedCustody::FirstLease(first, lease),
                        None => IssuedCustody::First(first),
                    };
                    return Err(());
                }
            };
            #[cfg(test)]
            record_phased(2, None);
            refusal_stage = 125;
            let prepared = match finish_reserved_kernel(parts, storage) {
                Ok(prepared) => prepared,
                Err(()) => {
                    let (first, second) = checked.into_operations();
                    *custody = IssuedCustody::Both(first, second);
                    return Err(());
                }
            };
            #[cfg(test)]
            record_phased(3, None);
            Ok((checked, prepared))
        });
        // The exclusive preparation has ended. All temporary descriptions,
        // duplicated claims and refused execution storage have retired before
        // copying the actual reservation into retained success/refusal custody.
        if reserved {
            self.active = prepared_active.take();
        }
        #[cfg(test)]
        record_phased(8, None);
        match result {
            Ok((credentials, join, prepared)) => {
                self.source = Some(credentials);
                self.join = Some(join);
                self.prepared = Some(prepared);
                self.phase = "installed-held";
                Ok(())
            }
            Err((pair, ())) => {
                trace_phased_refusal(refusal_stage, &self.flat_refusal, reserved, self.attempted);
                let (credentials, provider) = pair.into_ready();
                *source = Some(credentials);
                *join = Some(provider);
                Err(())
            }
        }
    }
    pub(crate) fn reserve_linked_backing() -> Result<n::PreparedNetworkKernelStorage, ()> {
        #[cfg(test)]
        let _refusal = if REFUSE_NEXT_BACKING.with(|flag| flag.replace(false)) {
            Some(crate::allocation_probe::refuse_next_allocation_size(
                core::mem::size_of::<PreparedNetworkKernel>(),
            ))
        } else {
            None
        };
        #[cfg(test)]
        let _log_refusal = if REFUSE_NEXT_LOG.with(|flag| flag.replace(false)) {
            Some(crate::allocation_probe::refuse_next_allocation_size(
                n::CONTROL_SIGN_BACKING_BYTES,
            ))
        } else {
            None
        };
        n::PreparedNetworkKernelStorage::try_new().map_err(|_| ())
    }
    pub(crate) fn install_linked_reserved_held(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        generation: c::OfferGeneration,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
        backing: n::PreparedNetworkKernelStorage,
    ) -> Result<(), ()> {
        self.install_linked_mode(
            image,
            expected,
            generation,
            source,
            join,
            issuer,
            Some(backing),
        )
    }
    fn install_linked_mode(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        generation: c::OfferGeneration,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
        backing: Option<n::PreparedNetworkKernelStorage>,
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
        let mut refusal_stage = 1u16;
        let result = pair.prepare(|owners| {
            refusal_stage = 1;
            let credential_grant =
                issuer.grant_ref(owners.credential_identity(), Operation::Credentials)?;
            refusal_stage = 2;
            let join_grant = issuer.grant_ref(owners.join_identity(), Operation::Join)?;
            let grants = [credential_grant, join_grant];
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
            refusal_stage = 3;
            owners
                .with_advertisement(header, &attachment, |owners, host| {
                    refusal_stage = 4;
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
                                &mut refusal_stage,
                            );
                        },
                    )
                    .map_err(|_| ());
                })
                .map_err(|_| ())?;
            planned?;
            checked?;
            refusal_stage = 5;
            let (checked, parts) = selected.ok_or(())?;
            // All persistent storage was reserved at Root's boundary, or the
            // compatible late path reserves fallibly after temporary planning.
            let materialized = match backing {
                Some(storage) => {
                    refusal_stage = 7;
                    finish_reserved_kernel(parts, storage)
                }
                None => {
                    refusal_stage = 6;
                    match reserve_kernel_backing() {
                        Ok(backing) => {
                            refusal_stage = 7;
                            finish_linked_kernel(parts, backing)
                        }
                        Err(()) => Err(()),
                    }
                }
            };
            let prepared = match materialized {
                Ok(prepared) => prepared,
                Err(()) => {
                    let (first, second) = checked.into_operations();
                    *custody = IssuedCustody::Both(first, second);
                    return Err(());
                }
            };
            Ok((checked, prepared))
        });
        // No Plan, owner projection, grants, or borrowed lowering survives this
        // point. Final storage is initialized and all fallible allocation has
        // completed before the actual Ready providers become selected calls.
        drop(bases);
        self.active = active;
        match result {
            Ok((credentials, join, prepared)) => {
                self.source = Some(credentials);
                self.join = Some(join);
                self.flat_refusal = IssuedCustody::Empty;
                self.prepared = Some(prepared);
                self.phase = "installed-held";
                Ok(())
            }
            Err((pair, ())) => {
                trace_flat_refusal(refusal_stage, &self.flat_refusal, self.active.is_some());
                let (credentials, provider) = pair.into_ready();
                *source = Some(credentials);
                *join = Some(provider);
                Err(())
            }
        }
    }
}
/// Materialization occurs after all planner callbacks and checks have returned,
/// while the actual Ready pair and both checked bearers remain affine. A Sign
/// allocation refusal returns them before selected Host Call conversion.
#[inline(never)]
#[cold]
fn finish_linked_kernel(
    parts: n::PreparedNetworkKernelParts,
    backing: alloc::boxed::Box<core::mem::MaybeUninit<PreparedNetworkKernel>>,
) -> Result<alloc::boxed::Box<PreparedNetworkKernel>, ()> {
    #[cfg(test)]
    let _refusal = if REFUSE_NEXT_LOG.with(|flag| flag.replace(false)) {
        Some(crate::allocation_probe::refuse_next_allocation_size(
            n::CONTROL_SIGN_BACKING_BYTES,
        ))
    } else {
        None
    };
    Ok(alloc::boxed::Box::write(
        backing,
        parts.finish().map_err(|_| ())?,
    ))
}

#[inline(never)]
#[cold]
fn finish_reserved_kernel(
    parts: n::PreparedNetworkKernelParts,
    storage: n::PreparedNetworkKernelStorage,
) -> Result<alloc::boxed::Box<PreparedNetworkKernel>, ()> {
    #[cfg(test)]
    let before = crate::allocation_probe::current_observation();
    let result = storage.finish(parts).map_err(|_| ());
    #[cfg(test)]
    if let Some(before) = before {
        let after = crate::allocation_probe::current_observation().unwrap();
        assert_eq!(
            (before.allocations, before.reallocations),
            (after.allocations, after.reallocations),
            "reserved materialization must allocate nothing"
        );
    }
    result
}

// Keep the concrete affine issuance/checking waist out of the generic planner
// callback. Its owner brand is preserved; no Source/Plan borrow escapes.
#[inline(never)]
fn prepare_linked_selection<'owner, 'root>(
    owners: n::ReadyNetworkPairContractView<'owner, '_, Provider, Source>,
    scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
    issuer: &'root mut RootIssuer,
    pins: (&str, &str, &str),
) -> Result<
    (
        n::PreparedNetworkPairSelection<'owner, Provider, Source>,
        n::PreparedNetworkKernelParts,
        crate::native_authority::LinkedNetworkPreparation<'root>,
    ),
    (),
> {
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
    let root = issuer.prepare_linked_network(scope, pins)?;
    let credential_scope =
        root.expected_scope(owners.credential_identity(), Operation::Credentials)?;
    let join_scope = root.expected_scope(owners.join_identity(), Operation::Join)?;
    let selected = owners
        .prepare_flat_selection(credential_scope, join_scope, &tables, credential, join)
        .map_err(|_| ())?;
    let parts = n::PreparedNetworkKernelParts::from_flat_direct(tables).map_err(|_| ())?;
    Ok((selected, parts, root))
}

fn check_linked_scope<'owner>(
    owners: n::ReadyNetworkPairContractView<'owner, '_, Provider, Source>,
    scope: &conduit_planner::borrowed_local::SourcePlannedLocalScope<'_>,
    issuer: &mut RootIssuer,
    pins: (&str, &str, &str),
    active: &mut Option<c::ActivePlayIdentity>,
    custody: &mut IssuedCustody,
    selected: &mut Option<(
        n::PreparedNetworkPairBindings<'owner>,
        n::PreparedNetworkKernelParts,
    )>,
    refusal_stage: &mut u16,
) -> Result<(), ()> {
    *refusal_stage = 10;
    let tables = n::NetworkControlFlatDirectTables::prepare(scope.verified()).map_err(|_| ())?;
    *refusal_stage = 11;
    let roles = n::PreparedNetworkKernelParts::check_flat_direct(&tables).map_err(|_| ())?;
    let view = scope.verified().view();
    // The pair checks Credentials and Join only. Keep this
    // independent actual attachment implementation preflight.
    *refusal_stage = 12;
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
    *refusal_stage = 13;
    let credential = view
        .placements
        .iter()
        .nth(usize::from(roles.credentials.0))
        .ok_or(())?
        .placement_id;
    *refusal_stage = 14;
    let join = view
        .placements
        .iter()
        .nth(usize::from(roles.join.0))
        .ok_or(())?
        .placement_id;
    *refusal_stage = 15;
    *active = Some(issuer.reserve_linked_network_play(scope, pins)?);
    let current = active.as_ref().expect("reserved exact parent");
    *refusal_stage = 16;
    let (table, handle, claim) = issuer.issue_linked_network(
        owners.credential_identity(),
        Operation::Credentials,
        scope,
        pins,
        current,
    )?;
    *refusal_stage = 17;
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
    *refusal_stage = 18;
    let issued =
        issuer.issue_linked_network(first.join_identity(), Operation::Join, scope, pins, current);
    let (table, handle, claim) = match issued {
        Ok(issued) => issued,
        Err(()) => {
            *custody = IssuedCustody::First(first.into_operation());
            return Err(());
        }
    };
    *refusal_stage = 19;
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
    *refusal_stage = 20;
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
    static REFUSE_NEXT_LOG: std::cell::Cell<bool> = const { std::cell::Cell::new(false) };
}
#[cfg(test)]
pub(super) fn refuse_next_log() {
    REFUSE_NEXT_LOG.with(|flag| flag.set(true));
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

// Phased stages: 100 entry; 101/102 grants; 103 advertisement; 104 planner;
// 105 selection; 110 claim memory; 111/112 issuer memory; 113 reservation memory;
// 114 kernel/Sign backing; 115 admission callback; 120 actual Root reservation;
// 121/122 credential issue/check; 123/124 Join issue/check; 125 materialization.
// Scalar diagnostics only; no payload, authority or retained execution state.
#[inline(never)]
#[cold]
fn trace_phased_refusal(stage: u16, custody: &IssuedCustody, reserved: bool, admitted: bool) {
    #[cfg(test)]
    PHASE_REFUSAL.with(|last| last.set(Some((stage, reserved, admitted))));
    #[cfg(target_os = "none")]
    {
        let custody = match custody {
            IssuedCustody::Empty => 0,
            IssuedCustody::Lease(_) => 1,
            IssuedCustody::First(_) => 2,
            IssuedCustody::FirstLease(_, _) => 3,
            IssuedCustody::Both(_, _) => 4,
        };
        esp_println::println!("{{\"event\":\"network-phased-install-refused\",\"stage\":{},\"admitted\":{},\"issued_custody\":{},\"active_reserved\":{}}}", stage, admitted, custody, reserved);
    }
    #[cfg(not(target_os = "none"))]
    let _ = (stage, custody, reserved, admitted);
}
#[cfg(test)]
std::thread_local! {
    static PHASE_REFUSAL: std::cell::Cell<Option<(u16, bool, bool)>> = const { std::cell::Cell::new(None) };
}
#[cfg(test)]
pub(super) fn phased_refusal() -> Option<(u16, bool, bool)> {
    PHASE_REFUSAL.with(|last| last.take())
}

// One line per actual failed installation, no retained metadata. Stage codes:
// 1/2 grants; 3 advertisement; 4 planner; 5 selected output; 6 backing;
// 7 materialization; 10 tables; 11 kernel profile; 12 attachment;
// 13/14 placement; 15 Root reserve; 16 credential issue; 17 credential check;
// 18 join issue; 19 join check; 20 prepared kernel parts.
#[inline(never)]
#[cold]
fn trace_flat_refusal(stage: u16, custody: &IssuedCustody, active: bool) {
    #[cfg(target_os = "none")]
    {
        let custody = match custody {
            IssuedCustody::Empty => 0,
            IssuedCustody::Lease(_) => 1,
            IssuedCustody::First(_) => 2,
            IssuedCustody::FirstLease(_, _) => 3,
            IssuedCustody::Both(_, _) => 4,
        };
        esp_println::println!("{{\"event\":\"network-flat-install-refused\",\"stage\":{},\"issued_custody\":{},\"active_reserved\":{}}}", stage, custody, active);
    }
    #[cfg(not(target_os = "none"))]
    let _ = (stage, custody, active);
}

#[cfg(test)]
#[derive(Clone, Copy, Default)]
pub(super) struct PhaseMeasurement {
    pub(super) observation: Option<crate::allocation_probe::Observation>,
    pub(super) backing: Option<usize>,
}
#[cfg(test)]
std::thread_local! {
    static PHASE_MEASUREMENTS: std::cell::Cell<[PhaseMeasurement; 9]> = const {
        std::cell::Cell::new([PhaseMeasurement { observation: None, backing: None }; 9])
    };
}
#[cfg(test)]
fn record_phased(index: usize, backing: Option<usize>) {
    PHASE_MEASUREMENTS.with(|measurements| {
        let mut state = measurements.get();
        state[index] = PhaseMeasurement {
            observation: crate::allocation_probe::current_observation(),
            backing,
        };
        measurements.set(state);
    });
}
#[cfg(test)]
pub(super) fn phase_measurements() -> [PhaseMeasurement; 9] {
    PHASE_MEASUREMENTS.with(|measurements| measurements.replace([PhaseMeasurement::default(); 9]))
}
