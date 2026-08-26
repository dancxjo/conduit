use conduit_body::{
    AuthenticatedHostObservation, Body, BodyMembership, CandidateInventory, HostPresenceClock,
    HostPresenceClockScale, HostPresenceTable, MembershipProofId, PartId,
};
use conduit_core::{BootId, HostId, LinkBindingId, OfferGeneration, SignId};
use conduit_presentation::{
    render_linear_presentation, PresentationError, PresentationTemporalRole, TemporalInstant,
    TemporalReference, TemporalRelation, TemporalScale,
};

use crate::{FormEditor, PartsView, PatchbayPresentation, PortableProjectionError};

fn fixture() -> (
    PatchbayPresentation,
    Body,
    conduit_body::Wake,
    BodyMembership,
    PartId,
    PartsView,
    HostPresenceTable,
) {
    let editor = FormEditor::from_source(
        "parts-temporal.conduit".into(),
        include_str!("../../../../examples/hello.conduit").into(),
    )
    .unwrap();
    let expanded = editor.expand_form("hello").unwrap();
    let body = Body::born(
        expanded.source_document_id,
        expanded.checked_form_id,
        1,
        SignId::from("sign/parts-temporal/body-born"),
    )
    .unwrap();
    let (body, wake) = body
        .wake(2, SignId::from("sign/parts-temporal/woke"))
        .unwrap();
    let projection =
        PatchbayPresentation::new(2, editor.view(), None, None, None, Vec::new()).unwrap();
    let mut membership = BodyMembership::new(body.body_id.clone()).unwrap();
    let part_id = PartId::bind(&body.body_id, "browser", 0).unwrap();
    let proof_id = MembershipProofId::bind("proof/parts-temporal/browser").unwrap();
    membership
        .admit(
            &body.body_id,
            membership.revision,
            part_id.clone(),
            proof_id.clone(),
            SignId::from("sign/parts-temporal/admitted"),
        )
        .unwrap();
    membership
        .observe_present(
            &body.body_id,
            membership.revision,
            &part_id,
            AuthenticatedHostObservation {
                host_id: HostId::from("browser/parts-temporal"),
                boot_id: BootId::from("browser-boot/parts-temporal"),
                offer_generation: OfferGeneration(1),
                proof_id,
                sequence: 1,
            },
            SignId::from("sign/parts-temporal/present"),
        )
        .unwrap();
    let mut presence = HostPresenceTable::new(
        body.body_id.clone(),
        HostPresenceClock::new(
            "clock/process-restart/parts-temporal".into(),
            HostPresenceClockScale::Milliseconds,
            1,
            2,
        )
        .unwrap(),
        30_000,
    )
    .unwrap();
    let session = LinkBindingId::from("binding/parts-temporal");
    presence
        .start(
            &membership,
            &part_id,
            session.clone(),
            1,
            1_000,
            10_000,
            SignId::from("sign/parts-temporal/started"),
        )
        .unwrap();
    presence
        .renew(
            &membership,
            &part_id,
            &session,
            2,
            2_000,
            10_000,
            SignId::from("sign/parts-temporal/renewed"),
        )
        .unwrap();
    let parts = PartsView::project_with_presence(
        &body,
        &membership,
        &CandidateInventory::new(body.body_id.clone()).unwrap(),
        &part_id,
        None,
        None,
        true,
        Some(&presence),
    )
    .unwrap();
    (projection, body, wake, membership, part_id, parts, presence)
}

fn reference(identity: &str, ticks: u64) -> TemporalReference {
    TemporalReference {
        identity: identity.into(),
        instant: TemporalInstant {
            ticks,
            scale: TemporalScale::Milliseconds,
            clock_basis: "clock/process-restart/parts-temporal".into(),
            resolution_ticks: 1,
            uncertainty_ticks: 0,
        },
    }
}

