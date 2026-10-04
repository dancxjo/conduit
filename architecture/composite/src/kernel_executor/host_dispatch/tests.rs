//! Kernel completion ownership and zero-extent canonical values.
use super::*;
use crate::test_support::{allocation, common, single_child};
use conduit_core::*;
use conduit_kernel::scheduler::{HostCallBack, SchedulerError, StepBack};
use conduit_kernel::{HostedValueStore, RequestId};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

struct CallFactory(ImplementationId);
impl crate::KernelOperationFactory for CallFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.0
    }
    fn budget(&self, _: &PlannedGear) -> Result<crate::KernelOperationBudget, String> {
        Ok(crate::KernelOperationBudget {
            value_items: 2,
            value_bytes: 2,
            maximum_value_bytes: 1,
            host_requests: 1,
            sign_items: 8,
        })
    }
    fn prepare(
        &self,
        _: &PlannedGear,
        _: &mut HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        Ok(Box::new(HostCallBack::new(1)))
    }
}

fn pending_call() -> (KernelCompositeHost, AdmittedKernelCompositeHostRequest) {
    let mut fragment = common::fragment();
    let placement = &mut fragment.placements[0];
    placement.inputs[0].value_kind = kind_id(BOOL_INFO_ID);
    placement.outputs[0].value_kind = kind_id(UNIT_INFO_ID);
    placement.host_calls = vec![HostCallRequirement {
        contract_id: "fixture/unit-result@1".into(),
        target_kind: None,
        maximum_in_flight: 1,
        maximum_input_bytes: 1,
        maximum_output_bytes: 1,
    }];
    let implementation = placement.implementation_id.clone();
    let input = placement.inputs[0].clone();
    let definition = single_child(fragment, 1);
    let mut registry = KernelOperationRegistry::new();
    registry.install(CallFactory(implementation)).unwrap();
    let mut host = KernelCompositeHost::prepare(definition, &registry).unwrap();
    host.start().unwrap();
    host.admit_input(
        &input.port_id,
        0,
        &ValuePayload {
            value_kind: input.value_kind,
            encoded: vec![1],
        },
    )
    .unwrap();
    host.step().unwrap();
    let request = host.next_host_request().unwrap();
    let obligation = host.host_request_obligation(&request).unwrap();
    let admitted = host
        .admit_host_request(
            &request,
            &obligation.host,
            &obligation.resources,
            &obligation.authorities,
        )
        .unwrap();
    (host, admitted)
}

#[test]
fn rejected_stored_completions_release_the_value_and_preserve_exact_dispatch() {
    let (mut host, admitted) = pending_call();
    let slot = host
        .outstanding_host_call_index(admitted.dispatch_token)
        .unwrap();
    let exact = host.outstanding_host_calls[slot].as_ref().unwrap().request;
    // Inject a stale correlation inside this private fixture, after dispatch.
    host.outstanding_host_calls[slot]
        .as_mut()
        .unwrap()
        .request
        .request = RequestId(99);
    let allocations = allocation::allocations_during(|| {
        for _ in 0..1_000 {
            assert_eq!(
                host.complete_host_call_bytes(&admitted, &[]),
                Err(KernelCompositeError::Execution {
                    child: 0,
                    reason: ChildExecutionError::Scheduler(
                        SchedulerError::HostCallCompletionRejected
                    ),
                })
            );
        }
        host.outstanding_host_calls[slot].as_mut().unwrap().request = exact;
        host.complete_host_call_bytes(&admitted, &[]).unwrap();
        assert_eq!(
            host.complete_host_call_bytes(&admitted, &[]),
            Err(KernelCompositeError::InvalidHostCallToken)
        );
    });
    assert_eq!(allocations, 0);
}

#[test]
fn unit_completion_retains_zero_extent_and_is_delivered_once() {
    let (mut host, admitted) = pending_call();
    let output = port_id("current");
    let mut payload = ValuePayload {
        value_kind: kind_id(UNIT_INFO_ID),
        encoded: Vec::new(),
    };
    let allocations = allocation::allocations_during(|| {
        host.complete_host_call_bytes(&admitted, &[]).unwrap();
        host.step().unwrap();
        assert_eq!(host.output_into(&output, &mut payload).unwrap(), Some(0));
        assert!(payload.encoded.is_empty());
        host.complete_output(&output, 0).unwrap();
        assert_eq!(host.output_into(&output, &mut payload).unwrap(), None);
    });
    assert_eq!(allocations, 0);
}
