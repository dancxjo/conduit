use std::{cell::RefCell, rc::Rc};

use conduit_body::{
    disclose_host_offer, Body, BodyResourceAllowance, BodyResourceEnvelope,
    BodyResourceEnvelopeError, BodyResourceReservationError, BodyResourceReservationLedger,
    CandidateObservation, ClaimPolicyRefusal, ClaimUseClass, DiscoveryProofId,
    OfferDisclosureRequest, OfferDisclosureStage, PartId, RemoteClaimPolicy, RemoteClaimProvenance,
    RemoteProofClass,
};
use conduit_core::{
    mandatory_sign_storage_requirement, prepare_plan_on_hosts, process_owned_line_offer,
    resource_offer, resource_requirement, seal_plan, start_prepared_plan, ActivePlayId,
    BaseImplementationId, BootId, CancellationPolicy, CapabilityId, CheckedFormId, ConnectionId,
    ExpandedFormId, ExpectedSign, ExpectedTerminal, FormIdentity, FragmentId, GearId,
    HostAdvertisement, HostId, HostPreparationRefusal, HostProfileId, KindId, LineScope,
    LinkBindingId, OfferGeneration, PlacementId, Plan, PlanFragment, PlanPreparationHost,
    PlannedConnection, PortId, PortTemporal, PreparationHostIdentity, PreparedFragmentReceipt,
    ProtectedResourceAccess, ProtectedResourceCommitPolicy, ResourceBinding, ResourceBindingRoleId,
    ResourceHandleId, ResourceHealth, ResourceObservation, SignId, SourceDocumentId,
    TerminalPolicy, PROTOCOL_VERSION,
};
use conduit_std_host::{
    prepare_copy_task, CopyRequestId, CopyResult, CopyStopToken, ProtectedFileAvailability,
    ProtectedFileRegistry, StdHost, StdHostConfig,
};
use conduit_wire::{SessionBinding, WireError};

fn advertisement(id: &str, generation: u64, capacity: u32) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(id),
        boot_id: BootId::from(format!("{id}/boot/1")),
        offer_generation: OfferGeneration(generation),
        profile: HostProfileId::from(format!("profile/{id}")),
        resources: vec![resource_offer("execution", "host/execution", capacity)],
        capabilities: vec![],
        planner_capabilities: vec![],
    }
}

fn envelope(
    body: &Body,
    label: &str,
    host: &HostAdvertisement,
    maximum: u32,
) -> BodyResourceEnvelope {
    BodyResourceEnvelope::new(
        body.body_id.clone(),
        PartId::bind(&body.body_id, label, 1).unwrap(),
        host,
        vec![BodyResourceAllowance {
            pool_id: "execution".into(),
            class_id: "host/execution".into(),
            maximum_units: maximum,
        }],
    )
    .unwrap()
}

fn observation(host: &HostAdvertisement, unreserved: u32) -> ResourceObservation {
    ResourceObservation {
        host_id: host.host_id.clone(),
        boot_id: host.boot_id.clone(),
        offer_generation: host.offer_generation,
        pool_id: "execution".into(),
        class_id: "host/execution".into(),
        health: ResourceHealth::Ready,
        unreserved_units: unreserved,
        utilized_units: 0,
        sign_id: SignId::from(format!("{}/resource/ready", host.host_id.as_str())),
    }
}

fn binding(units: u32) -> ResourceBinding {
    ResourceBinding {
        content: None,
        pool_id: "execution".into(),
        class_id: "host/execution".into(),
        units,
        protected: None,
        compute: None,
    }
}

fn exact_plan(hosts: &[HostAdvertisement], label: &str) -> Plan {
    let expected_sign = vec![
        ExpectedSign::PlanFragmentReceived,
        ExpectedSign::PlanTerminal,
    ];
    let fragments = hosts
        .iter()
        .map(|host| PlanFragment {
            plan_id: conduit_core::PlanId::from(""),
            fragment_id: FragmentId::from(""),
            source_document_id: SourceDocumentId::from(""),
            checked_form_id: CheckedFormId::from(""),
            expanded_form_id: ExpandedFormId::from(""),
            realization_backs: vec![],
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            offer_generation: host.offer_generation,
            placements: vec![],
            execution_regions: vec![],
            execution_fusions: vec![],
            states: Vec::new(),
            connections: vec![],
            shared_pools: vec![],
            startup_dependencies: vec![],
            startup_order: vec![],
            cancellation_policy: CancellationPolicy::CancelAllAndRejectLateCompletion,
            terminal_policy: TerminalPolicy::RequireAllPlacementsAndConnections,
            expected_terminals: vec![ExpectedTerminal::PlanCompleted],
            expected_sign: expected_sign.clone(),
            sign_storage_budget: mandatory_sign_storage_requirement(&expected_sign).unwrap(),
            plan_fragments: vec![],
        })
        .collect();
    seal_plan(
        FormIdentity {
            source_document_id: SourceDocumentId::from("form/host-fences-capstone"),
            checked_form_id: CheckedFormId::from("checked/host-fences-capstone"),
            expanded_form_id: ExpandedFormId::from(label),
        },
        fragments,
    )
}

