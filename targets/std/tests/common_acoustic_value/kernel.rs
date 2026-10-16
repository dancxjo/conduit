//! Standalone fixture boundary around the actual planned child kernel.
use conduit_composite::{
    KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeFrontBinding,
    KernelCompositeHost, KernelOperationRegistry,
};
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;
use conduit_std_host::flow_activation::{LocalExpressionFactory, PreparedValuePairFactory};

pub fn run(
    plan: Plan,
    front: &CheckedFront,
    pair: PreparedValuePairFactory,
    accumulator: Vec<u8>,
    item: Vec<u8>,
    cancel_held: bool,
) -> Option<Vec<u8>> {
    let fragment = &plan.fragments[0];
    let bind = |port: &PortDescriptor| {
        let selected = fragment
            .fore_ports
            .iter()
            .find(|selected| {
                selected.front_port_id == port.port_id && selected.direction == port.direction
            })
            .unwrap();
        KernelCompositeFrontBinding {
            external_port: port.clone(),
            internal_child: fragment.host_id.clone(),
            internal_placement_id: selected.placement_id.clone(),
            internal_port_id: selected.gear_port_id.clone(),
            terminal: CompositeFrontTerminal::Independent,
        }
    };
    // This wrapper is a named fixture boundary, not a new DSP Kind or Back.
    // The inner plan selects only the production expression and pair factories.
    let offer = BackOfferBuilder::new(
        Kind {
            startup_parameters: vec![],
            shorthand: None,
            kind_id: "fixture/acoustic-value-child".into(),
            kind_contract_revision: "fixture/acoustic-value-child@1".into(),
            inputs: front.inputs().to_vec(),
            outputs: front.outputs().to_vec(),
            configuration: vec![],
            semantic_laws: vec![KindSemanticLaw::ValueContracts(
                front.value_contracts().to_vec(),
            )],
            limits: CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 32768,
            },
        },
        Back {
            capability_id: "fixture/acoustic-value-child@1".into(),
            execution_profile_id: "fixture/acoustic-value-child@1".into(),
            implementation_id: "fixture/acoustic-value-child@1".into(),
            artifact_id: "fixture/acoustic-value-child@1".into(),
            host_calls: vec![],
            resource_requirements: vec![],
            authority_requirements: vec![],
        },
    )
    .build();
    let definition = KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/acoustic-value-child@1".into(),
        boundary: KernelCompositeBoundary {
            input_fronts: front.inputs().iter().map(bind).collect(),
            output_fronts: front.outputs().iter().map(bind).collect(),
        },
        external_capability: offer,
        internal_plan: plan,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };
    let mut registry = KernelOperationRegistry::new();
    registry.install(LocalExpressionFactory).unwrap();
    registry.install(pair).unwrap();
    let mut child = KernelCompositeHost::prepare(definition, &registry).unwrap();
    let inputs = front
        .inputs()
        .iter()
        .map(|port| ValuePayload {
            value_kind: port.value_kind.clone(),
            encoded: if port.port_id.as_str() == "accumulator" {
                accumulator.clone()
            } else {
                item.clone()
            },
        })
        .collect::<Vec<_>>();
    let mut output = ValuePayload {
        value_kind: front.outputs()[0].value_kind.clone(),
        encoded: Vec::with_capacity(32768),
    };
    let output_port = &front.outputs()[0].port_id;
    let mut cancelled = false;
    let (_, storage) = super::allocation_probe::observe(|| {
        child.start().unwrap();
        for (port, value) in front.inputs().iter().zip(&inputs) {
            child.admit_input(&port.port_id, 0, value).unwrap();
            child.close_input(&port.port_id).unwrap();
        }
        let mut delivered = false;
        let mut holds = 0;
        let mut output_digest = None;
        for _ in 0..1040 {
            child.step().unwrap_or_else(|error| {
                let signs = child.signs();
                let tail = signs.values().flatten().rev().take(8).collect::<Vec<_>>();
                panic!("{error:?}; final kernel signs: {tail:?}")
            });
            assert!(child.next_host_request().is_none());
            if child
                .output_into(output_port, &mut output)
                .unwrap()
                .is_some()
            {
                assert!(!delivered);
                let digest = semantic_digest("fixture/acoustic-child-output", &output.encoded);
                if let Some(expected) = output_digest {
                    assert_eq!(digest, expected);
                } else {
                    output_digest = Some(digest);
                }
                holds += 1;
                if cancel_held && holds == 2 {
                    child.cancel().unwrap();
                    assert!(matches!(
                        child.output_into(output_port, &mut output),
                        Err(conduit_composite::KernelCompositeError::InvalidLifecycle)
                    ));
                    cancelled = true;
                    return;
                }
                if holds == 4 {
                    child.complete_output(output_port, 0).unwrap();
                    delivered = true;
                }
            }
        }
        assert!(delivered);
    });
    assert_eq!(
        storage.allocations, 0,
        "prepared child advancement must not allocate: {storage:?}"
    );
    assert_eq!(storage.reallocations, 0);
    let signs = child.signs();
    assert!(signs.values().flatten().all(|event| !matches!(
        event.kind,
        conduit_kernel::KernelEventKind::HostCallRequested
            | conduit_kernel::KernelEventKind::HostCallCompleted
    )));
    assert_eq!(cancelled, cancel_held);
    (!cancelled).then_some(output.encoded)
}
