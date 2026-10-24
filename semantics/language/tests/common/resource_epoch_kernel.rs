//! Actual checked full Language Source and selected installed generic Std Backs.
//! Inputs come only from the opaque, Native-admitted owners in this proof.
use conduit_core::*;
use conduit_language::analysis_epoch_owner::fallback_round::{
    FallbackObservationProvider, FallbackRoundRefusal,
};
use conduit_plot::*;
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("Language proof has no timer effect");
    }
}
#[derive(Default)]
struct Output(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Output {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        assert_eq!(value.track, ConnectionTrack::Payload);
        self.0.push(value.bytes);
        Ok(())
    }
}
struct Live {
    host: StdHost,
    plan: Plan,
    programs: Vec<String>,
    input: PortId,
}
pub struct Runtime {
    checked: CheckedSyntaxDocument,
    live: BTreeMap<String, Live>,
    pub source_id: SourceDocumentId,
    pub operations: usize,
}
impl Runtime {
    pub fn new() -> Self {
        let source = conduit_language::language_semantic_source();
        let mut startup = StartupCatalog::new();
        startup
            .insert_value_kind_alias("ResourceRef", kind_id(RESOURCE_REFERENCE_INFO_ID))
            .unwrap();
        let checked = check_syntax_document(&parse_syntax_document(source), &startup).unwrap();
        Self {
            source_id: checked.source_document_id.clone(),
            checked,
            live: BTreeMap::new(),
            operations: 0,
        }
    }
    fn prepare(&self, entry: &str) -> Live {
        let authored =
            expand_canonical_plot_for_authoring(&self.checked, entry, &ProfileCatalog::new())
                .unwrap();
        let mut advertisement = StdHost::new().advertisement().clone();
        let mut programs = Vec::new();
        for gear in &authored.expanded.gears {
            let [configuration] = gear.configuration.as_slice() else {
                panic!("exact checked expression");
            };
            let ConfigurationValue::Text(hex) = &configuration.value else {
                panic!("program");
            };
            assert_eq!(configuration.key, "program");
            let program = PortableExpressionProgram::from_canonical_hex(hex).unwrap();
            let offer =
                conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                    .unwrap();
            assert!(gear.accepts_realization(&offer));
            if !advertisement
                .capabilities
                .iter()
                .any(|p| p.capability_id == offer.capability_id)
            {
                advertisement.capabilities.push(offer);
            }
            programs.push(hex.clone());
        }
        advertisement
            .capabilities
            .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
        let host = StdHost::from_advertisement(advertisement).unwrap();
        let hosts = [host.advertisement().clone()];
        let choices =
            conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
        let boundaries = authored
            .input_bindings
            .iter()
            .map(|b| (PortDirection::Input, b.front_port_id.clone()))
            .chain(
                authored
                    .output_bindings
                    .iter()
                    .map(|b| (PortDirection::Output, b.front_port_id.clone())),
            )
            .map(|(direction, front_port_id)| {
                (
                    conduit_planner::ForeBoundaryKey {
                        direction,
                        front_port_id,
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
            &choices,
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
        assert_eq!(plan.source_document_id, self.source_id);
        Live {
            host,
            plan,
            programs,
            input: authored.input_bindings[0].front_port_id.clone(),
        }
    }
    pub fn execute(&mut self, entry: &str, input: &[u8]) -> Vec<u8> {
        self.execute_exact(entry, input, &[])
    }
    pub fn execute_exact(&mut self, entry: &str, input: &[u8], expected: &[&str]) -> Vec<u8> {
        assert!(input.len() <= 65536);
        if !self.live.contains_key(entry) {
            let live = self.prepare(entry);
            self.live.insert(entry.into(), live);
        }
        let live = self.live.get_mut(entry).unwrap();
        if !expected.is_empty() {
            assert_eq!(
                live.programs.iter().map(String::as_str).collect::<Vec<_>>(),
                expected
            );
        }
        let mut output = Output::default();
        let report = live
            .host
            .run_external_plot_to(
                live.plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: live.input.clone(),
                    track: ConnectionTrack::Payload,
                    bytes: input.to_vec(),
                }],
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(output.0.len(), 1);
        let kernel = report.kernel.unwrap();
        assert_eq!(kernel.active_play.plan_id, live.plan.plan_id);
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
        self.operations += 1;
        println!(
            "resource-epoch Source={} entry={entry} Plan={} Play={} input={:?} output={:?}",
            self.source_id.as_str(),
            live.plan.plan_id.as_str(),
            kernel.active_play_id.as_str(),
            semantic_digest("proof/resource-epoch-input@1", input),
            semantic_digest("proof/resource-epoch-output@1", &output.0[0])
        );
        output.0.pop().unwrap()
    }
}
impl FallbackObservationProvider for Runtime {
    fn observe(&mut self, operation: &str, input: &[u8]) -> Result<Vec<u8>, FallbackRoundRefusal> {
        Ok(self.execute(operation, input))
    }
}
