//! Actual production-kernel class parsing with independently prepared pure owners.
use super::common::SOURCE;
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
pub(super) struct Execution {
    pub(super) kernel: KernelCompositeHost,
    owners: Vec<(NodeId, Owner)>,
    pub(super) input_schema: StructuredInfoType,
}
impl Execution {
    pub(super) fn prepare(entry_name: &str) -> Self {
        let package = ProtocolSourcePackage::compile(SOURCE.into(), &[]).unwrap();
        let entry =
            PreparedProtocolEntry::prepare(&serde_json::to_vec(&package).unwrap(), entry_name)
                .unwrap();
        let input_schema = entry.input_schema(&port_id("frame")).unwrap();
        let mut host = HostAdvertisement {
            protocol_version: PROTOCOL_VERSION,
            host_id: "fixture/hid-reports".into(),
            boot_id: "fixture/hid-boot".into(),
            offer_generation: OfferGeneration(1),
            profile: "fixture/pure-hid-class".into(),
            bases: vec![],
            resources: vec![],
            capabilities: vec![],
            planner_capabilities: vec![],
        };
        entry.publish_pure_backs(&mut host).unwrap();
        let hosts = [host];
        let choices = entry.placements(&hosts).unwrap();
        let artifact = entry
            .plan(
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
            )
            .unwrap();
        let definition = artifact.artifact().definition().clone();
        let fragment = &definition.internal_plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).unwrap();
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
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
                    other => panic!("unexpected physical or private class implementation: {other}"),
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
                additional_remote_items: 1024,
            },
        )
        .unwrap();
        Self {
            kernel,
            owners,
            input_schema,
        }
    }
    pub(super) fn step(&mut self) -> KernelCompositeStatus {
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
}
