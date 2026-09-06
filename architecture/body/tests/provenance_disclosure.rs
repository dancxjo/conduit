use conduit_body::{
    disclose_host_offer, AuthenticatedHostObservation, Body, BodyMembership, CandidateObservation,
    ClaimPolicyRefusal, ClaimUseClass, DiscoveryProofId, HostOfferProjection, MembershipProofId,
    OfferDisclosureRefusal, OfferDisclosureRequest, OfferDisclosureStage, PartId,
    RemoteClaimPolicy, RemoteClaimProvenance, RemoteProofClass,
};
use conduit_core::{
    ArtifactId, BootId, CapabilityId, CapabilityLimits, CapabilityOffer, CheckedFormId,
    ExecutionProfileId, HostAdvertisement, HostId, HostProfileId, ImplementationId,
    KindContractRevision, KindId, LinkBindingId, OfferGeneration, ResourceClassId, ResourceOffer,
    ResourcePoolId, SignId, SourceDocumentId, PROTOCOL_VERSION,
};

fn capability(id: &str) -> CapabilityOffer {
    CapabilityOffer {
        startup_parameters: vec![],
        shorthand: None,
        capability_id: CapabilityId::from(id),
        kind_id: KindId::from(format!("kind/{id}")),
        kind_contract_revision: KindContractRevision::from("kind/revision"),
        inputs: vec![],
        outputs: vec![],
        implementation: conduit_core::ImplementationOffer {
            execution_profile_id: ExecutionProfileId::from("profile/portable"),
            implementation_id: ImplementationId::from(format!("implementation/{id}")),
            artifact_id: ArtifactId::from("artifact/reviewed"),
        },
        host_operations: vec![],
        resource_requirements: vec![],
        authority_requirements: vec![],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: 2,
            max_queue_bytes: 64,
        },
    }
}

fn observation() -> CandidateObservation {
    CandidateObservation {
        advertisement: HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: HostId::from("host/browser-a"),
            boot_id: BootId::from("boot/browser-a/1"),
            offer_generation: OfferGeneration(4),
            profile: HostProfileId::from("profile/friendly-browser"),
            resources: vec![
                ResourceOffer {
                    content: None,
                    pool_id: ResourcePoolId::from("pool/audio"),
                    class_id: ResourceClassId::from("resource/audio"),
                    capacity_units: 1,
                    compute: None,
                },
                ResourceOffer {
                    content: None,
                    pool_id: ResourcePoolId::from("pool/private-camera"),
                    class_id: ResourceClassId::from("resource/camera"),
                    capacity_units: 1,
                    compute: None,
                },
            ],
            capabilities: vec![capability("cap/audio"), capability("cap/private-camera")],
            planner_capabilities: vec![],
        },
        friendly_label: "Browser A".into(),
        observed_binding_id: LinkBindingId::from("line/rendezvous"),
        observation_sign_id: SignId::from("sign/offer/4"),
        proof_id: DiscoveryProofId::bind("proof/rendezvous-attribution").unwrap(),
        freshness_sequence: 9,
        encoded_bytes: 1024,
    }
}

fn request(stage: OfferDisclosureStage) -> OfferDisclosureRequest {
    OfferDisclosureRequest {
        stage,
        capability_ids: vec![],
        resource_pool_ids: vec![],
    }
}

#[test]
fn discovery_reveals_only_rendezvous_identity_and_attributable_freshness() {
    let projection = disclose_host_offer(
        &observation(),
        RemoteProofClass::TransportAttributed,
        &request(OfferDisclosureStage::Discovery),
    )
    .unwrap();

    assert_eq!(projection.host_id.as_str(), "host/browser-a");
    assert_eq!(projection.boot_id.as_str(), "boot/browser-a/1");
    assert_eq!(projection.observation_sign_id.as_str(), "sign/offer/4");
    assert_eq!(projection.freshness_sequence, 9);
    assert_eq!(projection.profile, None);
    assert!(projection.capability_summary.is_empty());
    assert!(projection.capabilities.is_empty());
    assert!(projection.resources.is_empty());
}

