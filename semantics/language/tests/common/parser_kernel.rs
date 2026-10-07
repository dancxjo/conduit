//! Existing generic expression Backs executing the checked source graph.
#![allow(dead_code)]

use conduit_composite::*;
use conduit_core::*;
use conduit_kernel::{HostCallDisposition, HostCallOutcome, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduitos::{
    expression_host_call::{ExpressionHostCall, ExpressionOperationFactory},
    protocol_source::{PreparedProtocolEntry, ProtocolSourcePackage},
    structured_selector_host_call::{SelectorHostCall, SelectorOperationFactory},
};
use std::collections::BTreeMap;

enum Owner {
    Expression(ExpressionHostCall),
    Selector(SelectorHostCall),
}
pub struct Execution {
    pub kernel: KernelCompositeHost,
    owners: Vec<(NodeId, Owner)>,
    pub preparation_nanoseconds: [u128; 4],
}
pub struct Blueprint {
    entry: PreparedProtocolEntry,
    host: HostAdvertisement,
}
impl Blueprint {
    pub fn prepare(source: String, entry_name: &str) -> Self {
        let source_bytes = source.len();
        let package = ProtocolSourcePackage::compile(source, &[]).unwrap_or_else(|error| {
            panic!("{entry_name} ({source_bytes} Source bytes): {error:?}")
        });
        let entry =
            PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), entry_name)
                .unwrap_or_else(|error| panic!("{entry_name}: {error:?}"));
        let mut host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: "fixture/language-parser".into(),
            boot_id: "fixture/language-parser-boot".into(),
            offer_generation: OfferGeneration(1),
            profile: "fixture/pure-language-parser".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        entry.publish_pure_backs(&mut host).unwrap();
        Self { entry, host }
    }
    /// Each bounded corpus fixture has a fresh exact Boot/Plan identity.
    pub fn realize(&self, epoch: usize) -> Execution {
        let preparation_started = std::time::Instant::now();
        let entry = &self.entry;
        let mut host = self.host.clone();
        host.boot_id = BootId::from(format!("fixture/language-parser-boot/{epoch}"));
        let hosts = [host];
        let choices = entry.placements(&hosts).unwrap();
        let mut limits = entry.queue_limits(&hosts, &choices).unwrap();
        // This hosted proof reserves the selected exact schema envelope. The
        // ConduitOS packaged protocol convenience entrance caps Fore at 4096B.
        for (key, limit) in &mut limits.boundaries {
            let bindings = if key.direction == PortDirection::Input {
                &entry.expanded().input_bindings
            } else {
                &entry.expanded().output_bindings
            };
            let binding = bindings
                .iter()
                .find(|b| b.front_port_id == key.front_port_id && b.track == key.track)
                .unwrap();
            let choice = &choices.by_gear[&binding.gear_id];
            limit.byte_capacity = hosts[0]
                .capabilities
                .iter()
                .find(|offer| offer.capability_id == choice.capability_id)
                .unwrap()
                .limits
                .max_queue_bytes;
        }
        let plan = conduit_planner::plan_expanded_authoring_with_connection_limits(
            entry.expanded(),
            &hosts,
            &choices,
            &[BaseImplementationId::from("conduit.base/local@1")],
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: 4096,
                authority_grants: &[],
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &limits.connections,
            &limits.boundaries,
        )
        .unwrap();
        let artifact = conduitos::protocol_artifact::AdmittedProtocolArtifact::admit(
            conduitos::protocol_artifact::ProtocolArtifactIdentity {
                source: plan.source_document_id.clone(),
                checked: plan.checked_plot_id.clone(),
                expanded: plan.expanded_plot_id.clone(),
                artifact: entry.artifact_id().clone(),
            },
            plan,
        )
        .unwrap();
        let planned_ns = preparation_started.elapsed().as_nanos();
        let phase_started = std::time::Instant::now();
        let definition = artifact.definition().clone();
        let fragment = &definition.internal_plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).unwrap();
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let lowered_ns = phase_started.elapsed().as_nanos();
        let phase_started = std::time::Instant::now();
        let owners = fragment
            .placements
            .iter()
            .map(|gear| {
                assert!(
                    gear.base.is_none() && gear.authority.is_empty() && gear.resources.is_empty()
                );
                let owner = match gear.implementation_id.as_str() {
                    conduitos::expression_host_call::IMPLEMENTATION => Owner::Expression(
                        ExpressionHostCall::prepare(
                            fragment,
                            &lowered,
                            &active,
                            &gear.placement_id,
                        )
                        .unwrap(),
                    ),
                    conduitos::structured_selector_host_call::IMPLEMENTATION => Owner::Selector(
                        SelectorHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
                            .unwrap(),
                    ),
                    other => panic!("unexpected non-generic implementation: {other}"),
                };
                (
                    lowered
                        .identity
                        .node_for_placement(&gear.placement_id)
                        .unwrap(),
                    owner,
                )
            })
            .collect();
        let owner_ns = phase_started.elapsed().as_nanos();
        let phase_started = std::time::Instant::now();
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(ExpressionOperationFactory::default())
            .unwrap();
        registry
            .install(SelectorOperationFactory::default())
            .unwrap();
        let kernel = KernelCompositeHost::prepare_with_sign_storage(
            definition,
            &registry,
            KernelCompositeSignStorage {
                additional_local_items: 60000,
                additional_remote_items: 4096,
            },
        )
        .unwrap();
        Execution {
            kernel,
            owners,
            preparation_nanoseconds: [
                planned_ns,
                lowered_ns,
                owner_ns,
                phase_started.elapsed().as_nanos(),
            ],
        }
    }
}
impl Execution {
    pub fn prepare(source: String, entry_name: &str) -> Self {
        Blueprint::prepare(source, entry_name).realize(0)
    }
    pub fn step(&mut self) -> KernelCompositeStatus {
        let status = self.kernel.step().unwrap();
        if let Some(request) = self.kernel.next_host_request() {
            let node = self
                .kernel
                .host_request_view(&request)
                .unwrap()
                .request
                .node;
            let (_, owner) = self.owners.iter_mut().find(|(id, _)| *id == node).unwrap();
            let required = self.kernel.host_request_obligation(&request).unwrap();
            let admitted = self
                .kernel
                .admit_host_request(
                    &request,
                    &required.host,
                    &required.resources,
                    &required.authorities,
                )
                .unwrap();
            let call = *self
                .kernel
                .admitted_host_request_view(&admitted)
                .unwrap()
                .request;
            let input = self.kernel.host_request_input(&admitted).unwrap();
            let result = match owner {
                Owner::Expression(owner) => {
                    Some(owner.invoke(node, call.call, call.request, input).unwrap())
                }
                Owner::Selector(owner) => {
                    owner.invoke(node, call.call, call.request, input).unwrap()
                }
            };
            match result {
                Some(bytes) => self
                    .kernel
                    .complete_host_call_bytes(&admitted, bytes)
                    .unwrap(),
                None => self
                    .kernel
                    .complete_host_call(
                        &admitted,
                        HostCallOutcome {
                            disposition: HostCallDisposition::Completed,
                            output: None,
                            failure: None,
                        },
                    )
                    .unwrap(),
            }
        }
        status
    }
    pub fn transact(&mut self, sequence: u64, input: &StructuredInfoValue) -> StructuredInfoValue {
        let boundary = &self.kernel.definition().boundary;
        let input_port = boundary.input_fronts[0].external_port.clone();
        let output_port = boundary.output_fronts[0].external_port.clone();
        assert!(matches!(input_port.temporal, PortTemporal::Flow { .. }));
        assert!(matches!(output_port.temporal, PortTemporal::Flow { .. }));
        let payload = ValuePayload {
            value_kind: input_port.value_kind,
            encoded: input.canonical_bytes().unwrap(),
        };
        assert!(matches!(
            self.kernel
                .admit_input(&input_port.port_id, sequence, &payload)
                .unwrap(),
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
        ));
        let mut output = ValuePayload {
            value_kind: output_port.value_kind,
            encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
        };
        for _ in 0..4000 {
            self.step();
            if let Some(received) = self
                .kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
            {
                assert_eq!(received, sequence);
                self.kernel
                    .complete_output(&output_port.port_id, received)
                    .unwrap();
                return StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
            }
        }
        panic!("bounded source graph output");
    }
}
