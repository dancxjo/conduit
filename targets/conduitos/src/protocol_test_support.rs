//! Shared checked clock type for device-protocol fixtures; no offer or possession.
pub(crate) fn install_clock_result(startup: &mut conduit_plot::StartupCatalog) {
    let checked = conduit_plot::check_syntax_document(
        &conduit_plot::parse_syntax_document(crate::monotonic_clock::contract::CLOCK_TYPES),
        &conduit_plot::StartupCatalog::new(),
    )
    .unwrap();
    startup
        .insert_checked_native_type(
            "machine/clock/at/result",
            checked
                .native_types
                .iter()
                .find(|ty| ty.name == "MonotonicClockResult")
                .unwrap(),
        )
        .unwrap();
}

/// Fixture authentication and biography only; never native admission evidence.
pub(crate) fn protocol_body_session(
    partition: conduit_body::BodyPlotPlan,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
) -> conduit_body::BodyLifecycleSession {
    use conduit_body::*;
    let resident = &partition.plot;
    let body = Body::born(
        resident.source_document_id.clone(),
        resident.checked_plot_id.clone(),
        1,
        "fixture/born".into(),
    )
    .unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut evidence =
        BodyBiographyEvidence::born(body.clone(), membership.clone(), "Fixture".into()).unwrap();
    let part = PartId::bind(&body.body_id, host.as_str(), 1).unwrap();
    let proof = MembershipProofId::bind("fixture/local-birth").unwrap();
    let admitted = membership
        .admit(
            &body.body_id,
            membership.revision,
            part.clone(),
            proof.clone(),
            "fixture/admitted".into(),
        )
        .unwrap();
    let present = membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part,
            AuthenticatedHostObservation {
                host_id: host.clone(),
                boot_id: boot.clone(),
                offer_generation: conduit_core::OfferGeneration(1),
                proof_id: proof,
                sequence: 1,
            },
            "fixture/present".into(),
        )
        .unwrap();
    evidence
        .append_membership_events(membership, &[(admitted, 2), (present, 3)])
        .unwrap();
    let mut session = BodyLifecycleSession::open_admitted(evidence, host, boot).unwrap();
    session.propose(alloc::vec![partition], host, boot).unwrap();
    session
}