#[test]
fn admitted_summary_is_bounded_but_planning_discloses_only_selected_exact_facts() {
    let admitted = disclose_host_offer(
        &observation(),
        RemoteProofClass::SelfReported,
        &request(OfferDisclosureStage::AdmittedMembership),
    )
    .unwrap();
    assert_eq!(
        admitted.profile.unwrap().as_str(),
        "profile/friendly-browser"
    );
    assert_eq!(admitted.capability_summary.len(), 2);
    assert!(admitted.capabilities.is_empty());
    assert!(admitted.resources.is_empty());

    let planning = disclose_host_offer(
        &observation(),
        RemoteProofClass::SelfReported,
        &OfferDisclosureRequest {
            stage: OfferDisclosureStage::Planning,
            capability_ids: vec![CapabilityId::from("cap/audio")],
            resource_pool_ids: vec![ResourcePoolId::from("pool/audio")],
        },
    )
    .unwrap();
    assert_eq!(planning.capabilities.len(), 1);
    assert_eq!(planning.capabilities[0].capability_id.as_str(), "cap/audio");
    assert_eq!(planning.resources.len(), 1);
    assert_eq!(planning.resources[0].pool_id.as_str(), "pool/audio");
    assert!(planning
        .capabilities
        .iter()
        .all(|offer| offer.capability_id.as_str() != "cap/private-camera"));
    assert!(planning
        .resources
        .iter()
        .all(|offer| offer.pool_id.as_str() != "pool/private-camera"));
}

#[test]
fn disclosure_is_deterministic_canonical_and_refuses_early_or_unknown_detail() {
    let observation = observation();
    let early = OfferDisclosureRequest {
        stage: OfferDisclosureStage::Discovery,
        capability_ids: vec![CapabilityId::from("cap/audio")],
        resource_pool_ids: vec![],
    };
    assert_eq!(
        disclose_host_offer(&observation, RemoteProofClass::SelfReported, &early),
        Err(OfferDisclosureRefusal::DetailRequestedTooEarly)
    );
    let noncanonical = OfferDisclosureRequest {
        stage: OfferDisclosureStage::Planning,
        capability_ids: vec![
            CapabilityId::from("cap/private-camera"),
            CapabilityId::from("cap/audio"),
        ],
        resource_pool_ids: vec![],
    };
    assert_eq!(
        disclose_host_offer(&observation, RemoteProofClass::SelfReported, &noncanonical),
        Err(OfferDisclosureRefusal::NonCanonicalRequest)
    );
    let unknown = OfferDisclosureRequest {
        stage: OfferDisclosureStage::Planning,
        capability_ids: vec![CapabilityId::from("cap/not-offered")],
        resource_pool_ids: vec![],
    };
    assert_eq!(
        disclose_host_offer(&observation, RemoteProofClass::SelfReported, &unknown),
        Err(OfferDisclosureRefusal::UnknownCapability)
    );

    let a: HostOfferProjection = disclose_host_offer(
        &observation,
        RemoteProofClass::SelfReported,
        &request(OfferDisclosureStage::AdmittedMembership),
    )
    .unwrap();
    let b = disclose_host_offer(
        &observation,
        RemoteProofClass::SelfReported,
        &request(OfferDisclosureStage::AdmittedMembership),
    )
    .unwrap();
    assert_eq!(a, b);
}

fn current_membership() -> BodyMembership {
    let body = Body::born(
        SourceDocumentId::from("source/provenance"),
        CheckedFormId::from("checked/provenance"),
        1,
        SignId::from("sign/born"),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part = PartId::bind(&body.body_id, "browser-a", 0).unwrap();
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            MembershipProofId::bind("proof/admission").unwrap(),
            SignId::from("sign/admitted"),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: HostId::from("host/browser-a"),
                boot_id: BootId::from("boot/browser-a/1"),
                offer_generation: OfferGeneration(4),
                proof_id: MembershipProofId::bind("proof/continuity").unwrap(),
                sequence: 1,
            },
            SignId::from("sign/present"),
        )
        .unwrap();
    membership
}

