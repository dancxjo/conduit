//! Sequential generations execute actual Source through admitted native Host Calls.
use super::*;
use alloc::vec::Vec;
use conduit_composite::*;
use conduit_kernel::{NodeId, scheduler::RemoteIngressOutcome};

struct Execution {
    play: KernelCompositeHost,
    host: PreparationHostIdentity,
    expressions: Vec<(NodeId, crate::expression_host_call::ExpressionHostCall)>,
}
impl Execution {
    fn prepare(plan: Plan, owners: ProtocolOperations, offer: CapabilityOffer) -> Self {
        let fragment = &plan.fragments[0];
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment).unwrap();
        let active = bind_active_play(&plan.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        let host = PreparationHostIdentity {
            host_id: fragment.host_id.clone(),
            boot_id: fragment.boot_id.clone(),
            offer_generation: fragment.offer_generation,
        };
        let expressions = fragment
            .placements
            .iter()
            .filter(|gear| {
                gear.implementation_id.as_str() == crate::expression_host_call::IMPLEMENTATION
            })
            .map(|gear| {
                (
                    lowered
                        .identity
                        .node_for_placement(&gear.placement_id)
                        .unwrap(),
                    crate::expression_host_call::ExpressionHostCall::prepare(
                        fragment,
                        &lowered,
                        &active,
                        &gear.placement_id,
                    )
                    .unwrap(),
                )
            })
            .collect();
        let definition = crate::protocol_kernel_fixture::definition(plan, offer);
        let mut registry = KernelOperationRegistry::new();
        registry.install(owners.states).unwrap();
        registry.install(owners.joins).unwrap();
        registry
            .install(crate::expression_host_call::ExpressionOperationFactory::default())
            .unwrap();
        let mut play = KernelCompositeHost::prepare(definition, &registry).unwrap();
        play.start().unwrap();
        Self {
            play,
            host,
            expressions,
        }
    }

    fn step(&mut self) -> KernelCompositeStatus {
        let status = self.play.step().unwrap();
        if let Some(request) = self.play.next_host_request() {
            let admitted = self
                .play
                .admit_host_request(&request, &self.host, &[], &[])
                .unwrap();
            let view = self.play.admitted_host_request_view(&admitted).unwrap();
            let owner = self
                .expressions
                .iter_mut()
                .find(|(node, _)| *node == view.request.node)
                .unwrap();
            let bytes = owner
                .1
                .invoke(
                    view.request.node,
                    view.request.call,
                    view.request.request,
                    self.play.host_request_input(&admitted).unwrap(),
                )
                .unwrap();
            self.play
                .complete_host_call_bytes(&admitted, bytes)
                .unwrap();
        }
        status
    }

    fn observation(&mut self, sequence: u64, kind: &KindId) -> StructuredInfoValue {
        let mut output = ValuePayload {
            value_kind: kind.clone(),
            encoded: Vec::with_capacity(4096),
        };
        for _ in 0..512 {
            self.step();
            if self
                .play
                .output_into(&port_id("current"), &mut output)
                .unwrap()
                == Some(sequence)
            {
                let value = StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap();
                self.play
                    .complete_output(&port_id("current"), sequence)
                    .unwrap();
                return value;
            }
        }
        panic!("generation {sequence} did not reach the declared output");
    }
}

fn field<'a>(value: &'a StructuredInfoValue, name: &str) -> &'a StructuredInfoValue {
    let StructuredInfoValueShape::Record(fields) = value.shape() else {
        panic!("state record")
    };
    fields
        .iter()
        .find(|field| field.name() == name)
        .unwrap()
        .value()
}

#[test]
fn two_queued_events_use_successive_source_state_generations_and_close_normally() {
    let (plan, owners, offer) = planned();
    let (startup, _) = crate::i2c_base::contract::I2cContract::prepare()
        .unwrap()
        .catalogs();
    let types = check_syntax_document(&parse_syntax_document(LIFECYCLE), &startup).unwrap();
    let ty = |name| {
        &types
            .native_types
            .iter()
            .find(|ty| ty.name == name)
            .unwrap()
            .value_type
    };
    let begin = ty("BmeProtocolBegin");
    let StructuredInfoTypeShape::Record { fields, .. } = begin.shape() else {
        panic!("begin")
    };
    let begin_value = StructuredInfoValue::record(
        begin.clone(),
        vec![
            StructuredFieldValue::new(
                "address",
                StructuredInfoValue::leaf(fields[0].value_type().clone(), vec![118]).unwrap(),
            )
            .unwrap(),
        ],
    )
    .unwrap();
    let event = ty("BmeProtocolEvent");
    let StructuredInfoTypeShape::Variant { cases, .. } = event.shape() else {
        panic!("event")
    };
    let tick = cases
        .iter()
        .find(|case| case.tag() == "tick")
        .unwrap()
        .payload_type();
    let events = [1_u64, 0].map(|now| ValuePayload {
        value_kind: event.profile().unwrap().value_kind().clone(),
        encoded: StructuredInfoValue::variant(
            event.clone(),
            "tick",
            StructuredInfoValue::leaf(tick.clone(), now.to_le_bytes().to_vec()).unwrap(),
        )
        .unwrap()
        .canonical_bytes()
        .unwrap(),
    });
    let state_kind = ty("BmeProtocolState")
        .profile()
        .unwrap()
        .value_kind()
        .clone();
    let mut execution = Execution::prepare(plan, owners, offer);
    execution
        .play
        .admit_input(
            &port_id("begin"),
            0,
            &ValuePayload {
                value_kind: begin.profile().unwrap().value_kind().clone(),
                encoded: begin_value.canonical_bytes().unwrap(),
            },
        )
        .unwrap();
    execution.play.close_input(&port_id("begin")).unwrap();
    let initialized = execution.observation(0, &state_kind);
    assert!(
        matches!(field(&initialized, "phase").shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [0])
    );
    assert!(matches!(
        execution
            .play
            .admit_input(&port_id("event"), 0, &events[0])
            .unwrap(),
        RemoteIngressOutcome::Accepted { sequence: 0 }
    ));
    let mut accepted = false;
    for _ in 0..16 {
        execution.step();
        if matches!(
            execution
                .play
                .admit_input(&port_id("event"), 1, &events[1])
                .unwrap(),
            RemoteIngressOutcome::Accepted { sequence: 1 }
        ) {
            accepted = true;
            break;
        }
    }
    assert!(
        accepted,
        "next event remains queued before the preceding transition completes"
    );
    let first = execution.observation(1, &state_kind);
    assert!(
        matches!(field(&first, "clock").shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == 1_u64.to_le_bytes())
    );
    let second = execution.observation(2, &state_kind);
    assert!(
        matches!(field(&second, "phase").shape(), StructuredInfoValueShape::Leaf(bytes) if bytes == [15])
    );
    assert!(
        matches!(field(&second, "failure").shape(), StructuredInfoValueShape::Variant { tag, .. } if tag == "malformed")
    );
    execution.play.close_input(&port_id("event")).unwrap();
    let mut status = KernelCompositeStatus::Active;
    for _ in 0..128 {
        status = execution.step();
        if status == KernelCompositeStatus::Complete {
            break;
        }
    }
    assert_eq!(status, KernelCompositeStatus::Complete);
}
