//! Exact Speech selection Source executes through installed generic StdHost.
//! Paired Speech suite separately proves current authority and compact PCM.
#[allow(dead_code)]
#[path = "../../../semantics/speech/tests/common/record_pronunciation/inventory.rs"]
mod inventory;
#[allow(dead_code)]
#[path = "../../../semantics/speech/tests/common/record_pronunciation/lexical.rs"]
mod lexical;
use conduit_core::*;
use conduit_language::{
    pronunciation_selection::prepare_pronunciation_selection, LanguageUniversalDependencyRelation,
};
use conduit_plot::*;
use conduit_speech::{
    ipa_inventory::PreparedIpaInventory, record_pronunciation::PreparedRecordPronunciationProfile,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::collections::BTreeMap;
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("control has no timer effect")
    }
}
#[derive(Default)]
struct Outputs(Vec<conduit_std_host::ExternalForeBytes>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}

#[test]
fn record_selection_source_executes_installed_plan_play_with_exact_receipt_output() {
    let profile = PreparedRecordPronunciationProfile::new().unwrap();
    let checked = profile.checked_source();
    assert_eq!(checked.source_document_id.as_str(), profile.source_id());
    let owned = inventory::reviewed();
    let inventory = PreparedIpaInventory::prepare(
        &owned.inventory,
        &owned.profile,
        owned.profile.variety(),
        owned.profile.revision(),
        &owned.phones,
        &owned.phonemes,
    )
    .unwrap();
    let mut actual_outputs = Vec::new();
    for (sentence, relation) in [
        ("a record", LanguageUniversalDependencyRelation::Det),
        ("we record", LanguageUniversalDependencyRelation::Nsubj),
    ] {
        let tape = lexical::lexical(sentence);
        let selection = prepare_pronunciation_selection(
            &tape,
            1,
            &lexical::analysis(),
            &lexical::arc(&tape, relation),
            &lexical::profile(),
        )
        .unwrap();
        let supplied = profile
            .project_supplied_selection(&selection, &inventory)
            .unwrap();
        let bytes = supplied.input_canonical().to_vec();
        let expected = supplied.output_canonical().to_vec();
        let capacity = 65536;
        assert!(
            bytes.len().max(expected.len()) <= capacity as usize,
            "finite single-value preparation profile"
        );
        let authored = expand_canonical_plot_for_authoring(
            checked,
            "speech/record-pronunciation",
            &ProfileCatalog::new(),
        )
        .unwrap();
        let mut advertisement = StdHost::new().advertisement().clone();
        for gear in &authored.expanded.gears {
            let [entry] = gear.configuration.as_slice() else {
                panic!("one checked expression configuration")
            };
            let ConfigurationValue::Text(encoded) = &entry.value else {
                panic!("authored control program")
            };
            assert_eq!(encoded, profile.program_hex());
            let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
            advertisement.capabilities.push(
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .unwrap(),
            );
        }
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let mut host = StdHost::from_advertisement(advertisement).unwrap();
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = [
            (PortDirection::Input, "value"),
            (PortDirection::Output, "result"),
        ]
        .into_iter()
        .map(|(direction, port)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: port.into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: capacity,
                },
            )
        })
        .collect();
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &authored,
            &hosts,
            &placements,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: capacity,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        assert_eq!(plan.source_document_id, checked.source_document_id);
        assert!(verify_plan(&plan));
        assert!(!plan.fragments.is_empty());
        let mut outputs = Outputs::default();
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: bytes.clone(),
                }],
                &mut outputs,
                &mut vec![],
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(outputs.0.len(), 1);
        assert_eq!(&outputs.0[0][..], expected.as_slice());
        let kernel = report.kernel.unwrap();
        println!(
            "record-source={:?} plan={:?} play={:?}",
            plan.source_document_id, plan.plan_id, kernel.active_play_id
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        actual_outputs.push(outputs.0.pop().unwrap());
    }
    assert_ne!(actual_outputs[0], actual_outputs[1]);
}
