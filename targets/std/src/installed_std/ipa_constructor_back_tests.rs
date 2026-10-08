use super::*;
use conduit_core::{
    BootId, ConfigurationEntry, ConfigurationValue, GearId, HostId, OfferGeneration, PlacementId,
    StructuredConfigurationValue,
};
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId, ValueRef,
};
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_speech::semantic::*;
fn placement() -> PlannedGear {
    let offer = ipa_constructors::offer(IpaConstructor::Phone);
    let request = SpeechIpaUniversalRequest::new(
        "tʰ".into(),
        SpeechEvidenceProvenance::new(
            "installed fixture".into(),
            SpeechEvidenceSource::Manual,
            None,
        )
        .unwrap(),
    )
    .unwrap();
    conduit_core::planned_gear_from_parts! {
        semantic_contract: ipa_constructors::contract(IpaConstructor::Phone).semantic_contract(),
        placement_id: PlacementId::from("ipa-placement"), gear_id: GearId::from("ipa"),
        kind_id: offer.kind_id, kind_contract_revision: offer.kind_contract_revision,
        execution_profile_id: offer.implementation.execution_profile_id,
        configuration: vec![ConfigurationEntry { key: "request".into(), value: ConfigurationValue::Structured(StructuredConfigurationValue::new(IpaConstructor::Phone.request_type().profile().unwrap().value_kind().clone(), request.encode().unwrap()).unwrap()) }],
        host_id: HostId::from("ipa-host"), boot_id: BootId::from("ipa-boot"), offer_generation: OfferGeneration(1),
        capability_id: offer.capability_id, implementation_id: offer.implementation.implementation_id, artifact_id: offer.implementation.artifact_id,
        base: None, realization_characteristics: Vec::new(), limits: offer.limits,
        inputs: offer.inputs, outputs: offer.outputs, terminal_transductions: Vec::new(), host_calls: offer.host_calls,
        resources: Vec::new(), authority: Vec::new(), pool_references: Vec::new(),
    }
}
#[test]
fn installed_preparation_refuses_foreign_artifact_front_contract_and_arguments() {
    let correct = placement();
    assert!(admitted(&correct).is_ok());
    let mut artifact = correct.clone();
    artifact.artifact_id = "foreign/artifact".into();
    let mut front = correct.clone();
    front.outputs[0].value_kind = "value/text".into();
    let mut contract = correct.clone();
    contract.semantic_contract.laws.clear();
    let mut extra = correct.clone();
    extra.configuration.push(extra.configuration[0].clone());
    let mut wrong_type = correct.clone();
    let output = admitted(&correct).unwrap();
    wrong_type.configuration[0].value = ConfigurationValue::Structured(
        StructuredConfigurationValue::new(output.value_kind().clone(), output.bytes().to_vec())
            .unwrap(),
    );
    let mut revision = correct.clone();
    revision.kind_contract_revision = "foreign/revision".into();
    let mut profile = correct.clone();
    profile.execution_profile_id = "foreign/profile".into();
    let mut limits = correct.clone();
    limits.limits.max_queue_items += 1;
    for invalid in [
        artifact, front, contract, extra, wrong_type, revision, profile, limits,
    ] {
        assert!(admitted(&invalid).is_err());
    }
}
#[test]
fn prepared_value_survives_pressure_and_is_emitted_only_once() {
    let prepared = admitted(&placement()).unwrap();
    let value = ValueRef {
        slot: 1,
        generation: 1,
        byte_len: prepared.bytes().len() as u32,
    };
    let mut back = StructuredLiteralBack::prepared(value);
    let input = StepInputBytes::test_frame([None; 1], None);
    let mut blocked = StepIo::test_frame([None; 1], [false; 1], [None; 1], None, 8);
    assert_eq!(back.step(&mut blocked, &input), StepOutcome::Await);
    assert!(blocked.test_output(PortId(0)).is_none());
    let mut ready = StepIo::test_frame([None; 1], [false; 1], [Some(value.byte_len)], None, 8);
    assert_eq!(back.step(&mut ready, &input), StepOutcome::Progress);
    assert_eq!(ready.test_output(PortId(0)), Some(value));
    let mut next = StepIo::test_frame([None; 1], [false; 1], [Some(value.byte_len)], None, 8);
    assert_eq!(back.step(&mut next, &input), StepOutcome::Complete);
    assert!(next.test_output(PortId(0)).is_none());
}
