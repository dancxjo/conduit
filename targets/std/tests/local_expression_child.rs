//! Explicit computational profile: actual checked Source, planner and child kernel.
//! This proves a generic Gear implementation, not a complete PCM flow.
use conduit_composite::{
    KernelCompositeBoundary, KernelCompositeDefinition, KernelCompositeFrontBinding,
    KernelCompositeHost, KernelOperationFactory, KernelOperationRegistry,
};
use conduit_core::*;
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    KernelEventKind, PortId as KPort, ValueRef,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    CompositeFrontTerminal, PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    flow_activation::{LocalExpressionFactory, PreparedLocalExpressionBack},
    StdHost,
};
use std::{
    alloc::{GlobalAlloc, Layout, System},
    cell::Cell,
    collections::BTreeMap,
};
thread_local! { static ENABLED: Cell<bool> = const { Cell::new(false) }; static CALLS: Cell<usize> = const { Cell::new(0) }; }
struct Counted;
fn record() {
    let _ = ENABLED.try_with(|v| {
        if v.get() {
            let _ = CALLS.try_with(|v| v.set(v.get() + 1));
        }
    });
}
unsafe impl GlobalAlloc for Counted {
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc(l) }
    }
    unsafe fn alloc_zeroed(&self, l: Layout) -> *mut u8 {
        record();
        unsafe { System.alloc_zeroed(l) }
    }
    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        record();
        unsafe { System.realloc(p, l, n) }
    }
    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) }
    }
}
#[global_allocator]
static ALLOCATOR: Counted = Counted;
fn measured(f: impl FnOnce()) {
    CALLS.with(|v| v.set(0));
    ENABLED.with(|v| v.set(true));
    f();
    ENABLED.with(|v| v.set(false));
    assert_eq!(
        CALLS.with(Cell::get),
        0,
        "prepared local execution must not allocate"
    );
}
fn planned() -> (PortableExpressionProgram, Plan, CapabilityOffer) {
    let source = "plot local (\n input: I32 >> output: I32\n) = (. + 1)\n";
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "local", &ProfileCatalog::new()).unwrap();
    assert_eq!(authored.expanded.gears.len(), 1);
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let offer = conduit_std_offers::pure_expression_local_offer(&program).unwrap();
    assert!(authored.expanded.gears[0].accepts_realization(&offer));
    let mut host = StdHost::new().advertisement().clone();
    host.host_id = "fixture/local-expression-child".into();
    host.capabilities = vec![offer.clone()];
    let hosts = [host];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "input"),
        (PortDirection::Output, "output"),
    ]
    .into_iter()
    .map(|(direction, name)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: name.into(),
                track: ConnectionTrack::Payload,
            },
            conduit_planner::ConnectionQueueLimits {
                item_capacity: 1,
                byte_capacity: 4,
            },
        )
    })
    .collect();
    let plan = conduit_planner::plan_expanded_authoring_with_options(
        &authored,
        &hosts,
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
        conduit_planner::PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: 4,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    (program, plan, offer)
}
fn definition(plan: Plan, offer: CapabilityOffer) -> KernelCompositeDefinition {
    let fragment = &plan.fragments[0];
    let placement = &fragment.placements[0];
    let bind = |port: &PortDescriptor| KernelCompositeFrontBinding {
        external_port: port.clone(),
        internal_child: fragment.host_id.clone(),
        internal_placement_id: placement.placement_id.clone(),
        internal_port_id: port.port_id.clone(),
        terminal: CompositeFrontTerminal::Independent,
    };
    KernelCompositeDefinition {
        host_id: fragment.host_id.clone(),
        boot_id: fragment.boot_id.clone(),
        offer_generation: fragment.offer_generation,
        profile: "fixture/local-expression-child@1".into(),
        boundary: KernelCompositeBoundary {
            input_fronts: vec![bind(&placement.inputs[0])],
            output_fronts: vec![bind(&placement.outputs[0])],
        },
        external_capability: offer,
        internal_plan: plan,
        failure_translation: FailureReason::CompositeCapabilityFailed,
    }
}
#[test]
fn source_plan_executes_in_existing_child_kernel_without_host_calls_or_allocations() {
    let (_, plan, offer) = planned();
    let definition = definition(plan, offer);
    let input_port = definition.boundary.input_fronts[0].external_port.clone();
    let output_port = definition.boundary.output_fronts[0].external_port.clone();
    let mut registry = KernelOperationRegistry::new();
    registry.install(LocalExpressionFactory).unwrap();
    let mut host = KernelCompositeHost::prepare(definition, &registry).unwrap();
    let input = ValuePayload {
        value_kind: input_port.value_kind.clone(),
        encoded: 41_i32.to_le_bytes().to_vec(),
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind.clone(),
        encoded: Vec::with_capacity(4),
    };
    measured(|| {
        host.start().unwrap();
        host.admit_input(&input_port.port_id, 0, &input).unwrap();
        host.close_input(&input_port.port_id).unwrap();
        let mut delivered = false;
        for _ in 0..16 {
            host.step().unwrap();
            assert!(host.next_host_request().is_none());
            if host
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
                .is_some()
            {
                assert!(!delivered);
                assert_eq!(output.encoded, 42_i32.to_le_bytes());
                host.complete_output(&output_port.port_id, 0).unwrap();
                delivered = true;
            }
        }
        assert!(delivered);
    });
    let signs = host.signs();
    assert!(!signs.is_empty());
    assert!(signs.values().flatten().all(|e| !matches!(
        e.kind,
        KernelEventKind::HostCallRequested | KernelEventKind::HostCallCompleted
    )));
}
#[test]
fn local_profile_is_distinct_and_factory_refuses_stale_or_effectful_placements() {
    let (program, plan, local) = planned();
    let hosted =
        conduit_std_offers::pure_expression_std_offer(&program, PortTemporal::Value).unwrap();
    assert_eq!(local.kind_id, hosted.kind_id);
    assert_eq!(local.inputs, hosted.inputs);
    assert_eq!(local.outputs, hosted.outputs);
    assert_eq!(local.semantic_contract, hosted.semantic_contract);
    assert_ne!(local.implementation, hosted.implementation);
    assert!(local.host_calls.is_empty());
    assert!(!hosted.host_calls.is_empty());
    let placement = &plan.fragments[0].placements[0];
    let budget = LocalExpressionFactory.budget(placement).unwrap();
    assert_eq!(budget.host_requests, 0);
    assert_eq!(budget.maximum_value_bytes, 4);
    for field in 0..6 {
        let mut forged = placement.clone();
        match field {
            0 => forged.artifact_id = "forged/artifact".into(),
            1 => forged.execution_profile_id = hosted.implementation.execution_profile_id.clone(),
            2 => forged.capability_id = "forged/capability".into(),
            3 => forged.host_calls = hosted.host_calls.clone(),
            4 => forged.inputs[0].temporal = PortTemporal::Flow { closes: true },
            5 => forged.semantic_contract = Default::default(),
            _ => unreachable!(),
        }
        assert!(LocalExpressionFactory.budget(&forged).is_err());
    }
}
fn frame<'a>(input: &'a [u8], ready: bool, fuel: u16) -> (StepIo<1>, StepInputBytes<'a, 1>) {
    (
        StepIo::test_frame(
            [Some(ValueRef {
                slot: 0,
                generation: 1,
                byte_len: input.len() as u32,
            })],
            [false],
            [ready.then_some(4)],
            None,
            fuel,
        ),
        StepInputBytes::test_frame([Some(input)], None),
    )
}
#[test]
fn pressure_retry_and_cancel_preserve_transactional_prepared_output() {
    let (program, _, _) = planned();
    let mut back = PreparedLocalExpressionBack::prepare(&program).unwrap();
    let capacity = back.output_capacity();
    let input = 7_i32.to_le_bytes();
    measured(|| {
        let (mut io, bytes) = frame(&input, false, 8);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(!io.test_consumed(KPort(0)));
        let (mut io, bytes) = frame(&input, true, 8);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
        assert_eq!(
            <PreparedLocalExpressionBack as StepBack<1>>::prepared_output(&back, KPort(0)),
            Some(8_i32.to_le_bytes().as_slice())
        );
        // Simulate rejection by withholding step_committed, then retry pressure.
        let (mut io, bytes) = frame(&input, false, 8);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Await);
        assert!(
            <PreparedLocalExpressionBack as StepBack<1>>::prepared_output(&back, KPort(0))
                .is_none()
        );
        let (mut io, bytes) = frame(&input, true, 8);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Progress);
        <PreparedLocalExpressionBack as StepBack<1>>::step_committed(&mut back);
        let (mut io, bytes) = frame(&input, true, 8);
        assert_eq!(back.step(&mut io, &bytes), StepOutcome::Complete);
        assert!(!io.test_consumed(KPort(0)));
    });
    assert_eq!(back.output_capacity(), capacity);
    let mut cancelled = PreparedLocalExpressionBack::prepare(&program).unwrap();
    <PreparedLocalExpressionBack as StepBack<1>>::cancel(&mut cancelled);
    let (mut io, bytes) = frame(&input, true, 8);
    assert_eq!(cancelled.step(&mut io, &bytes), StepOutcome::Complete);
    assert!(!io.test_consumed(KPort(0)));
}