#[test]
fn explicit_reference_attaches_exact_signed_observation_and_recomputes_relation() {
    let (projection, body, wake, _, part_id, parts, _) = fixture();
    let first = projection
        .to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &parts,
            reference("reference/parts/first", 3_000),
        )
        .unwrap();
    assert_eq!(first.temporal_references.len(), 1);
    assert_eq!(first.temporal_facts.len(), 1);
    let fact = &first.temporal_facts[0];
    assert_eq!(fact.subject, format!("part/{}", part_id.as_str()));
    assert_eq!(fact.role, PresentationTemporalRole::Observation);
    assert_eq!(
        fact.sign_id,
        Some(SignId::from("sign/parts-temporal/renewed"))
    );
    assert_eq!(fact.source.ticks, 2_000);
    assert_eq!(fact.source.uncertainty_ticks, 2);
    assert_eq!(
        fact.relation,
        TemporalRelation::Past {
            minimum_ticks: 998,
            maximum_ticks: 1_002,
        }
    );

    let second = projection
        .to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &parts,
            reference("reference/parts/second", 4_000),
        )
        .unwrap();
    assert_ne!(first.identity, second.identity);
    assert_eq!(first.subjects, second.subjects);
    assert_eq!(
        first.temporal_facts[0].source,
        second.temporal_facts[0].source
    );
    assert_eq!(
        first.temporal_facts[0].sign_id,
        second.temporal_facts[0].sign_id
    );
    assert_eq!(
        second.temporal_facts[0].relation,
        TemporalRelation::Past {
            minimum_ticks: 1_998,
            maximum_ticks: 2_002,
        }
    );

    let future = projection
        .to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &parts,
            reference("reference/parts/future", 1_000),
        )
        .unwrap();
    assert_eq!(
        future.temporal_facts[0].relation,
        TemporalRelation::Future {
            minimum_ticks: 998,
            maximum_ticks: 1_002,
        }
    );
    let indeterminate = projection
        .to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &parts,
            reference("reference/parts/overlap", 2_000),
        )
        .unwrap();
    assert_eq!(
        indeterminate.temporal_facts[0].relation,
        TemporalRelation::Indeterminate
    );
    let mut exact_parts = parts.clone();
    exact_parts.parts[0]
        .details
        .presence_clock
        .as_mut()
        .unwrap()
        .uncertainty_ticks = 0;
    let present = projection
        .to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &exact_parts,
            reference("reference/parts/present", 2_000),
        )
        .unwrap();
    assert_eq!(
        present.temporal_facts[0].relation,
        TemporalRelation::Present
    );
    let linear = render_linear_presentation(&first).unwrap();
    assert!(linear.lines.iter().any(|line| {
        line.starts_with("TEMPORAL_FACT ")
            && line.contains(&format!("subject=\"part/{}\"", part_id.as_str()))
            && line.contains("role=Observation")
            && line.contains("sign=sign/parts-temporal/renewed")
    }));
}

#[test]
fn missing_presence_stays_legacy_exact_and_invalid_relations_fail_closed() {
    let (projection, body, wake, membership, part_id, parts, _) = fixture();
    let candidates = CandidateInventory::new(body.body_id.clone()).unwrap();
    let without_presence =
        PartsView::project(&body, &membership, &candidates, &part_id, None, None, true).unwrap();
    assert_eq!(
        projection
            .to_portable_front_door_with_temporal_reference(
                &body,
                &wake,
                &without_presence,
                reference("reference/unused", 3_000),
            )
            .unwrap(),
        projection
            .to_portable_front_door(&body, &wake, &without_presence)
            .unwrap()
    );

    let mut wrong_basis = reference("reference/wrong-basis", 3_000);
    wrong_basis.instant.clock_basis = "clock/unix-epoch/utc".into();
    assert_eq!(
        projection.to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &parts,
            wrong_basis,
        ),
        Err(PortableProjectionError::InvalidPresentation(
            PresentationError::IncomparableTemporalInstants,
        ))
    );

    let mut wrong_scale = reference("reference/wrong-scale", 3_000);
    wrong_scale.instant.scale = TemporalScale::Seconds;
    assert_eq!(
        projection.to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &parts,
            wrong_scale,
        ),
        Err(PortableProjectionError::InvalidPresentation(
            PresentationError::IncomparableTemporalInstants,
        ))
    );

    let mut partial = parts.clone();
    partial.parts[0].details.presence_sign_id = None;
    assert_eq!(
        projection.to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &partial,
            reference("reference/partial", 3_000),
        ),
        Err(PortableProjectionError::InvalidPresentation(
            PresentationError::InvalidTemporalInstant,
        ))
    );

    let mut overflowing = parts;
    overflowing.parts[0]
        .details
        .presence_clock
        .as_mut()
        .unwrap()
        .uncertainty_ticks = 2_001;
    assert_eq!(
        projection.to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &overflowing,
            reference("reference/overflow", 3_000),
        ),
        Err(PortableProjectionError::InvalidPresentation(
            PresentationError::TemporalIntervalOverflow,
        ))
    );
}

#[test]
fn offline_presence_uses_the_exact_terminal_observation_and_sign() {
    let (projection, body, wake, mut membership, part_id, _, mut presence) = fixture();
    presence
        .expire(
            &mut membership,
            &part_id,
            12_000,
            SignId::from("sign/parts-temporal/expired"),
        )
        .unwrap();
    let offline = PartsView::project_with_presence(
        &body,
        &membership,
        &CandidateInventory::new(body.body_id.clone()).unwrap(),
        &part_id,
        None,
        None,
        true,
        Some(&presence),
    )
    .unwrap();
    let portable = projection
        .to_portable_front_door_with_temporal_reference(
            &body,
            &wake,
            &offline,
            reference("reference/parts/offline", 13_000),
        )
        .unwrap();
    assert!(!offline.parts[0].available);
    assert_eq!(
        offline.parts[0]
            .details
            .evidence_signs
            .iter()
            .filter(|sign| sign.as_str() == "sign/parts-temporal/expired")
            .count(),
        1
    );
    assert_eq!(portable.temporal_facts[0].source.ticks, 12_000);
    assert_eq!(
        portable.temporal_facts[0].sign_id,
        Some(SignId::from("sign/parts-temporal/expired"))
    );
    assert_eq!(
        portable.temporal_facts[0].relation,
        TemporalRelation::Past {
            minimum_ticks: 998,
            maximum_ticks: 1_002,
        }
    );
    assert_eq!(
        portable
            .basis
            .sign_ids
            .iter()
            .filter(|sign| sign.as_str() == "sign/parts-temporal/expired")
            .count(),
        1
    );
}
