//! Fixture boundary around the authored parent Plan and production Gear registry.
use conduit_composite::{
    KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeFrontBinding,
    KernelCompositeHost, KernelOperationRegistry,
};
use conduit_core::*;
use conduit_plot::CompositeFrontTerminal;
#[path = "../../../../architecture/plot/tests/common/allocation_probe.rs"]
mod allocation_probe;
#[global_allocator]
static ALLOCATOR: allocation_probe::Allocator = allocation_probe::Allocator;
pub fn run(plan: Plan, front: &CheckedFront, registry: KernelOperationRegistry, tick: Vec<u8>) {
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
            kind_id: "fixture/acoustic-pcm-parent".into(),
            kind_contract_revision: "fixture/acoustic-pcm-parent@1".into(),
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
            capability_id: "fixture/acoustic-pcm-parent@1".into(),
            execution_profile_id: "fixture/acoustic-pcm-parent@1".into(),
            implementation_id: "fixture/acoustic-pcm-parent@1".into(),
            artifact_id: "fixture/acoustic-pcm-parent@1".into(),
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
        profile: "fixture/acoustic-pcm-parent@1".into(),
        boundary: KernelCompositeBoundary {
            input_fronts: front.inputs().iter().map(bind).collect(),
            output_fronts: front.outputs().iter().map(bind).collect(),
        },
        external_capability: offer,
        internal_plan: plan,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    };

    let mut kernel = KernelCompositeHost::prepare(definition, &registry).unwrap();
    let item = ValuePayload {
        value_kind: front.inputs()[0].value_kind.clone(),
        encoded: tick,
    };
    let mut output = ValuePayload {
        value_kind: front.outputs()[0].value_kind.clone(),
        encoded: Vec::with_capacity(2077),
    };
    let input_port = &front.inputs()[0].port_id;
    let output_port = &front.outputs()[0].port_id;
    let mut blocks = [[0_u8; 31]; 4];
    let (_, storage) = allocation_probe::observe(|| {
        kernel.start().unwrap();
        for frame in 0..4 {
            kernel.admit_input(input_port, frame, &item).unwrap();
            let mut delivered = false;
            for _ in 0..5000 {
                kernel.step().unwrap();
                assert!(kernel.next_host_request().is_none());
                if kernel
                    .output_into(output_port, &mut output)
                    .unwrap()
                    .is_some()
                {
                    let (header, samples) =
                        conduit_audio::PcmFrameHeader::decode_frame(&output.encoded).unwrap();
                    assert_eq!(header.sample_rate_hz(), 8000);
                    assert_eq!(header.clock_id(), 1);
                    assert_eq!(header.start_frame(), frame);
                    assert_eq!(header.frame_count(), 1);
                    assert!(!header.discontinuity());
                    if frame == 0 {
                        assert_eq!(samples, 24_i16.to_le_bytes());
                    }
                    blocks[frame as usize].copy_from_slice(&output.encoded);
                    kernel.complete_output(output_port, frame).unwrap();
                    delivered = true;
                    break;
                }
            }
            assert!(delivered);
        }
        kernel.close_input(input_port).unwrap();
        let mut complete = false;
        for _ in 0..64 {
            complete |=
                kernel.step().unwrap() == conduit_composite::KernelCompositeStatus::Complete;
        }
        assert!(complete);
        assert_eq!(
            kernel
                .output_terminal_into(output_port, &mut output)
                .unwrap(),
            Some(conduit_composite::KernelCompositeTerminal::Normal)
        );
    });
    assert_eq!(
        (storage.allocations, storage.reallocations),
        (0, 0),
        "{storage:?}"
    );
    println!("acoustic-plays={:?}", kernel.active_plays());
    println!("acoustic-pcm-blocks={blocks:02x?}; storage={storage:?}");
}
