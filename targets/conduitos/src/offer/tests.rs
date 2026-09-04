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
