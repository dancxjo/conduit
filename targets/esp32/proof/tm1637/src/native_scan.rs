//! Root-selected one-shot scan through the ordinary checked Source and kernel.
//! Kept separately from join execution so no additional dormant radio lives in
//! NativeNetwork. The caller admits this complete transient workspace first.
use crate::native_authority::{Operation, RootIssuer};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_kernel::{self as k, scheduler::SchedulerStatus};
use conduit_net::*;
use conduit_network_join_call::scan::*;
use conduit_plan_lowering::selected_operation::IssuedFlatDirectOperation;

const CAP: &str = "conduit/esp32/network-scan-evidence@1";
const IMPLEMENTATION: &str = "esp32/network-scan-evidence@1";
const ARTIFACT: &str = "native/esp32-network-scan-evidence@1";

enum Ownership<P> {
    Ready(ReadyNetworkScanBase<P>),
    Selected(NetworkScanHostCall<P>),
}
pub(crate) struct NativeScan<P> {
    owner: Option<Ownership<P>>,
    kernel: Option<Box<PreparedScanKernel>>,
    active: Option<c::ActivePlayIdentity>,
    refused_issued: Option<IssuedFlatDirectOperation>,
    refused_bound: Option<c::BoundBaseOperation>,
    held: bool,
    evidence_seen: bool,
}
/// Original returned custody, not a durable commitment or renewal permission.
#[must_use = "retain original scan observations and Signs until commitment"]
pub(crate) struct RetiredNativeScan<P> {
    pub provider: P,
    evidence: Option<Evidence>,
    pub signs: k::HostedSignLog,
    pub active: c::ActivePlayIdentity,
    pub evidence_seen: bool,
}
enum Evidence {
    Retained(RetiredScanEvidence),
    Committed(CommittedScanEvidence),
}
impl<P> RetiredNativeScan<P> {
    pub(crate) fn evidence(&self) -> &RetiredScanEvidence {
        match self.evidence.as_ref().expect("retained evidence") {
            Evidence::Retained(evidence) => evidence,
            Evidence::Committed(committed) => committed.evidence(),
        }
    }
    pub(crate) fn committed(&self) -> Option<&CommittedScanEvidence> {
        match self.evidence.as_ref().expect("retained evidence") {
            Evidence::Committed(committed) => Some(committed),
            Evidence::Retained(_) => None,
        }
    }
    fn commit_with(
        &mut self,
        append: impl FnOnce(
            RetiredScanEvidence,
            &c::ActivePlayIdentity,
            bool,
            &k::HostedSignLog,
        )
            -> Result<CommittedScanEvidence, (RetiredScanEvidence, &'static str)>,
    ) -> Result<(), &'static str> {
        if self.committed().is_some() {
            return Ok(());
        }
        let Some(Evidence::Retained(evidence)) = self.evidence.take() else {
            unreachable!()
        };
        match append(evidence, &self.active, self.evidence_seen, &self.signs) {
            Ok(committed) => {
                self.evidence = Some(Evidence::Committed(committed));
                Ok(())
            }
            Err((evidence, error)) => {
                self.evidence = Some(Evidence::Retained(evidence));
                Err(error)
            }
        }
    }
    #[cfg(any(feature = "local-evidence-store", feature = "network-credential-store"))]
    pub(crate) fn commit_evidence(&mut self) -> Result<(), &'static str> {
        self.commit_with(crate::native_play::journal::append_scan)
    }
    #[cfg(test)]
    pub(crate) fn commit_to_fixture_journal<
        S: conduit_network_join_call::recovery_evidence::Storage,
    >(
        &mut self,
        journal: &mut conduit_network_join_call::recovery_evidence::Journal<S>,
        work: &mut [u8; conduit_network_join_call::recovery_evidence::MAXIMUM_BYTES],
    ) -> Result<(), &'static str> {
        self.commit_with(|evidence, active, seen, signs| {
            evidence
                .commit(active, seen, signs, journal, work)
                .map_err(|(evidence, _)| (evidence, "fixture-scan-commit-refused"))
        })
    }
}
impl<P: NetworkScanProvider> NativeScan<P> {
    pub(crate) fn new(ready: ReadyNetworkScanBase<P>) -> Self {
        Self {
            owner: Some(Ownership::Ready(ready)),
            kernel: None,
            active: None,
            refused_issued: None,
            refused_bound: None,
            held: true,
            evidence_seen: false,
        }
    }
    /// Recover the same cold owner only before an irreversible Root reservation
    /// or issued possession. A failed description/admission cannot strand the
    /// restored AP; a selected/partially issued operation cannot use this exit.
    pub(crate) fn take_unselected_ready(&mut self) -> Result<ReadyNetworkScanBase<P>, ()> {
        if self.active.is_some()
            || self.kernel.is_some()
            || self.refused_issued.is_some()
            || self.refused_bound.is_some()
            || !matches!(self.owner, Some(Ownership::Ready(_)))
        {
            return Err(());
        }
        let Some(Ownership::Ready(ready)) = self.owner.take() else {
            unreachable!()
        };
        Ok(ready)
    }
    /// All backing exists before the admission callback can spend an attempt.
    /// Descriptions retire before issuance. Every terminal refusal retains the
    /// actual Ready owner and any irreversible Root-issued bearer in this slot.
    pub(crate) fn install_held(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        pins: (&str, &str, &str),
        generation: c::OfferGeneration,
        issuer: &mut RootIssuer,
        admit: impl FnOnce() -> Result<(), ()>,
    ) -> Result<(), ()> {
        if self.active.is_some()
            || self.kernel.is_some()
            || self.refused_issued.is_some()
            || self.refused_bound.is_some()
            || !matches!(self.owner, Some(Ownership::Ready(_)))
        {
            return Err(());
        }
        let storage = PreparedScanKernelStorage::try_new().map_err(|_| ())?;
        let mut local = alloc::string::String::new();
        local
            .try_reserve_exact(c::LOCAL_BASE_IMPLEMENTATION_ID.len())
            .map_err(|_| ())?;
        local.push_str(c::LOCAL_BASE_IMPLEMENTATION_ID);
        let bases = [c::BaseImplementationId(local)];
        let Some(Ownership::Ready(ready)) = self.owner.take() else {
            unreachable!()
        };
        let mut active = None;
        let mut reserved = false;
        let result = ready.prepare(|view| {
            let (selection, parts, mut root) = {
                let native = view.native_identity();
                let grant = issuer.grant_ref(native, Operation::Scan)?;
                let grants = [grant];
                let header = c::HostAdvertisementRef {
                    protocol_version: c::PROTOCOL_VERSION,
                    host_id: native.host.as_str(),
                    boot_id: native.boot.as_str(),
                    offer_generation: generation,
                    profile: "environment/esp32-native-root@1",
                    bases: c::BaseProviderAdvertisementRefs::Borrowed(&[]),
                    resources: c::ResourceOfferRefs::Borrowed(&[]),
                    capabilities: c::CapabilityOfferRefs::Borrowed(&[]),
                    planner_capabilities: &[],
                };
                let evidence =
                    PreparedCanonicalNetworkOffer::scan_sign(CAP, IMPLEMENTATION, ARTIFACT);
                view.with_advertisement(header, &evidence, |owner, host| {
                    conduit_planner::borrowed_local::with_linked_local_plan(
                        image,
                        host,
                        &bases,
                        c::AuthorityGrantRefs::Borrowed(&grants),
                        1,
                        MAXIMUM_SCAN_OUTPUT_BYTES,
                        |scope| {
                            let tables =
                                ScanFlatDirectTables::prepare(scope.verified()).map_err(|_| ())?;
                            let roles = PreparedScanKernelParts::check_flat_direct(&tables)
                                .map_err(|_| ())?;
                            let plan = scope.verified().view();
                            let sink = plan
                                .placements
                                .iter()
                                .nth(usize::from(roles.evidence.0))
                                .ok_or(())?;
                            if sink.capability_id != CAP
                                || sink.implementation_id != IMPLEMENTATION
                                || sink.artifact_id != ARTIFACT
                                || sink.execution_profile_id != NETWORK_SCAN_SIGN_EXECUTION_PROFILE
                                || sink.host_calls.len() != 1
                                || sink.host_calls.iter().next().ok_or(())?.contract_id
                                    != NETWORK_SCAN_SIGN_HOST_CALL
                            {
                                return Err(());
                            }
                            let placement = plan
                                .placements
                                .iter()
                                .nth(usize::from(roles.scan.0))
                                .ok_or(())?
                                .placement_id;
                            let root = issuer.prepare_linked_scan(scope, pins)?;
                            let expected =
                                root.expected_scope(owner.native_identity(), Operation::Scan)?;
                            let selection = owner
                                .prepare_flat_selection(expected, &tables, placement)
                                .map_err(|_| ())?;
                            let parts = PreparedScanKernelParts::from_flat_direct(tables)
                                .map_err(|_| ())?;
                            Ok((selection, parts, root))
                        },
                    )
                    .map_err(|_| ())
                })
                .map_err(|_| ())???
            };
            let memory = selection.prepare_claim_storage().map_err(|_| ())?;
            selection.prepare_issuance_storage(|native, prepared| {
                root.prepare_issuance_storage(native, Operation::Scan, prepared)
            })?;
            active = Some(root.prepare_reservation()?);
            admit()?;
            root.reserve()?;
            reserved = true;
            let binding = match selection.issue_reusing(memory, |native, prepared, memory| {
                root.issue_prepared(
                    native,
                    Operation::Scan,
                    prepared,
                    active.as_ref().ok_or(())?,
                    memory,
                )
            }) {
                Ok(binding) => binding,
                Err((issued, _)) => {
                    self.refused_issued = issued;
                    return Err(());
                }
            };
            let kernel = match storage.finish(parts) {
                Ok(kernel) => kernel,
                Err(_) => {
                    self.refused_bound = Some(binding.into_operation());
                    return Err(());
                }
            };
            Ok((binding, kernel))
        });
        self.active = if reserved { active } else { None };
        match result {
            Ok((selected, kernel)) => {
                self.owner = Some(Ownership::Selected(selected));
                self.kernel = Some(kernel);
                Ok(())
            }
            Err((ready, ())) => {
                self.owner = Some(Ownership::Ready(ready));
                Err(())
            }
        }
    }
    pub(crate) fn release(&mut self) -> Result<(), ()> {
        if self.active.is_none()
            || self.kernel.is_none()
            || !matches!(self.owner, Some(Ownership::Selected(_)))
        {
            return Err(());
        }
        self.held = false;
        Ok(())
    }
    /// Bounded scheduler work; the selected provider retains cancelled futures'
    /// controller, original deadline and mandatory cleanup obligation.
    pub(crate) async fn drive(&mut self) -> Result<(), ()> {
        if self.held {
            return Err(());
        }
        let Some(Ownership::Selected(owner)) = self.owner.as_mut() else {
            return Err(());
        };
        let prepared = self.kernel.as_mut().ok_or(())?;
        let roles = prepared.roles;
        let kernel = &mut prepared.kernel;
        for _ in 0..100 {
            if let Some(request) = kernel.next_host_request() {
                let input = kernel.host_value(request.input.value).map_err(|_| ())?;
                let output = if request.node == roles.scan {
                    let bytes = owner
                        .invoke(request.node, request.call, request.request, input)
                        .await
                        .map_err(|_| ())?;
                    let value = kernel.store_host_value(bytes).map_err(|_| ())?;
                    Some(
                        k::BoundedValueRef::new(value, MAXIMUM_SCAN_OUTPUT_BYTES)
                            .map_err(|_| ())?,
                    )
                } else if request.node == roles.evidence
                    && request.call == k::HostCallId(0)
                    && request.request == k::RequestId(0)
                    && !self.evidence_seen
                    && owner
                        .completed_output()
                        .is_some_and(|expected| input == expected)
                {
                    self.evidence_seen = true;
                    None
                } else {
                    return Err(());
                };
                kernel
                    .complete_host_call(
                        request.node,
                        request.request,
                        k::HostCallOutcome {
                            disposition: k::HostCallDisposition::Completed,
                            output,
                            failure: None,
                        },
                    )
                    .map_err(|_| ())?;
            }
            match kernel.step().map_err(|_| ())? {
                SchedulerStatus::Drained => return self.evidence_seen.then_some(()).ok_or(()),
                SchedulerStatus::Cancelled => return Err(()),
                _ => {}
            }
        }
        Err(())
    }
    pub(crate) async fn restore_for_retirement(&mut self) -> Result<(), ()> {
        let Some(Ownership::Selected(owner)) = self.owner.as_mut() else {
            return Err(());
        };
        let kernel = self.kernel.as_mut().ok_or(())?;
        if !kernel.kernel.can_retire() {
            kernel.kernel.cancel().map_err(|_| ())?;
        }
        owner
            .revoke(kernel.roles.scan, k::HostCallId(0))
            .map_err(|_| ())?;
        owner.cleanup().await.map_err(|_| ())
    }
    pub(crate) fn take_retired(&mut self) -> Result<RetiredNativeScan<P>, ()> {
        if self.active.is_none()
            || !self.kernel.as_ref().is_some_and(|k| k.kernel.can_retire())
            || !matches!(self.owner.as_ref(), Some(Ownership::Selected(o)) if o.retirement_ready())
        {
            return Err(());
        }
        let Some(Ownership::Selected(owner)) = self.owner.take() else {
            unreachable!()
        };
        let (provider, evidence) =
            NetworkScanHostCall::retire_with_evidence_retained(&mut Some(owner))
                .unwrap_or_else(|_| unreachable!("exclusive scan retirement preflight"));
        let signs = match self
            .kernel
            .take()
            .expect("checked kernel")
            .try_retire_signs()
        {
            Ok(signs) => signs,
            Err(_) => unreachable!("exclusive terminal scheduler preflight"),
        };
        Ok(RetiredNativeScan {
            provider,
            evidence: Some(Evidence::Retained(evidence)),
            signs,
            active: self.active.take().expect("checked active"),
            evidence_seen: self.evidence_seen,
        })
    }
}
