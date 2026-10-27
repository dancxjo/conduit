//! Ordinary linked Source planning and affine actual-owner selection. No Step
//! is polled here; successful construction remains held for Root admission.
use super::*;
use n::{NetworkOperationLease, ReadyNetworkPair};

/// Issuance is irreversible at this waist. A terminal refusal retains every
/// actual bearer beside the original Ready slots until explicit owner recovery.
#[derive(Default)]
pub(super) struct IssuedCustody {
    operations: [Option<c::BoundBaseOperation>; 2],
    lease: Option<NetworkOperationLease>,
}
impl IssuedCustody {
    /// Reserve terminal evidence custody before taking providers or minting a
    /// bearer. Null is an ordinary refusal, never an allocation panic.
    fn try_new() -> Result<alloc::boxed::Box<Self>, ()> {
        let layout = core::alloc::Layout::new::<Self>();
        // SAFETY: nonzero exact Layout; null is checked before initialization.
        let pointer = unsafe { alloc::alloc::alloc(layout) }.cast::<Self>();
        if pointer.is_null() {
            return Err(());
        }
        // SAFETY: checked fresh allocation has space/alignment for Self; None
        // owns no bearers, and conversion transfers unique allocation custody.
        unsafe {
            pointer.write(Self::default());
            Ok(alloc::boxed::Box::from_raw(pointer))
        }
    }
    fn is_empty(&self) -> bool {
        self.operations.iter().all(Option::is_none) && self.lease.is_none()
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
        if self.attempted || source.is_none() || join.is_none() {
            return Err(());
        }
        self.attempted = true;
        self.configuration_revision = crate::native_credentials::committed_revision();
        self.phase = "installation-refused";
        let mut custody = IssuedCustody::try_new()?;
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
            owners
                .with_advertisement(header, &attachment, |owners, host| {
                    conduit_planner::borrowed_local::with_linked_local_plan(
                        image,
                        host,
                        &bases,
                        c::AuthorityGrantRefs::Borrowed(&grants),
                        1,
                        conduit_net::MAXIMUM_JOIN_INPUT_BYTES,
                        |scope| {
                            let tables =
                                n::NetworkControlFlatDirectTables::prepare(scope.verified())
                                    .map_err(|_| ())?;
                            let roles = n::PreparedNetworkKernelParts::check_flat_direct(&tables)
                                .map_err(|_| ())?;
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
                                || sign.execution_profile_id
                                    != conduit_net::NETWORK_ATTACHMENT_LOCAL_PROFILE
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
                            active = Some(issuer.reserve_linked_network_play(scope, pins)?);
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
                                    custody.lease = Some(lease);
                                    return Err(());
                                }
                            };
                            let issued = issuer.issue_linked_network(
                                first.join_identity(),
                                Operation::Join,
                                scope,
                                pins,
                                current,
                            );
                            let (table, handle, claim) = match issued {
                                Ok(issued) => issued,
                                Err(()) => {
                                    custody.operations[0] = Some(first.into_operation());
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
                                    custody.operations[0] = Some(first.into_operation());
                                    custody.lease = Some(lease);
                                    return Err(());
                                }
                            };
                            match n::PreparedNetworkKernelParts::from_flat_direct(tables) {
                                Ok(parts) => Ok((checked, parts)),
                                Err(_) => {
                                    let (first, second) = checked.into_operations();
                                    custody.operations = [Some(first), Some(second)];
                                    Err(())
                                }
                            }
                        },
                    )
                    .map_err(|_| ())?
                })
                .map_err(|_| ())?
        });
        // No Plan, owner projection, grants, or borrowed lowering survives this
        // point. The unchanged full kernel storage is allocated only afterward.
        drop(bases);
        self.active = active;
        match result {
            Ok((credentials, join, parts)) => {
                self.source = Some(credentials);
                self.join = Some(join);
                drop(custody);
                self.materialize_held(parts)
            }
            Err((pair, ())) => {
                let (credentials, provider) = pair.into_ready();
                *source = Some(credentials);
                *join = Some(provider);
                if !custody.is_empty() {
                    self.flat_refusal = Some(custody);
                }
                Err(())
            }
        }
    }
}
