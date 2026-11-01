//! Checked preparation retires before complete execution storage is reserved.
use super::*;
impl NativePlay {
    /// Separate immutable checked preparation from admitted execution. Only
    /// unique uninitialized backing overlaps temporary descriptions; the kernel
    /// materializes later. Before `admit`, no attempt/sequence/bearer changes.
    pub(super) fn install_linked_phased_mode(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        generation: c::OfferGeneration,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
        storage: Option<backing::PhasedStorage>,
        descriptions_retired: impl FnOnce() -> Result<(), ()>,
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
        let (storage, local_base) = backing::reserve_phased_storage(storage, &self.flat_refusal)?;
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
                let bases = [local_base];
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
                                prepare_linked_selection(
                                    owners,
                                    scope,
                                    issuer,
                                    pins,
                                    &mut refusal_stage,
                                )
                            },
                        )
                        .map_err(|_| ())
                    })
                    .map_err(|_| ())???
            };
            // Both native contracts, borrowed Source/Plan projections, grants
            // and lowering tables have retired. Only exact compact proofs and
            // numeric parts overlap final backing and actual issuance.
            // Only the kernel backing overlapped descriptions. Allocate the
            // unchanged complete Sign grant now, before admission or issuance.
            #[cfg(test)]
            record_phased(0, None);
            refusal_stage = 118;
            descriptions_retired()?;
            refusal_stage = 117;
            let storage = backing::complete_phased_storage(storage)?;
            #[cfg(test)]
            record_phased(1, Some(storage.as_ptr() as usize));
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
}