struct PreparedHost {
    identity: PreparationHostIdentity,
    receipt: Option<PreparedFragmentReceipt>,
    starts: Rc<RefCell<u8>>,
}

impl PreparedHost {
    fn new(host: &HostAdvertisement, starts: Rc<RefCell<u8>>) -> Self {
        Self {
            identity: PreparationHostIdentity {
                host_id: host.host_id.clone(),
                boot_id: host.boot_id.clone(),
                offer_generation: host.offer_generation,
            },
            receipt: None,
            starts,
        }
    }
}

impl PlanPreparationHost for PreparedHost {
    fn preparation_identity(&self) -> PreparationHostIdentity {
        self.identity.clone()
    }

    fn prepare_fragment(
        &mut self,
        fragment: &PlanFragment,
    ) -> Result<PreparedFragmentReceipt, HostPreparationRefusal> {
        let receipt = PreparedFragmentReceipt::new(fragment);
        self.receipt = Some(receipt.clone());
        Ok(receipt)
    }

    fn release_fragment(
        &mut self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        if self.receipt.as_ref() != Some(receipt) {
            return Err(HostPreparationRefusal::PreparedBindingMismatch);
        }
        self.receipt = None;
        Ok(())
    }

    fn validate_start(
        &self,
        receipt: &PreparedFragmentReceipt,
    ) -> Result<(), HostPreparationRefusal> {
        (self.receipt.as_ref() == Some(receipt))
            .then_some(())
            .ok_or(HostPreparationRefusal::PreparedBindingMismatch)
    }

    fn start_fragment(&mut self, receipt: &PreparedFragmentReceipt) -> ActivePlayId {
        self.validate_start(receipt).unwrap();
        *self.starts.borrow_mut() += 1;
        ActivePlayId::from(format!("{}/play/1", self.identity.host_id.as_str()))
    }
}

fn candidate(host: &HostAdvertisement) -> CandidateObservation {
    CandidateObservation {
        advertisement: host.clone(),
        friendly_label: host.host_id.as_str().into(),
        observed_binding_id: LinkBindingId::from("body/rendezvous"),
        observation_sign_id: SignId::from(format!("{}/offer", host.host_id.as_str())),
        proof_id: DiscoveryProofId::bind(&format!("proof/{}", host.host_id.as_str())).unwrap(),
        freshness_sequence: 1,
        encoded_bytes: 256,
    }
}

#[test]
fn three_fenced_hosts_prepare_then_start_one_unchanged_form() {
    let body = Body::born(
        SourceDocumentId::from("source/body"),
        CheckedFormId::from("checked/body"),
        1,
        SignId::from("body/born"),
    )
    .unwrap();
    let hosts = [
        advertisement("workstation", 7, 32),
        advertisement("browser", 4, 8),
        advertisement("constrained", 2, 2),
    ];
    let maxima = [24, 6, 1];
    let units = [8, 3, 1];
    let plan = exact_plan(&hosts, "expanded/host-fences/old");
    let mut ledgers = hosts
        .iter()
        .zip(maxima)
        .map(|(host, maximum)| {
            let envelope = envelope(&body, host.host_id.as_str(), host, maximum);
            (BodyResourceReservationLedger::new(&envelope), envelope)
        })
        .collect::<Vec<_>>();
    for (((ledger, envelope), host), requested) in ledgers.iter_mut().zip(&hosts).zip(units) {
        let selected = binding(requested);
        let requirement = resource_requirement("host/execution", requested);
        ledger
            .reserve(
                plan.plan_id.clone(),
                envelope,
                host,
                &[observation(host, host.resources[0].capacity_units)],
                &[(&requirement, &selected)],
            )
            .unwrap();
    }

    let starts = Rc::new(RefCell::new(0));
    let mut workstation = PreparedHost::new(&hosts[0], starts.clone());
    let mut browser = PreparedHost::new(&hosts[1], starts.clone());
    let mut constrained = PreparedHost::new(&hosts[2], starts.clone());
    let prepared = prepare_plan_on_hosts(
        &plan,
        &mut [&mut workstation, &mut browser, &mut constrained],
    )
    .unwrap();
    assert_eq!(*starts.borrow(), 0, "preparation performs no semantic work");
    assert_eq!(prepared.receipts().len(), 3);
    let started = start_prepared_plan(
        prepared,
        &mut [&mut workstation, &mut browser, &mut constrained],
    )
    .unwrap();
    assert_eq!(started.active_plays().len(), 3);
    assert_eq!(*starts.borrow(), 3);

    ledgers[2].0.release(&plan.plan_id).unwrap();
    let too_large = binding(2);
    let (constrained_ledger, constrained_envelope) = &mut ledgers[2];
    assert_eq!(
        constrained_ledger.reserve(
            conduit_core::PlanId::from("plan/too-large"),
            constrained_envelope,
            &hosts[2],
            &[observation(&hosts[2], 2)],
            &[(&resource_requirement("host/execution", 2), &too_large)],
        ),
        Err(BodyResourceReservationError::Envelope(
            BodyResourceEnvelopeError::ReservationExceedsAllowance
        ))
    );
    let one = binding(1);
    assert_eq!(
        constrained_ledger.reserve(
            conduit_core::PlanId::from("plan/unavailable"),
            constrained_envelope,
            &hosts[2],
            &[observation(&hosts[2], 0)],
            &[(&resource_requirement("host/execution", 1), &one)],
        ),
        Err(BodyResourceReservationError::Envelope(
            BodyResourceEnvelopeError::ReservationUnavailable
        ))
    );
}

