use alloc::string::ToString;
use conduit_core::{
    BodyTimeEstimate, BodyTimeQuality, BodyTimeRefusal, BootId, ClockProvenance, HostId,
    MonotonicClockIdentity, MonotonicInstant, TemporalScale,
};

use crate::{explain_clock_quality, CausalExplanationVisibility, ClockQualityExplanation};

fn estimate() -> BodyTimeEstimate {
    let clock = MonotonicClockIdentity::new(
        HostId::from("host/private"),
        BootId::from("boot/private"),
        "mono/private".into(),
        TemporalScale::Milliseconds,
        1,
        1,
    )
    .unwrap();
    BodyTimeEstimate {
        body_basis: "body/private".into(),
        generation: 12,
        earliest_ticks: 94,
        center_ticks: 100,
        latest_ticks: 106,
        scale: TemporalScale::Milliseconds,
        local_sample: MonotonicInstant::new(50, clock).unwrap(),
        correlation_age_ticks: 3,
        provenance: ClockProvenance::Peer {
            host_id: HostId::from("peer/private"),
            boot_id: BootId::from("peer-boot/private"),
            policy_id: "policy/private".into(),
            admission_reference: "BEARER-DO-NOT-EXPOSE".into(),
            membership_revision: 4,
            observation_sequence: 9,
        },
    }
}

#[test]
fn operator_retains_bounds_and_warning_without_admission_material() {
    let quality = BodyTimeQuality::Degrading {
        now: estimate(),
        reason: BodyTimeRefusal::Stale,
    };
    let explained = explain_clock_quality(&quality, CausalExplanationVisibility::Operator).unwrap();
    let ClockQualityExplanation::Degrading {
        now: Some(now),
        reason,
    } = &explained
    else {
        panic!("missing degradation")
    };
    assert_eq!(*reason, BodyTimeRefusal::Stale);
    assert_eq!(
        (now.earliest_ticks, now.center_ticks, now.latest_ticks),
        (94, 100, 106)
    );
    assert_eq!(now.local_host, "host/private");
    assert_eq!(now.local_boot, "boot/private");
    assert_eq!(now.body_basis, "body/private");
    assert_eq!(now.generation, 12);
    assert_eq!(now.correlation_age_ticks, 3);
    let json = serde_json::to_string(&explained).unwrap();
    assert!(!json.contains("BEARER-DO-NOT-EXPOSE"));
    assert!(!json.contains("admission_reference"));
    assert!(json.contains("policy/private"));
}

#[test]
fn public_quality_retains_loss_reason_and_redacts_all_identities() {
    let quality = BodyTimeQuality::Degrading {
        now: estimate(),
        reason: BodyTimeRefusal::InsufficientQuality,
    };
    let explained = explain_clock_quality(&quality, CausalExplanationVisibility::Public).unwrap();
    assert_eq!(
        explained,
        ClockQualityExplanation::Degrading {
            now: None,
            reason: BodyTimeRefusal::InsufficientQuality
        }
    );
    let json = serde_json::to_string(&explained).unwrap();
    assert!(!json.contains("private"));
    assert!(!json.contains("BEARER"));
    let unsupported = BodyTimeQuality::Unsupported {
        reason: BodyTimeRefusal::Unavailable,
    };
    for visibility in [
        CausalExplanationVisibility::Public,
        CausalExplanationVisibility::Operator,
    ] {
        assert_eq!(
            explain_clock_quality(&unsupported, visibility).unwrap(),
            ClockQualityExplanation::Unsupported {
                reason: BodyTimeRefusal::Unavailable
            }
        );
    }
}

#[test]
fn ready_projection_retains_both_estimates_only_for_operator() {
    let now = estimate();
    let mut horizon = now.clone();
    horizon.center_ticks += 10;
    horizon.earliest_ticks += 10;
    horizon.latest_ticks += 10;
    let quality = BodyTimeQuality::Ready { now, horizon };
    assert!(matches!(
        explain_clock_quality(&quality, CausalExplanationVisibility::Operator).unwrap(),
        ClockQualityExplanation::Ready {
            now: Some(_),
            horizon: Some(_)
        }
    ));
    assert_eq!(
        explain_clock_quality(&quality, CausalExplanationVisibility::Public).unwrap(),
        ClockQualityExplanation::Ready {
            now: None,
            horizon: None
        }
    );
}

#[test]
fn external_projection_excludes_admission_material() {
    let mut now = estimate();
    now.provenance = ClockProvenance::External {
        provider_id: "provider/safe".into(),
        policy_id: "policy/safe".into(),
        admission_reference: "BEARER-DO-NOT-EXPOSE".to_string(),
    };
    let quality = BodyTimeQuality::Degrading {
        now,
        reason: BodyTimeRefusal::Stale,
    };
    let json = serde_json::to_string(
        &explain_clock_quality(&quality, CausalExplanationVisibility::Operator).unwrap(),
    )
    .unwrap();
    assert!(json.contains("provider/safe"));
    assert!(!json.contains("BEARER-DO-NOT-EXPOSE"));
}

#[test]
fn invalid_quality_is_refused_before_any_inspection_projection() {
    let mut now = estimate();
    now.earliest_ticks = now.center_ticks + 1;
    let quality = BodyTimeQuality::Degrading {
        now,
        reason: BodyTimeRefusal::Stale,
    };
    for visibility in [
        CausalExplanationVisibility::Public,
        CausalExplanationVisibility::Operator,
    ] {
        assert_eq!(
            explain_clock_quality(&quality, visibility),
            Err(BodyTimeRefusal::InvalidEstimate)
        );
    }
}

#[test]
fn assessed_clock_expiry_projects_the_authoritative_refusal() {
    use conduit_core::{
        BodyClockCorrelation, BodyTimeRequirement, BodyTimeTolerance, MonotonicDuration,
    };
    let original = estimate();
    let correlation = BodyClockCorrelation::new(
        original.body_basis.clone(),
        original.scale,
        original.generation,
        original.local_sample.clone(),
        original.center_ticks,
        0,
        100,
        6,
        10,
        original.provenance,
    )
    .unwrap();
    let requirement = BodyTimeRequirement::new(
        original.body_basis,
        BodyTimeTolerance::new(20, original.scale),
        MonotonicDuration::new(1, original.scale),
    )
    .unwrap();
    let stale_sample = MonotonicInstant::new(61, original.local_sample.clock().clone()).unwrap();
    let quality = requirement.assess(&correlation, &stale_sample);
    assert_eq!(
        quality,
        BodyTimeQuality::Unsupported {
            reason: BodyTimeRefusal::Stale
        }
    );
    for visibility in [
        CausalExplanationVisibility::Public,
        CausalExplanationVisibility::Operator,
    ] {
        assert_eq!(
            explain_clock_quality(&quality, visibility).unwrap(),
            ClockQualityExplanation::Unsupported {
                reason: BodyTimeRefusal::Stale
            }
        );
    }
}
