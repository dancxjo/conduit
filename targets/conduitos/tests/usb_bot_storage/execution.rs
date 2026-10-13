//! Production Kernel/Back topology with deterministic, explicitly admitted
//! physical-call boundaries. This fixture is not a native storage provider.
use super::advertisement;
use conduit_composite::*;
use conduit_core::*;
use conduit_kernel::{HostCallDisposition, HostCallOutcome, NodeId};
use conduit_plan_lowering::lowering::lower_plan_fragment;
use conduitos::{
    expression_host_call::{ExpressionHostCall, ExpressionOperationFactory},
    protocol_source::*,
    structured_selector_host_call::{SelectorHostCall, SelectorOperationFactory},
};
use std::collections::BTreeMap;
enum Pure {
    Expression(ExpressionHostCall),
    Selector(SelectorHostCall),
}
pub(super) struct Execution {
    pub kernel: KernelCompositeHost,
    pure: Vec<(NodeId, Pure)>,
}
impl Execution {
    pub fn prepare(package: ProtocolSourcePackage) -> Self {
        Self::prepare_entry(package, "block-storage-entrance")
    }
    pub fn prepare_entry(package: ProtocolSourcePackage, entry_name: &str) -> Self {
        let bytes = serde_json::to_vec(&package).unwrap();
        let entry = PreparedProtocolEntry::prepare(&bytes, entry_name).unwrap();
        let (mut host, grants) = advertisement::host();
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
                    authority_grants: &grants,
                    protected_resource_grants: &[],
                    line_offers: &[],
                },
            )
            .unwrap();
        let definition = artifact.artifact().definition().clone();
        let fragment = &definition.internal_plan.fragments[0];
        let lowered = lower_plan_fragment(fragment).unwrap();
        let active = bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let mut pure = vec![];
        for gear in &fragment.placements {
            let owner = match gear.implementation_id.as_str() {
                conduitos::expression_host_call::IMPLEMENTATION => Some(Pure::Expression(
                    ExpressionHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
                        .unwrap(),
                )),
                conduitos::structured_selector_host_call::IMPLEMENTATION => Some(Pure::Selector(
                    SelectorHostCall::prepare(fragment, &lowered, &active, &gear.placement_id)
                        .unwrap(),
                )),
                _ => None,
            };
            if let Some(owner) = owner {
                pure.push((
                    lowered
                        .identity
                        .node_for_placement(&gear.placement_id)
                        .unwrap(),
                    owner,
                ));
            }
        }
        let operations = PreparedProtocolSource::prepare(package).unwrap().operations;
        let mut registry = KernelOperationRegistry::new();
        registry
            .install(ExpressionOperationFactory::default())
            .unwrap();
        registry
            .install(SelectorOperationFactory::default())
            .unwrap();
        registry
            .install(conduitos::bytes_literal::BytesLiteralOperationFactory::default())
            .unwrap();
        registry.install(operations.singletons).unwrap();
        registry.install(operations.joins).unwrap();
        registry.install(operations.states).unwrap();
        registry.install(operations.concats).unwrap();
        registry.install(operations.merges).unwrap();
        registry
            .install(
                conduitos::usb_base::control_factory::ControlOperationFactory::prepare_contract()
                    .unwrap(),
            )
            .unwrap();
        registry.install(conduitos::usb_base::endpoint_read_factory::EndpointReadOperationFactory::prepare_contract().unwrap()).unwrap();
        registry.install(conduitos::usb_base::endpoint_write_factory::EndpointWriteOperationFactory::prepare_contract().unwrap()).unwrap();
        let kernel = KernelCompositeHost::prepare_with_sign_storage(
            definition,
            &registry,
            KernelCompositeSignStorage {
                additional_local_items: 60000,
                additional_remote_items: 4096,
            },
        )
        .unwrap();
        Self { kernel, pure }
    }
    pub fn service_pure(&mut self, request: &KernelCompositeHostRequest) -> bool {
        let node = self.kernel.host_request_view(request).unwrap().request.node;
        let Some((_, owner)) = self.pure.iter_mut().find(|(actual, _)| *actual == node) else {
            return false;
        };
        let required = self.kernel.host_request_obligation(request).unwrap();
        let admitted = self
            .kernel
            .admit_host_request(
                request,
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
        let output = match owner {
            Pure::Expression(owner) => {
                Some(owner.invoke(node, call.call, call.request, input).unwrap())
            }
            Pure::Selector(owner) => owner.invoke(node, call.call, call.request, input).unwrap(),
        };
        if let Some(bytes) = output {
            self.kernel
                .complete_host_call_bytes(&admitted, bytes)
                .unwrap();
        } else {
            self.kernel
                .complete_host_call(
                    &admitted,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: None,
                        failure: None,
                    },
                )
                .unwrap();
        }
        true
    }
}
