//! Root supplies external store fixtures. Production software preparation must
//! neither invent a read choice nor partially replace the Root advertisement.
use super::*;
use conduit_environment_make::{
    ReadyRecordingBatchSoftware, RecordingSoftwarePreparationRefusal as E,
};

#[test]
fn missing_foreign_or_unoffered_read_choices_leave_the_host_unchanged() {
    let mut first = reference();
    first.lifetime.version = c::ResourceVersionIdentity::from_digest([1; 32]);
    let fixture = batch_plan::fixture(&[first]);
    let mut host = fixture.host.clone();
    host.capabilities
        .retain(|offer| offer.kind_id.as_str() == "resource/read-json-snapshot");
    let original = host.clone();
    let read = fixture
        .source
        .expanded()
        .gears
        .iter()
        .find(|gear| gear.kind_id.as_str() == "resource/read-json-snapshot")
        .unwrap();
    let valid = conduit_planner::PlacementChoices {
        by_gear: [(
            read.gear_id.clone(),
            fixture.choices.by_gear[&read.gear_id].clone(),
        )]
        .into(),
    };
    let mut choices = valid.clone();
    choices.by_gear.clear();
    let contract = ObservationContract::prepare().unwrap();
    assert!(matches!(
        ReadyRecordingBatchSoftware::prepare(
            &fixture.source,
            &mut host,
            &choices,
            context(),
            &contract
        ),
        Err(E::ReadSelection)
    ));
    assert_eq!(host, original);
    choices = valid.clone();
    choices.by_gear.get_mut(&read.gear_id).unwrap().host_id = "fixture/foreign".into();
    assert!(matches!(
        ReadyRecordingBatchSoftware::prepare(
            &fixture.source,
            &mut host,
            &choices,
            context(),
            &contract
        ),
        Err(E::ReadSelection)
    ));
    assert_eq!(host, original);
    choices = valid;
    choices
        .by_gear
        .get_mut(&read.gear_id)
        .unwrap()
        .capability_id = "fixture/missing".into();
    assert!(matches!(
        ReadyRecordingBatchSoftware::prepare(
            &fixture.source,
            &mut host,
            &choices,
            context(),
            &contract
        ),
        Err(E::ReadSelection)
    ));
    assert_eq!(host, original);
}
#[test]
fn late_publication_collision_drops_new_owners_without_changing_supplied_offers() {
    let mut first = reference();
    first.lifetime.version = c::ResourceVersionIdentity::from_digest([1; 32]);
    let mut second = first.clone();
    second.lifetime.version = c::ResourceVersionIdentity::from_digest([2; 32]);
    let fixture = batch_plan::fixture(&[first, second]);
    let mut host = fixture.host.clone();
    // Recovery is published after all generation/Flow owners are initialized.
    host.capabilities.retain(|offer| {
        matches!(
            offer.kind_id.as_str(),
            "resource/read-json-snapshot" | "environment/recover-recording"
        )
    });
    let original = host.clone();
    let reads = conduit_planner::PlacementChoices {
        by_gear: fixture
            .source
            .expanded()
            .gears
            .iter()
            .filter(|gear| gear.kind_id.as_str() == "resource/read-json-snapshot")
            .map(|gear| {
                (
                    gear.gear_id.clone(),
                    fixture.choices.by_gear[&gear.gear_id].clone(),
                )
            })
            .collect(),
    };
    assert!(matches!(
        ReadyRecordingBatchSoftware::prepare(
            &fixture.source,
            &mut host,
            &reads,
            context(),
            &ObservationContract::prepare().unwrap()
        ),
        Err(E::Software(RecoveryInstallationRefusal::Advertisement))
    ));
    assert_eq!(host, original);
}
