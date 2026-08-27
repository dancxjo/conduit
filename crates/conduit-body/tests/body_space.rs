use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, BodySpace, BodySpaceRefusal,
    MembershipProofId, PartId, MAX_BODY_LINES,
};
use conduit_core::{
    process_owned_line_offer, BaseImplementationId, BootId, CheckedFormId, HostAdvertisement,
    HostId, HostProfileId, LineAvailability, LineId, OfferGeneration, SignId, SourceDocumentId,
    PROTOCOL_VERSION,
};

fn host(name: &str, boot: &str) -> HostAdvertisement {
    HostAdvertisement {
        protocol_version: PROTOCOL_VERSION,
        host_id: HostId::from(name),
        boot_id: BootId::from(boot),
        offer_generation: OfferGeneration(1),
        profile: HostProfileId::from(format!("profile/{name}")),
        resources: Vec::new(),
        capabilities: Vec::new(),
        planner_capabilities: Vec::new(),
    }
}

fn membership() -> (Body, BodyMembership, Vec<PartId>, Vec<HostAdvertisement>) {
    let body = Body::born(
        SourceDocumentId::from("source/body-space"),
        CheckedFormId::from("checked/body-space"),
        1,
        SignId::from("sign/body-space-born"),
    )
    .unwrap();
    let hosts = vec![
        host("std/main", "std-boot/1"),
        host("browser/tab", "browser-boot/1"),
        host("pico/provisioned", "pico-boot/1"),
    ];
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let mut parts = Vec::new();
    for (index, host) in hosts.iter().enumerate() {
        let part = PartId::bind(&body.body_id, host.host_id.as_str(), index as u64).unwrap();
        membership
            .admit(
                &body.body_id,
                membership.revision,
                part.clone(),
                MembershipProofId::bind(&format!("proof/{index}")).unwrap(),
                SignId::from(format!("sign/{index}/admitted")),
            )
            .unwrap();
        membership
            .observe_present(
                &body.body_id,
                membership.revision,
                &part,
                AuthenticatedHostObservation {
                    host_id: host.host_id.clone(),
                    boot_id: host.boot_id.clone(),
                    offer_generation: host.offer_generation,
                    proof_id: MembershipProofId::bind(&format!("proof/{index}/current")).unwrap(),
                    sequence: 1,
                },
                SignId::from(format!("sign/{index}/present")),
            )
            .unwrap();
        parts.push(part);
    }
    (body, membership, parts, hosts)
}

fn lines(hosts: &[HostAdvertisement]) -> Vec<conduit_core::LineOffer> {
    vec![
        process_owned_line_offer(
            "line/std-browser",
            "binding/std-browser",
            BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            "websocket/std-browser",
            &hosts[0],
            &hosts[1],
            4,
            1_024,
        ),
        process_owned_line_offer(
            "line/std-pico",
            "binding/std-pico",
            BaseImplementationId::from("conduit.base/usb-cdc-acm@1"),
            "usb-cdc/std-pico",
            &hosts[0],
            &hosts[2],
            4,
            1_024,
        ),
    ]
}

#[test]
fn three_parts_share_body_addresses_without_inventing_a_full_mesh() {
    let (body, membership, parts, hosts) = membership();
    let space = BodySpace::project(&body.body_id, &membership, &lines(&hosts)).unwrap();
    assert_eq!(space.addresses.len(), 3);
    assert_eq!(space.body_id, body.body_id);
    assert!(space.ready_line_between(&parts[0], &parts[1]).is_some());
    assert!(space.ready_line_between(&parts[0], &parts[2]).is_some());
    assert!(space.ready_line_between(&parts[1], &parts[2]).is_none());
    assert_eq!(space.planner_line_candidates().len(), 2);
}

#[test]
fn line_loss_changes_route_truth_without_deleting_membership() {
    let (body, membership, parts, hosts) = membership();
    let retained_membership = membership.clone();
    let mut offered = lines(&hosts);
    let with_both = BodySpace::project(&body.body_id, &membership, &offered).unwrap();
    assert!(with_both.ready_line_between(&parts[0], &parts[2]).is_some());

    offered[1].availability.availability = LineAvailability::Unavailable;
    let unavailable = BodySpace::project(&body.body_id, &membership, &offered).unwrap();
    assert!(unavailable
        .ready_line_between(&parts[0], &parts[2])
        .is_none());
    offered.pop();
    let partitioned = BodySpace::project(&body.body_id, &membership, &offered).unwrap();
    assert!(!partitioned.contains_line(&LineId::from("line/std-pico")));
    assert_eq!(membership, retained_membership);
    assert_eq!(partitioned.addresses.len(), 3);
}

#[test]
fn wrong_body_stale_endpoint_duplicate_and_pressure_refuse() {
    let (body, membership, _, hosts) = membership();
    let other = Body::born(
        SourceDocumentId::from("source/other-space"),
        CheckedFormId::from("checked/other-space"),
        2,
        SignId::from("sign/other-space-born"),
    )
    .unwrap();
    assert_eq!(
        BodySpace::project(&other.body_id, &membership, &[]),
        Err(BodySpaceRefusal::WrongBody)
    );

    let mut stale = lines(&hosts);
    stale[0].binding.sink.boot_id = BootId::from("browser-boot/stale");
    assert_eq!(
        BodySpace::project(&body.body_id, &membership, &stale),
        Err(BodySpaceRefusal::StaleEndpoint)
    );

    let mut duplicate = lines(&hosts);
    duplicate.push(duplicate[0].clone());
    assert_eq!(
        BodySpace::project(&body.body_id, &membership, &duplicate),
        Err(BodySpaceRefusal::DuplicateLine)
    );

    let mut pressure = Vec::new();
    for index in 0..=MAX_BODY_LINES {
        let mut line = lines(&hosts)[0].clone();
        line.line_id = LineId::from(format!("line/pressure/{index}"));
        line.availability.line_id = line.line_id.clone();
        pressure.push(line);
    }
    assert_eq!(
        BodySpace::project(&body.body_id, &membership, &pressure),
        Err(BodySpaceRefusal::LineCapacityExhausted)
    );
}
