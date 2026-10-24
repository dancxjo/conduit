//! Automatic law-carrying preparation of all four unchanged original G2 programs.
#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[allow(dead_code)] // Shared fixture's negative constructors belong to owner conformance.
#[path = "common/phone_gestures/fixture.rs"]
mod fixture;
#[allow(dead_code)]
#[path = "common/native_law_program.rs"]
mod native_law_program;
use conduit_core::*;
use conduit_plot::rust_binding::NativeRustBinding;
use conduit_plot::*;
use conduit_speech::{phone_gestures::*, semantic::*};
use conduit_std_host::*;
use std::collections::BTreeMap;
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: std::time::Duration) {
        panic!("no timer operation in G2 Source");
    }
}
#[derive(Default)]
struct Outputs(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        self.0.push(value);
        Ok(())
    }
}
#[test]
fn automatic_source_laws_replay_all_four_g2_programs_through_std_kernel() {
    let fixture = fixture::Fixture::new(false);
    let owner = fixture.owner();
    let controls = fixture::controls(&owner);
    let profile = PhoneGestureProfile::prepare(&owner, "G2/tata@1", &controls).unwrap();
    let gestures = prepare_phone_gestures(
        &owner,
        &profile,
        &fixture::anchor(),
        &fixture::provenance(),
        fixture::capacity(),
    )
    .unwrap();
    let mut startup = StartupCatalog::new();
    conduit_audio::authoring::install(&mut startup).unwrap();
    let source = format!(
        "{}\n{}",
        include_str!("../timing.conduit"),
        include_str!("../phone_gestures.conduit")
    );
    let checked = check_syntax_document(&parse_syntax_document(&source), &startup).unwrap();
    for operation in ["advance", "phases", "curve", "energy"] {
        let entry = format!("speech/phone-gesture-{operation}");
        let historical =
            expand_canonical_plot_for_authoring(&checked, &entry, &ProfileCatalog::new()).unwrap();
        let ConfigurationValue::Text(original_hex) =
            &historical.expanded.gears[0].configuration[0].value
        else {
            panic!("original program");
        };
        let original = PortableExpressionProgram::from_canonical_hex(original_hex).unwrap();
        let (document, catalog, context) = actual_law_sources(&original);
        let authored = expand_canonical_plot_for_authoring(&document, &entry, &catalog).unwrap();
        let [gear] = authored.expanded.gears.as_slice() else {
            panic!("one Source expression")
        };
        let [configuration] = gear.configuration.as_slice() else {
            panic!("one selected Source program")
        };
        let ConfigurationValue::Text(hex) = &configuration.value else {
            panic!("canonical Source program")
        };
        assert_eq!(configuration.key, "law-program");
        let material = LawExpressionMaterial::from_canonical_hex(hex).unwrap();
        assert_eq!(
            material.program, original,
            "{operation}: original checked program including proven hints"
        );
        assert_ne!(gear.kind_id, historical.expanded.gears[0].kind_id);
        let matching = gestures
            .executions()
            .iter()
            .filter(|e| e.program_hex() == original_hex)
            .collect::<Vec<_>>();
        let expected = gestures.phones().len() * if operation == "curve" { 4 } else { 1 };
        assert_eq!(
            matching.len(),
            expected,
            "{operation}: replay every exact owner request, not merely a matching shape"
        );
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: format!("G2/{operation}").into(),
                boot_id: format!("G2/{operation}/boot").into(),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::minimal(),
        );
        let before = host.advertisement().clone();
        assert!(host
            .prepare_expression_realizations(&authored.expanded)
            .is_err());
        assert_eq!(host.advertisement(), &before);
        host.prepare_expression_realizations_with_law_context(&authored.expanded, &context)
            .unwrap();
        let offer = host
            .advertisement()
            .capabilities
            .iter()
            .find(|offer| gear.accepts_realization(offer))
            .expect("automatic exact law offer");
        let selected_offer = offer.capability_id.clone();
        let hosts = [host.advertisement().clone()];
        let placements =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = [
            (PortDirection::Input, "value"),
            (PortDirection::Output, "result"),
        ]
        .into_iter()
        .map(|(direction, front)| {
            (
                conduit_planner::ForeBoundaryKey {
                    direction,
                    front_port_id: front.into(),
                    track: ConnectionTrack::Payload,
                },
                conduit_planner::ConnectionQueueLimits {
                    item_capacity: 1,
                    byte_capacity: 65536,
                },
            )
        })
        .collect();
        let plan = conduit_planner::plan_expanded_authoring_with_options(
            &authored,
            &hosts,
            &placements,
            &["conduit.base/local@1".into()],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 65536,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &boundaries,
        )
        .unwrap();
        assert!(verify_plan(&plan));
        retain_receipt(
            &format!("{operation}-plan"),
            &serde_json::to_value(&plan).unwrap(),
        );
        eprintln!(
            "G2-kernel operation={operation} requests={} program-kind={} offer={} plan={:?}",
            matching.len(),
            gear.kind_id.as_str(),
            selected_offer.as_str(),
            plan.plan_id
        );
        // Every exact retained input is replayed, including all three distinct
        // formants and energy via the one curve program. No detached caller tape.
        for (request_index, retained) in matching.into_iter().enumerate() {
            let mut output = Outputs::default();
            let report = host
                .run_external_plot_to(
                    plan.fragments[0].clone(),
                    &[ExternalForeInput {
                        front_port_id: "value".into(),
                        track: ConnectionTrack::Payload,
                        bytes: retained.input_canonical().to_vec(),
                    }],
                    &mut output,
                    &mut Vec::new(),
                    &mut NoTimer,
                )
                .unwrap();
            assert_eq!(output.0.len(), 1);
            assert_eq!(
                output.0[0].bytes,
                retained.output_canonical(),
                "{operation}: exact retained Source result"
            );
            // Validate the actual delivered output at the same Native boundary
            // used by the retained owner, independently of its byte comparison.
            match operation {
                "advance" => {
                    SpeechPhoneGestureRawTime::decode(&output.0[0].bytes).unwrap();
                }
                "phases" => {
                    SpeechPhoneGestureRawPhases::decode(&output.0[0].bytes).unwrap();
                }
                "curve" | "energy" => {
                    conduit_audio::AudioQuantityTrajectory::decode(&output.0[0].bytes).unwrap();
                }
                _ => unreachable!(),
            }
            let kernel = report.kernel.unwrap();
            let signs = kernel
                .kernel_sign
                .iter()
                .map(|event| {
                    serde_json::json!({
                        "sequence":event.sequence,"node":event.node.0,
                        "port":event.port.map(|port| port.0),
                        "request":event.request.map(|request| request.0),
                        "kind":format!("{:?}", event.kind),
                    })
                })
                .collect::<Vec<_>>();
            retain_receipt(
                &format!("{operation}-request-{request_index}"),
                &serde_json::json!({
                    "operation":operation,"request_index":request_index,
                    "source":authored.expanded.source_document_id,
                    "checked":authored.expanded.checked_plot_id,
                    "expanded":authored.expanded.expanded_plot_id,
                    "plan":plan.plan_id,"active_play":kernel.active_play_id,
                    "gear":gear.gear_id,"selected_offer":selected_offer,
                    "input_canonical":retained.input_canonical(),
                    "expected_output_canonical":retained.output_canonical(),
                    "delivered_output_canonical":output.0[0].bytes,
                    "kernel_sign":signs,
                    "value_capacity_before":kernel.value_allocation_capacity_before,
                    "value_capacity_after":kernel.value_allocation_capacity_after,
                }),
            );
            assert_eq!(
                kernel.value_allocation_capacity_before,
                kernel.value_allocation_capacity_after
            );
            assert!(kernel
                .kernel_sign
                .iter()
                .any(|s| s.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
            assert!(kernel
                .kernel_sign
                .iter()
                .any(|s| s.kind == conduit_kernel::KernelEventKind::BackCompleted));
            assert!(kernel
                .kernel_sign
                .iter()
                .all(|s| s.kind != conduit_kernel::KernelEventKind::BackFailed));
        }
    }
}

fn actual_law_sources(
    program: &PortableExpressionProgram,
) -> (CheckedSyntaxDocument, ProfileCatalog, LawExpressionContext) {
    let speech_metadata = conduit_speech::authoring::checked_types().unwrap();
    let audio_metadata = conduit_audio::authoring::checked_types().unwrap();
    let mut types = std::collections::BTreeSet::new();
    native_law_program::types(&program.input_type, &mut types);
    native_law_program::types(&program.output_type, &mut types);
    native_law_program::nodes(&program.root, &mut types);
    let mut speech_names = std::collections::BTreeSet::new();
    let mut audio_names = std::collections::BTreeSet::new();
    for ty in &types {
        if let Some(owner) = speech_metadata.iter().find(|owner| &owner.value_type == ty) {
            speech_names.insert(owner.name.as_str());
        } else if let Some(owner) = audio_metadata.iter().find(|owner| &owner.value_type == ty) {
            audio_names.insert(owner.name.as_str());
        } else {
            native_law_program::assert_builtin_structure(ty);
        }
    }
    let speech = conduit_speech::authoring::checked_document_with_law_custody_for(
        &speech_names.iter().copied().collect::<Vec<_>>(),
    )
    .unwrap();
    let audio = conduit_audio::authoring::checked_document_with_law_custody_for(
        &audio_names.iter().copied().collect::<Vec<_>>(),
    )
    .unwrap();
    let mut witnesses = Vec::new();
    for (document, names) in [(&speech, &speech_names), (&audio, &audio_names)] {
        for name in names {
            witnesses.push(document.native_law_witness(name).unwrap().clone());
        }
    }
    let context = LawExpressionContext::from_source_witnesses(&witnesses).unwrap();
    let mut catalog = ProfileCatalog::new();
    catalog.enable_checked_native_law_expressions();
    catalog
        .install_expression_law_witnesses(&witnesses)
        .unwrap();
    (speech, catalog, context)
}

fn retain_receipt(name: &str, value: &serde_json::Value) {
    if let Some(root) = std::env::var_os("CONDUIT_G2_LAW_RECEIPT_OUTPUT") {
        let root = std::path::PathBuf::from(root);
        std::fs::create_dir_all(&root).unwrap();
        std::fs::write(
            root.join(format!("{name}.json")),
            serde_json::to_vec_pretty(value).unwrap(),
        )
        .unwrap();
    }
}