#[test]
fn remote_session_claim_and_disclosure_keep_exact_truth() {
    let workstation = advertisement("workstation", 7, 32);
    let browser = advertisement("browser", 4, 8);
    let mut line = process_owned_line_offer(
        "line/browser-webrtc",
        "binding/browser-webrtc",
        BaseImplementationId::from("conduit.base/webrtc-data-channel@1"),
        "browser-datachannel/1",
        &workstation,
        &browser,
        1,
        128,
    );
    line.contract.scope = LineScope::LocalNetwork;
    line.binding.limits.maximum_frame_bytes = 2_048;
    let admitted: conduit_core::AdmittedLine = (&line).into();
    let connection = PlannedConnection {
        connection_id: ConnectionId::from("cord/text"),
        source_placement_id: PlacementId::from("placement/workstation"),
        source_port_id: PortId::from("out"),
        sink_placement_id: PlacementId::from("placement/browser"),
        sink_port_id: PortId::from("in"),
        value_kind: KindId::from("text/utf8"),
        temporal: PortTemporal::Flow { closes: true },
        selected_line: Some(admitted.clone()),
        admitted_lines: vec![admitted],
        item_capacity: 1,
        byte_capacity: 128,
    };
    let session = SessionBinding::from_planned_connection(
        conduit_core::PlanId::from("plan/host-fences"),
        FragmentId::from("fragment/workstation"),
        FragmentId::from("fragment/browser"),
        &connection,
    )
    .unwrap();
    assert_eq!(
        session.attachment.base,
        BaseImplementationId::from("conduit.base/webrtc-data-channel@1")
    );
    assert_eq!(session.attachment.limits.maximum_in_flight_items, 1);
    let mut mismatch = session.clone();
    mismatch.attachment.sink_boot_id = BootId::from("browser/boot/stale");
    assert_eq!(mismatch.validate(), Err(WireError::InvalidSession));

    let provenance = RemoteClaimProvenance {
        asserting_host_id: browser.host_id.clone(),
        asserting_boot_id: browser.boot_id.clone(),
        offer_generation: browser.offer_generation,
        capability_id: None,
        implementation_id: None,
        base_id: None,
        resource_pool_id: None,
        plan_id: Some(session.plan_id.clone()),
        active_play_id: None,
        sign_id: SignId::from("browser/result/1"),
        freshness_sequence: 1,
        proof_class: RemoteProofClass::PlatformObserved,
    };
    let policy = RemoteClaimPolicy {
        accepted_sources: vec![browser.host_id.clone()],
        accepted_proof_classes: vec![RemoteProofClass::PlatformObserved],
        require_current_member: false,
        minimum_independent_sources: 1,
        use_class: ClaimUseClass::Planning,
    };
    assert_eq!(
        policy.admits(&provenance, None, ClaimUseClass::Planning),
        Ok(())
    );
    let mut weak = provenance;
    weak.proof_class = RemoteProofClass::SelfReported;
    assert_eq!(
        policy.admits(&weak, None, ClaimUseClass::Planning),
        Err(ClaimPolicyRefusal::ProofClassNotAccepted)
    );

    let projection = disclose_host_offer(
        &candidate(&browser),
        RemoteProofClass::TransportAttributed,
        &OfferDisclosureRequest {
            stage: OfferDisclosureStage::Planning,
            capability_ids: vec![],
            resource_pool_ids: vec!["execution".into()],
        },
    )
    .unwrap();
    assert_eq!(projection.resources.len(), 1);
    assert_eq!(projection.resources[0].capacity_units, 8);
}

