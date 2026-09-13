use super::*;

fn offer(features: CpuFeatures) -> HostOffer<'static> {
    HostOffer::new(
        &BootIdentities {
            host: [1; 32],
            boot: [2; 32],
        },
        "build",
        features,
        262_144,
    )
}

#[test]
fn exact_boot_offer_is_finite_and_bases_do_not_imply_authority() {
    let offer = offer(CpuFeatures {
        sse2: true,
        rdrand: false,
        invariant_tsc: true,
    });
    assert_eq!(offer.validate(), Ok(()));
    assert!(
        offer
            .bases
            .iter()
            .all(|base| { base.id != base.provider_instance_id && base.provider_generation == 1 })
    );
    assert_ne!(
        offer.bases[0].provider_instance_id,
        offer.bases[1].provider_instance_id
    );
    assert_eq!(offer.resources[0].capacity, 262_144);
    assert_eq!(offer.resources[0].base, BaseKind::Memory);
    assert_eq!(offer.capabilities[1].maximum_in_flight, 1);
    assert_eq!(
        offer.capabilities[1].host_operation,
        Some("conduit.host/present@1")
    );
    assert_eq!(
        offer.capabilities[0].output,
        Some(PortOffer {
            name: "tick",
            value_kind: "conduit.value/tick@1",
            direction: PortDirection::Output,
            closes: true,
        })
    );
    assert_eq!(offer.capabilities[0].input, None);
    assert_eq!(
        offer.capabilities[1].input.map(|port| port.direction),
        Some(PortDirection::Input)
    );
}

#[test]
fn stale_or_missing_base_provider_truth_refuses_without_changing_host_identity() {
    let features = CpuFeatures {
        sse2: true,
        rdrand: false,
        invariant_tsc: true,
    };
    let current = offer(features);
    let host = current.host_id;
    let boot = current.boot_id;
    let unaffected = current.bases[1];
    let mut stale = offer(features);
    stale.bases[0].provider_generation = 0;
    assert_eq!(stale.validate(), Err(OfferError::InvalidBaseProvider));
    assert_eq!(stale.host_id, host);
    assert_eq!(stale.boot_id, boot);
    assert_eq!(stale.bases[1], unaffected);
}

#[test]
fn one_revoked_base_refuses_only_its_exact_capability_binding() {
    let features = CpuFeatures {
        sse2: true,
        rdrand: false,
        invariant_tsc: true,
    };
    let mut offer = offer(features);
    let timer = offer.capability_provider(&offer.capabilities[0]).unwrap();
    let serial = offer.capability_provider(&offer.capabilities[1]).unwrap();
    offer
        .bases
        .iter_mut()
        .find(|base| base.kind == BaseKind::Timer)
        .unwrap()
        .lifecycle = BaseLifecycle::Revoked;
    assert_eq!(offer.validate(), Ok(()));
    assert_eq!(
        offer.require_base_provider(timer),
        Err(OfferError::BaseUnavailable)
    );
    assert_eq!(
        offer.require_base_provider(serial).unwrap().kind,
        BaseKind::Serial
    );

    let mut stale = serial;
    stale.provider_generation += 1;
    assert_eq!(
        offer.require_base_provider(stale),
        Err(OfferError::StaleBaseProvider)
    );
}

#[test]
fn isa_admission_rejects_stale_missing_and_disagreeing_facts() {
    let offer = offer(CpuFeatures {
        sse2: true,
        rdrand: false,
        invariant_tsc: true,
    });
    let scalar = ImplementationCandidate {
        id: "scalar",
        boot_id: offer.boot_id,
        offer_requirement: IsaRequirement {
            sse2: true,
            rdrand: false,
        },
        artifact_requirement: IsaRequirement {
            sse2: true,
            rdrand: false,
        },
    };
    let vector = ImplementationCandidate {
        id: "rdrand",
        boot_id: offer.boot_id,
        offer_requirement: IsaRequirement {
            sse2: true,
            rdrand: true,
        },
        artifact_requirement: IsaRequirement {
            sse2: true,
            rdrand: true,
        },
    };
    assert_eq!(
        select_equal_face(&offer, &[vector, scalar]).unwrap().id,
        "scalar"
    );
    assert_eq!(
        select_equal_face(&offer, &[vector]),
        Err(OfferError::MissingIsaFeature)
    );

    let mut stale = scalar;
    stale.boot_id = [9; 32];
    assert_eq!(
        select_equal_face(&offer, &[stale]),
        Err(OfferError::StaleObservation)
    );

    let mut disagreeing = scalar;
    disagreeing.artifact_requirement.rdrand = true;
    assert_eq!(
        select_equal_face(&offer, &[disagreeing]),
        Err(OfferError::ArtifactRequirementMismatch)
    );
}

#[test]
fn malformed_memory_and_port_facts_fail_closed() {
    let features = CpuFeatures {
        sse2: true,
        rdrand: false,
        invariant_tsc: true,
    };
    let mut missing_memory = offer(features);
    missing_memory.runtime_arena_bytes = 0;
    assert_eq!(missing_memory.validate(), Err(OfferError::InvalidCapacity));

    let mut oversized_resource = offer(features);
    oversized_resource.resources[2].capacity = 2;
    assert_eq!(
        oversized_resource.validate(),
        Err(OfferError::InvalidCapacity)
    );

    let mut wrong_direction = offer(features);
    wrong_direction.capabilities[1]
        .input
        .as_mut()
        .unwrap()
        .direction = PortDirection::Output;
    assert_eq!(wrong_direction.validate(), Err(OfferError::InvalidCapacity));
}