fn provenance(class: RemoteProofClass) -> RemoteClaimProvenance {
    RemoteClaimProvenance {
        asserting_host_id: HostId::from("host/browser-a"),
        asserting_boot_id: BootId::from("boot/browser-a/1"),
        offer_generation: OfferGeneration(4),
        capability_id: Some(CapabilityId::from("cap/audio")),
        implementation_id: Some(ImplementationId::from("implementation/cap/audio")),
        base_id: None,
        resource_pool_id: Some(ResourcePoolId::from("pool/audio")),
        plan_id: None,
        active_play_id: None,
        sign_id: SignId::from("sign/measurement/1"),
        freshness_sequence: 1,
        proof_class: class,
    }
}

#[test]
fn membership_attributes_but_never_strengthens_a_self_reported_claim() {
    let membership = current_membership();
    let policy = RemoteClaimPolicy {
        accepted_sources: vec![HostId::from("host/browser-a")],
        accepted_proof_classes: vec![RemoteProofClass::PlatformObserved],
        require_current_member: true,
        minimum_independent_sources: 1,
        use_class: ClaimUseClass::Planning,
    };
    assert_eq!(
        policy.admits(
            &provenance(RemoteProofClass::SelfReported),
            Some(&membership),
            ClaimUseClass::Planning
        ),
        Err(ClaimPolicyRefusal::ProofClassNotAccepted)
    );
    assert_eq!(
        policy.admits(
            &provenance(RemoteProofClass::PlatformObserved),
            Some(&membership),
            ClaimUseClass::Planning
        ),
        Ok(())
    );
}

#[test]
fn policy_keeps_display_only_selected_source_and_current_boot_distinct() {
    let membership = current_membership();
    let display = RemoteClaimPolicy {
        accepted_sources: vec![HostId::from("host/browser-a")],
        accepted_proof_classes: vec![RemoteProofClass::SelfReported],
        require_current_member: true,
        minimum_independent_sources: 1,
        use_class: ClaimUseClass::DisplayOrDiagnostic,
    };
    assert_eq!(
        display.admits(
            &provenance(RemoteProofClass::SelfReported),
            Some(&membership),
            ClaimUseClass::Planning
        ),
        Err(ClaimPolicyRefusal::DisplayOnly)
    );

    let mut wrong_source = provenance(RemoteProofClass::SelfReported);
    wrong_source.asserting_host_id = HostId::from("host/browser-b");
    assert_eq!(
        display.admits(
            &wrong_source,
            Some(&membership),
            ClaimUseClass::DisplayOrDiagnostic
        ),
        Err(ClaimPolicyRefusal::SourceNotSelected)
    );

    let mut stale_boot = provenance(RemoteProofClass::SelfReported);
    stale_boot.asserting_boot_id = BootId::from("boot/browser-a/old");
    assert_eq!(
        display.admits(
            &stale_boot,
            Some(&membership),
            ClaimUseClass::DisplayOrDiagnostic
        ),
        Err(ClaimPolicyRefusal::NotCurrentMember)
    );
}

#[test]
fn corroboration_counts_exact_independent_hosts_only_when_policy_requires_it() {
    let policy = RemoteClaimPolicy {
        accepted_sources: vec![],
        accepted_proof_classes: vec![RemoteProofClass::PlatformObserved],
        require_current_member: false,
        minimum_independent_sources: 2,
        use_class: ClaimUseClass::Planning,
    };
    let first = provenance(RemoteProofClass::PlatformObserved);
    assert_eq!(
        policy.admits(&first, None, ClaimUseClass::Planning),
        Err(ClaimPolicyRefusal::InsufficientIndependentSources)
    );

    let duplicate = first.clone();
    assert_eq!(
        policy.admits_corroborated(&[first.clone(), duplicate], None, ClaimUseClass::Planning),
        Err(ClaimPolicyRefusal::InsufficientIndependentSources)
    );

    let mut second = first.clone();
    second.asserting_host_id = HostId::from("host/browser-b");
    second.asserting_boot_id = BootId::from("boot/browser-b/1");
    second.sign_id = SignId::from("sign/measurement/b/1");
    assert_eq!(
        policy.admits_corroborated(&[first, second], None, ClaimUseClass::Planning),
        Ok(())
    );
}
