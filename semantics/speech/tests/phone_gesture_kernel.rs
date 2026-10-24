//! Replay actual Native-admitted G2 owner requests through explicit ordinary Std offers.
//! This approved caller-owned profile does not admit arbitrary untrusted Source frames.
#![cfg(all(feature = "semantic-bindings", feature = "kernel"))]
#[allow(dead_code)] // Shared fixture's negative constructors belong to owner conformance.
#[path = "common/phone_gestures/fixture.rs"]
mod fixture;
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
fn actual_owner_receipts_replay_all_four_gesture_source_programs_through_std_kernel() {
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
        let authored =
            expand_canonical_plot_for_authoring(&checked, &entry, &ProfileCatalog::new()).unwrap();
        let [gear] = authored.expanded.gears.as_slice() else {
            panic!("one Source expression")
        };
        let [configuration] = gear.configuration.as_slice() else {
            panic!("one selected Source program")
        };
        let ConfigurationValue::Text(hex) = &configuration.value else {
            panic!("canonical Source program")
        };
        let matching = gestures
            .executions()
            .iter()
            .filter(|e| e.program_hex() == hex)
            .collect::<Vec<_>>();
        assert!(
            !matching.is_empty(),
            "{operation}: exact owner program identity must match, not merely shape"
        );
        let mut host = StdHost::new_with_composition(
            StdHostConfig {
                host_id: format!("G2/{operation}").into(),
                boot_id: format!("G2/{operation}/boot").into(),
                offer_generation: OfferGeneration(1),
            },
            StdHostComposition::minimal(),
        );
        // Automatic arbitrary-Source admission retains its independent guard.
        // The approved G2 profile instead carries the opaque owner's generated
        // Native input admission and selects the existing explicit Host offer.
        if operation == "phases" {
            let before = host.advertisement().clone();
            assert!(host
                .prepare_expression_realizations(&authored.expanded)
                .unwrap_err()
                .contains("arithmetic-witness-unavailable"));
            assert_eq!(host.advertisement(), &before);
        }
        let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        let mut advertisement = host.advertisement().clone();
        advertisement.capabilities.push(offer);
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        host = StdHost::from_advertisement(advertisement).unwrap();
        let before = host.advertisement().clone();
        // The public preparation API deliberately preserves matching explicit
        // offers. This does not confer arbitrary Native-law validation on them.
        host.prepare_expression_realizations(&authored.expanded)
            .unwrap();
        assert_eq!(host.advertisement(), &before);
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
        // Every exact retained input is replayed, including all three distinct
        // formants and energy via the one curve program. No detached caller tape.
        for retained in matching {
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
