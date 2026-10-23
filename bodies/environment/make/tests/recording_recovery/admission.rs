//! Rejected preparation cannot consume an unrelated retained software owner.
use super::*;

#[test]
fn full_first_class_recording_fits_the_snapshot_text_profile() {
    let contract = ObservationContract::prepare().unwrap();
    let mut reference = reference();
    reference.extent.bytes =
        PreparedRecordingSnapshot::content_bytes(context(), &contract, 64).unwrap() as u64;
    let mut snapshot =
        PreparedRecordingSnapshot::prepare(context(), &contract, 64, &reference).unwrap();
    let mut forms =
        conduit_environment_runtime::observation::PreparedObservationQuantityForms::prepare(
            &contract,
        )
        .unwrap();
    let observation = forms.unpack(&[198, 0, 78, 2]).unwrap();
    for sequence in 1..=64 {
        snapshot
            .append(
                sequence,
                sequence * 60_000,
                RecordingDisposition::Available,
                Some(observation),
            )
            .unwrap();
    }
    let content = snapshot.seal().unwrap();
    assert_eq!(content.len() as u64, reference.extent.bytes);
    assert!(content.len() > 2688 && content.len() <= 4096);
    let (result, memory) =
        allocation_probe::observe(|| conduit_web::validate_json_text(content.as_bytes()));
    assert_eq!(result, Ok(()));
    assert_eq!((memory.allocations, memory.reallocations), (0, 0));
}

#[test]
fn stale_tables_and_unavailable_software_refuse_before_other_transfers() {
    let source = RecordingRecoverySource::compile(&reference()).unwrap();
    let (mut host, grant) = fixture_store(source.reference());
    let (mut generations, mut recovery, mut singleton) = software(&source, &mut host);
    let prepared = PreparedRecordingRecovery::prepare(&source, &host, &bases(), &[grant]).unwrap();
    let fragment = &prepared.plan().fragments[0];
    let placement = |kind: &str| {
        &fragment
            .placements
            .iter()
            .find(|p| p.kind_id.as_str() == kind)
            .unwrap()
            .placement_id
    };
    let generation = placement("environment/recording-generation");
    let recovery_id = placement("environment/recover-recording");
    let bridge = placement("flow/singleton");
    let mut stale = prepared.lowered().clone();
    stale.cords[0].spec.byte_capacity += 1;
    assert!(matches!(
        PreparedRecordingKernel::prepare(
            fragment,
            &stale,
            &mut generations,
            &mut singleton,
            &mut recovery
        ),
        Err(RecordingKernelRefusal::Lowering)
    ));
    generations.check_selected(fragment, generation).unwrap();
    singleton.check_selected(fragment, bridge).unwrap();
    recovery.check_selected(fragment, recovery_id).unwrap();
    // A previously transferred bridge is no longer availability. Preflight
    // catches this before consuming the still-retained source or recovery.
    let _transferred = singleton.take_selected(fragment, bridge).unwrap();
    assert!(matches!(
        PreparedRecordingKernel::prepare(
            fragment,
            prepared.lowered(),
            &mut generations,
            &mut singleton,
            &mut recovery
        ),
        Err(RecordingKernelRefusal::Software(
            RecoveryInstallationRefusal::AlreadySelected
        ))
    ));
    generations.check_selected(fragment, generation).unwrap();
    recovery.check_selected(fragment, recovery_id).unwrap();
}