#[test]
fn browser_loss_requires_fresh_plan_without_mutating_old_truth() {
    let workstation = advertisement("workstation", 7, 32);
    let browser = advertisement("browser", 4, 8);
    let constrained = advertisement("constrained", 2, 2);
    let old = exact_plan(
        &[workstation.clone(), browser.clone(), constrained.clone()],
        "expanded/unchanged-form",
    );
    let old_snapshot = old.clone();
    let replacement = exact_plan(&[workstation, constrained], "expanded/unchanged-form");

    assert_eq!(old.source_document_id, replacement.source_document_id);
    assert_eq!(old.checked_form_id, replacement.checked_form_id);
    assert_eq!(old.expanded_form_id, replacement.expanded_form_id);
    assert_ne!(old.plan_id, replacement.plan_id);
    assert_eq!(old, old_snapshot);
    assert!(old
        .fragments
        .iter()
        .any(|fragment| fragment.host_id == browser.host_id));
    assert!(replacement
        .fragments
        .iter()
        .all(|fragment| fragment.host_id != browser.host_id));

    let mut stale_browser = PreparedHost::new(&browser, Rc::new(RefCell::new(0)));
    stale_browser.identity.offer_generation = OfferGeneration(5);
    let mut old_workstation =
        PreparedHost::new(&old_snapshot_host(&old, 0), Rc::new(RefCell::new(0)));
    let mut old_constrained =
        PreparedHost::new(&old_snapshot_host(&old, 2), Rc::new(RefCell::new(0)));
    assert!(matches!(
        prepare_plan_on_hosts(
            &old,
            &mut [
                &mut old_workstation,
                &mut stale_browser,
                &mut old_constrained
            ]
        ),
        Err(conduit_core::PlanPreparationError::HostRefused {
            reason: HostPreparationRefusal::StaleOffer,
            ..
        })
    ));
}

fn old_snapshot_host(plan: &Plan, index: usize) -> HostAdvertisement {
    let fragment = &plan.fragments[index];
    advertisement(fragment.host_id.as_str(), fragment.offer_generation.0, 8)
}

#[test]
fn protected_use_revalidates_the_exact_current_grant() {
    let path = std::env::temp_dir().join(format!("conduit-f5-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&path);
    std::fs::create_dir(&path).unwrap();
    let source = path.join("source");
    let destination = path.join("destination");
    std::fs::write(&source, b"bounded").unwrap();
    let mut host = StdHost::new_with_config(StdHostConfig {
        host_id: HostId::from("workstation"),
        boot_id: BootId::from("workstation/boot/1"),
        offer_generation: OfferGeneration(7),
    });
    let mut registry = ProtectedFileRegistry::default();
    let register = |registry: &mut ProtectedFileRegistry,
                    handle: &str,
                    role: &str,
                    file: &std::path::Path,
                    access,
                    policy| {
        registry
            .register(
                ResourceHandleId::from(handle),
                file,
                GearId::from("copy-task/task"),
                ResourceBindingRoleId::from(role),
                host.advertisement().host_id.clone(),
                host.advertisement().boot_id.clone(),
                CapabilityId::from(conduit_std_offers::COPY_FILE_CAPABILITY),
                access,
                64,
                policy,
                ProtectedFileAvailability::Available,
            )
            .unwrap()
    };
    let source_grant = register(
        &mut registry,
        "handle/source",
        conduit_semantic_catalog::COPY_SOURCE_ROLE,
        &source,
        ProtectedResourceAccess::ReadExisting,
        ProtectedResourceCommitPolicy::NotApplicable,
    );
    let destination_grant = register(
        &mut registry,
        "handle/destination",
        conduit_semantic_catalog::COPY_DESTINATION_ROLE,
        &destination,
        ProtectedResourceAccess::Create,
        ProtectedResourceCommitPolicy::CreateOnly,
    );
    let prepared = prepare_copy_task(&host, &[source_grant, destination_grant]).unwrap();
    registry
        .set_availability(
            &ResourceHandleId::from("handle/destination"),
            ProtectedFileAvailability::Denied,
        )
        .unwrap();
    let play = host.issue_kernel_play(&prepared.fragment).unwrap();
    let receipt = host
        .run_copy_fragment(
            play,
            CopyRequestId::new("request/stale-grant").unwrap(),
            prepared.fragment,
            &mut registry,
            &CopyStopToken::default(),
        )
        .unwrap();
    assert_eq!(receipt.result, CopyResult::Denied);
    assert!(!destination.exists());
    std::fs::remove_dir_all(path).unwrap();
}
