use super::*;
use crate::{
    decode_observation_bundle, lower_charging_sources, lower_group_zero, CreatePortableObservation,
};
use conduit_core::{TemporalRelation, TemporalScale};

fn instant(ticks: u64, uncertainty_ticks: u64) -> TemporalInstant {
    TemporalInstant {
        ticks,
        scale: TemporalScale::Milliseconds,
        clock_basis: "clock/pete-create-session".into(),
        resolution_ticks: 1,
        uncertainty_ticks,
    }
}

fn reference(ticks: u64) -> TemporalReference {
    TemporalReference {
        identity: format!("reference/pete-model-turn/{ticks}"),
        instant: instant(ticks, 0),
    }
}

fn snapshot() -> CreateObservationSnapshot {
    CreateObservationSnapshot {
        host_id: HostId::from("host/pete-std"),
        boot_id: BootId::from("boot/pete-std/7"),
        offer_generation: OfferGeneration(3),
        serial_base_id: "base/create-uart".into(),
        robot_identity: "robot/pete-create1".into(),
        observation_generation: 9,
        observed_at_tick: 1_000,
        maximum_age_ticks: 50_000,
        observation: observation(),
        odometry: None,
    }
}

fn observation() -> CreatePortableObservation {
    let mut group = [0_u8; 26];
    group[16] = 3;
    group[17..19].copy_from_slice(&14_000_u16.to_be_bytes());
    group[19..21].copy_from_slice(&100_i16.to_be_bytes());
    group[22..24].copy_from_slice(&1_000_u16.to_be_bytes());
    group[24..26].copy_from_slice(&2_000_u16.to_be_bytes());
    let mut frame = vec![19, 29, 0];
    frame.extend_from_slice(&group);
    frame.extend_from_slice(&[34, 2]);
    let sum = frame.iter().fold(0_u8, |sum, byte| sum.wrapping_add(*byte));
    frame.push(0_u8.wrapping_sub(sum));
    let bundle = decode_observation_bundle(&frame).unwrap();
    CreatePortableObservation {
        group_zero: lower_group_zero(&bundle.group_zero).unwrap(),
        charging_sources: lower_charging_sources(&bundle.charging_sources).unwrap(),
    }
}

#[test]
fn pete_model_context_leads_with_age_and_keeps_exact_observation_truth() {
    let value = project_pete_create_model_context(
        &snapshot(),
        CreateObservationChannel::Battery,
        SignId::from("sign/pete/battery/9"),
        instant(1_000, 0),
        reference(19_000),
    )
    .unwrap();
    assert_eq!(value.observed.relative_time, "18 seconds ago");
    assert_eq!(value.freshness, PeteCreateObservationFreshness::Current);
    assert_eq!(value.observation.robot_identity, "robot/pete-create1");
    assert_eq!(value.observation.host_id.as_str(), "host/pete-std");
    assert_eq!(value.observation.boot_id.as_str(), "boot/pete-std/7");
    assert_eq!(value.observation.offer_generation, OfferGeneration(3));
    assert_eq!(value.observation.observation_generation, 9);
    assert_eq!(value.observation.channel, "battery");
    assert_eq!(value.observed.source, instant(1_000, 0));
    assert_eq!(value.observed.reference, reference(19_000));
    assert_eq!(
        value.observed.sign_id,
        Some(SignId::from("sign/pete/battery/9"))
    );
    value.request.validate().unwrap();
    assert_eq!(
        value.request.evidence[0].sign_id,
        SignId::from("sign/pete/battery/9")
    );
    assert!(value.request.evidence[0]
        .observation
        .starts_with("battery observed 18 seconds ago"));
    assert!(value
        .request
        .context
        .starts_with("[{\"relative_time\":\"18 seconds ago\""));

    let json = serde_json::to_string(&value).unwrap();
    assert!(json.starts_with("{\"observed\":{\"relative_time\":\"18 seconds ago\""));
    assert!(json.len() <= MAXIMUM_PETE_MODEL_CONTEXT_BYTES);
}

#[test]
fn reviewed_observation_age_boundary_is_current_expired_or_visible_as_uncertain() {
    let current = project_pete_create_model_context(
        &snapshot(),
        CreateObservationChannel::BumpAggregate,
        SignId::from("sign/pete/bump/9"),
        instant(1_000, 0),
        reference(51_000),
    )
    .unwrap();
    let expired = project_pete_create_model_context(
        &snapshot(),
        CreateObservationChannel::BumpAggregate,
        SignId::from("sign/pete/bump/9"),
        instant(1_000, 0),
        reference(51_001),
    )
    .unwrap();
    let uncertain = project_pete_create_model_context(
        &snapshot(),
        CreateObservationChannel::BumpAggregate,
        SignId::from("sign/pete/bump/9"),
        instant(1_000, 5),
        reference(51_000),
    )
    .unwrap();
    assert_eq!(current.freshness, PeteCreateObservationFreshness::Current);
    assert_eq!(expired.freshness, PeteCreateObservationFreshness::Expired);
    assert_eq!(
        uncertain.freshness,
        PeteCreateObservationFreshness::Indeterminate
    );
    assert_eq!(
        uncertain.observed.relation,
        TemporalRelation::Past {
            minimum_ticks: 49_995,
            maximum_ticks: 50_005,
        }
    );
}

#[test]
fn new_model_turn_recomputes_age_without_changing_observation_identity() {
    let first = project_pete_create_model_context(
        &snapshot(),
        CreateObservationChannel::Contact,
        SignId::from("sign/pete/contact/9"),
        instant(1_000, 0),
        reference(11_000),
    )
    .unwrap();
    let later = project_pete_create_model_context(
        &snapshot(),
        CreateObservationChannel::Contact,
        SignId::from("sign/pete/contact/9"),
        instant(1_000, 0),
        reference(81_000),
    )
    .unwrap();
    assert_eq!(first.observation, later.observation);
    assert_eq!(first.observed.source, later.observed.source);
    assert_eq!(first.observed.subject, later.observed.subject);
    assert_eq!(first.observed.relative_time, "10 seconds ago");
    assert_eq!(later.observed.relative_time, "between 1 and 2 minutes ago");
    assert_eq!(first.freshness, PeteCreateObservationFreshness::Current);
    assert_eq!(later.freshness, PeteCreateObservationFreshness::Expired);
}

#[test]
fn mismatched_future_and_invalid_observation_truth_refuse() {
    assert_eq!(
        project_pete_create_model_context(
            &snapshot(),
            CreateObservationChannel::Battery,
            SignId::from("sign/pete/battery/9"),
            instant(999, 0),
            reference(1_010),
        ),
        Err(PeteCreateModelContextRefusal::ObservationInstantMismatch)
    );
    assert_eq!(
        project_pete_create_model_context(
            &snapshot(),
            CreateObservationChannel::Battery,
            SignId::from("sign/pete/battery/9"),
            instant(1_000, 0),
            reference(999),
        ),
        Err(PeteCreateModelContextRefusal::ObservationAfterReference)
    );
    let mut invalid = snapshot();
    invalid.maximum_age_ticks = 0;
    assert_eq!(
        project_pete_create_model_context(
            &invalid,
            CreateObservationChannel::Battery,
            SignId::from("sign/pete/battery/9"),
            instant(1_000, 0),
            reference(1_010),
        ),
        Err(PeteCreateModelContextRefusal::InvalidObservation)
    );
}
