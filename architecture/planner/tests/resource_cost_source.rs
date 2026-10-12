//! A sampled cost cannot outlive the exact advertised subjects it measured.
mod common;
use conduit_core::*;
use conduit_planner::resource_cost::*;

fn window(host: &HostAdvertisement) -> CostObservationWindow {
    let clock = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "cost-clock".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    CostObservationWindow::admit(
        MonotonicInstant::new(100, clock.clone()).unwrap(),
        MonotonicInstant::new(110, clock).unwrap(),
    )
    .unwrap()
}
#[test]
fn source_fences_full_offer_resource_boot_generation_and_calibration_snapshots() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let offer = &host.capabilities[0];
    let resource = host
        .resources
        .iter()
        .find(|pool| {
            offer
                .resource_requirements
                .iter()
                .any(|need| need.class_id == pool.class_id)
        })
        .unwrap();
    let profile = ArtifactId::from("profile/cost@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let sign = SignId::from("sign/cost/sample");
    let source =
        CostEvidenceSource::bind(&host, offer, Some(resource), &sign, revision, window(&host))
            .unwrap();
    let now = MonotonicInstant::new(105, source.window().observed().clock().clone()).unwrap();
    assert_eq!(source.validate_at(&host, revision, &now), Ok(()));
    let gear = common::pulse_gear();
    assert_eq!(
        CostWorkClass::from_checked_gear(&gear).validate_source(&source),
        Ok(())
    );
    let mut unrelated = gear.clone();
    unrelated.kind_id = "work/foreign".into();
    assert_eq!(
        CostWorkClass::from_checked_gear(&unrelated).validate_source(&source),
        Err(CostEvidenceRefusal::IncompatibleRealization)
    );
    assert_eq!(source.sign(), &sign);
    assert_eq!(source.offer(), offer);
    assert_eq!(source.resource(), Some(resource));
    let realization_sample =
        CostEvidenceSource::bind(&host, offer, None, &sign, revision, window(&host)).unwrap();
    let mut changed_resource = host.clone();
    changed_resource
        .resources
        .iter_mut()
        .find(|pool| pool.pool_id == resource.pool_id)
        .unwrap()
        .capacity_units += 1;
    assert_eq!(
        realization_sample.validate_at(&changed_resource, revision, &now),
        Err(CostEvidenceRefusal::Scope(CostScopeChange::Resource))
    );
    for (mutation, expected) in [
        (0, CostScopeChange::Host),
        (1, CostScopeChange::Boot),
        (2, CostScopeChange::OfferGeneration),
        (3, CostScopeChange::Offer),
        (4, CostScopeChange::Offer),
        (5, CostScopeChange::Offer),
        (6, CostScopeChange::Resource),
        (7, CostScopeChange::Offer),
    ] {
        let mut current = host.clone();
        match mutation {
            0 => current.host_id = "host/foreign".into(),
            1 => current.boot_id = "boot/new".into(),
            2 => current.offer_generation.0 += 1,
            3 => current.capabilities[0].implementation.artifact_id = "artifact/changed".into(),
            4 => current.capabilities[0].kind_contract_revision = "kind/changed".into(),
            5 => current.capabilities[0].limits.max_queue_bytes += 1,
            6 => {
                current
                    .resources
                    .iter_mut()
                    .find(|p| p.pool_id == resource.pool_id)
                    .unwrap()
                    .capacity_units += 1
            }
            _ => current.capabilities.push(offer.clone()),
        }
        assert_eq!(
            source.validate_at(&current, revision, &now),
            Err(CostEvidenceRefusal::Scope(expected))
        );
    }
    assert_eq!(
        source.validate_at(
            &host,
            CostCalibrationRevision::new(&profile, 2).unwrap(),
            &now
        ),
        Err(CostEvidenceRefusal::Scope(CostScopeChange::Calibration))
    );
    let other_profile = ArtifactId::from("profile/other@1");
    assert_eq!(
        source.validate_at(
            &host,
            CostCalibrationRevision::new(&other_profile, 1).unwrap(),
            &now
        ),
        Err(CostEvidenceRefusal::Scope(CostScopeChange::Calibration))
    );
    let expired = MonotonicInstant::new(110, now.clock().clone()).unwrap();
    assert_eq!(
        source.validate_at(&host, revision, &expired),
        Err(CostEvidenceRefusal::Stale)
    );
}
#[test]
fn binding_refuses_unadvertised_ambiguous_and_unattributed_evidence() {
    let mut host = common::competing_hosts().into_iter().next().unwrap();
    let profile = ArtifactId::from("profile/cost@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let sign = SignId::from("sign/cost/sample");
    let empty = SignId::from("");
    let offer = &host.capabilities[0];
    assert!(matches!(
        CostEvidenceSource::bind(&host, offer, None, &empty, revision, window(&host)),
        Err(CostEvidenceRefusal::InvalidIdentity)
    ));
    let mut foreign = offer.clone();
    foreign.capability_id = "capability/foreign".into();
    assert!(matches!(
        CostEvidenceSource::bind(&host, &foreign, None, &sign, revision, window(&host)),
        Err(CostEvidenceRefusal::UnadvertisedSubject)
    ));
    host.resources
        .push(resource_offer("pool/unrelated", "resource/unrelated", 1));
    assert!(matches!(
        CostEvidenceSource::bind(
            &host,
            &host.capabilities[0],
            host.resources.last(),
            &sign,
            revision,
            window(&host)
        ),
        Err(CostEvidenceRefusal::UnadvertisedSubject)
    ));
    host.capabilities.push(host.capabilities[0].clone());
    assert!(matches!(
        CostEvidenceSource::bind(
            &host,
            &host.capabilities[0],
            None,
            &sign,
            revision,
            window(&host)
        ),
        Err(CostEvidenceRefusal::UnadvertisedSubject)
    ));
    assert_eq!(
        CostCalibrationRevision::new(&profile, 0),
        Err(CostEvidenceRefusal::InvalidIdentity)
    );
}

#[test]
fn borrowing_the_original_snapshot_does_not_admit_future_or_foreign_clock_samples() {
    let host = common::competing_hosts().into_iter().next().unwrap();
    let profile = ArtifactId::from("profile/cost@1");
    let revision = CostCalibrationRevision::new(&profile, 1).unwrap();
    let sign = SignId::from("sign/cost/sample");
    let source = CostEvidenceSource::bind(
        &host,
        &host.capabilities[0],
        None,
        &sign,
        revision,
        window(&host),
    )
    .unwrap();
    let before = MonotonicInstant::new(99, source.window().observed().clock().clone()).unwrap();
    assert_eq!(
        source.validate_at(&host, revision, &before),
        Err(CostEvidenceRefusal::FutureObservation)
    );
    let foreign = MonotonicClockIdentity::new(
        host.host_id.clone(),
        host.boot_id.clone(),
        "different-basis".into(),
        TemporalScale::Milliseconds,
        1,
        0,
    )
    .unwrap();
    assert!(source
        .validate_at(
            &host,
            revision,
            &MonotonicInstant::new(105, foreign).unwrap()
        )
        .is_err());
    let copied = host.clone();
    let now = MonotonicInstant::new(105, source.window().observed().clock().clone()).unwrap();
    assert_eq!(source.validate_at(&copied, revision, &now), Ok(()));
}
