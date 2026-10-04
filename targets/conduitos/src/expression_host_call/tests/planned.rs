//! The native factory admits and executes the sealed ordinary expression Plan.
use super::*;
use conduit_composite::*;
use conduit_plot::CompositeFrontTerminal;

fn definition(fragment: PlanFragment) -> KernelCompositeDefinition {
    let gear = &fragment.placements[0];
    let front = |port: &PortDescriptor| KernelCompositeFrontBinding {
        external_port: port.clone(),
        internal_child: fragment.host_id.clone(),
        internal_placement_id: gear.placement_id.clone(),
        internal_port_id: port.port_id.clone(),
        terminal: CompositeFrontTerminal::Independent,
    };
    let boundary = KernelCompositeBoundary {
        input_fronts: vec![front(&gear.inputs[0])],
        output_fronts: vec![front(&gear.outputs[0])],
    };
    let identity = PlotIdentity {
        source_document_id: fragment.source_document_id.clone(),
        checked_plot_id: fragment.checked_plot_id.clone(),
        expanded_plot_id: fragment.expanded_plot_id.clone(),
    };
    let mut external = offer(&program(), PortTemporal::Value).unwrap();
    external.limits.max_queue_items = 1;
    external.limits.max_queue_bytes = 1;
    KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: HostProfileId::from("fixture/native-expression@1"),
        external_capability: external,
        internal_plan: seal_plan(identity, vec![fragment]),
        boundary,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    }
}

#[test]
fn native_factory_executes_exact_plan_and_retains_unacknowledged_output() {
    let (fragment, lowered, active, placement) = selected();
    let factory = ExpressionOperationFactory::default();
    let budget = factory.budget(&fragment.placements[0]).unwrap();
    assert_eq!(budget.maximum_value_bytes, 1);
    assert_eq!(budget.value_bytes, 4);
    let mut registry = KernelOperationRegistry::new();
    registry.install(factory).unwrap();
    let mut native = ExpressionHostCall::prepare(&fragment, &lowered, &active, &placement).unwrap();
    let mut host = KernelCompositeHost::prepare(definition(fragment), &registry).unwrap();
    host.start().unwrap();
    host.admit_input(
        &port_id("input"),
        0,
        &ValuePayload {
            value_kind: kind_id("value/u8"),
            encoded: vec![41],
        },
    )
    .unwrap();
    host.close_input(&port_id("input")).unwrap();
    let mut calls = 0;
    let mut output = ValuePayload {
        value_kind: kind_id("value/u8"),
        encoded: Vec::with_capacity(1),
    };
    let mut sequence = None;
    for _ in 0..32 {
        host.step().unwrap();
        if let Some(request) = host.next_host_request() {
            let exact = host.host_request_obligation(&request).unwrap();
            let admitted = host
                .admit_host_request(&request, &exact.host, &[], &[])
                .unwrap();
            let call = *host.admitted_host_request_view(&admitted).unwrap().request;
            let bytes = native
                .invoke(
                    call.node,
                    call.call,
                    call.request,
                    host.host_request_input(&admitted).unwrap(),
                )
                .unwrap();
            host.complete_host_call_bytes(&admitted, bytes).unwrap();
            calls += 1;
        }
        sequence = host.output_into(&port_id("output"), &mut output).unwrap();
        if sequence.is_some() {
            break;
        }
    }
    assert_eq!(calls, 1);
    assert_eq!(sequence, Some(0));
    assert_eq!(output.encoded, [42]);
    for _ in 0..1000 {
        host.step().unwrap();
        assert!(host.next_host_request().is_none());
        assert_eq!(
            host.output_into(&port_id("output"), &mut output).unwrap(),
            Some(0)
        );
        assert_eq!(output.encoded, [42]);
    }
    host.complete_output(&port_id("output"), 0).unwrap();
    assert_eq!(
        host.output_into(&port_id("output"), &mut output).unwrap(),
        None
    );
}
