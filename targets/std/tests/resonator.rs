//! Ordinary installed Host/Plan/Play for the checked finite resonator.
use conduit_core::{
    BaseImplementationId, ConfigurationValue, ConnectionTrack, PortDirection, StructuredFieldValue,
    StructuredInfoType, StructuredInfoTypeShape, StructuredInfoValue, StructuredInfoValueShape,
};
use conduit_plot::{
    check_syntax_document, expand_canonical_plot_for_authoring, parse_syntax_document,
    PortableExpressionProgram, ProfileCatalog, StartupCatalog,
};
use conduit_std_host::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use std::{collections::BTreeMap, time::Duration};
#[path = "../../../architecture/plot/tests/common/resonator.rs"]
mod fixture;
use fixture::{field, integer, leaf};
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("resonator has no timer/device effect");
    }
}
#[derive(Default)]
struct Outputs(Vec<Vec<u8>>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value.bytes);
        }
        Ok(())
    }
}
fn prepare(source: &str, entry: &str, input_port: &str) -> (StdHost, conduit_core::Plan) {
    let checked =
        check_syntax_document(&parse_syntax_document(source), &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, entry, &ProfileCatalog::new()).unwrap();
    let mut advertisement = StdHost::new().advertisement().clone();
    for gear in &authored.expanded.gears {
        let ConfigurationValue::Text(encoded) = &gear.configuration[0].value else {
            panic!("program")
        };
        let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
        let offer =
            conduit_std_offers::pure_expression_std_offer(&program, gear.inputs[0].temporal)
                .unwrap();
        assert!(gear.accepts_realization(&offer));
        if !advertisement
            .capabilities
            .iter()
            .any(|old| old.capability_id == offer.capability_id)
        {
            advertisement.capabilities.push(offer);
        }
    }
    advertisement
        .capabilities
        .sort_by(|a, b| a.capability_id.cmp(&b.capability_id));
    let host = StdHost::from_advertisement(advertisement).unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, input_port),
        (PortDirection::Output, "result"),
    ]
    .into_iter()
    .map(|(direction, port)| {
        (
            conduit_planner::ForeBoundaryKey {
                direction,
                front_port_id: port.into(),
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
        &placements,
        &[BaseImplementationId::from("conduit.base/local@1")],
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
    assert!(conduit_core::verify_plan(&plan));
    (host, plan)
}
fn run(
    host: &mut StdHost,
    plan: &conduit_core::Plan,
    port: &str,
    inputs: Vec<Vec<u8>>,
) -> Vec<Vec<u8>> {
    let count = inputs.len();
    let inputs = inputs
        .into_iter()
        .map(|bytes| ExternalForeInput {
            front_port_id: port.into(),
            track: ConnectionTrack::Payload,
            bytes,
        })
        .collect::<Vec<_>>();
    let mut output = Outputs(Vec::with_capacity(count));
    for input in &inputs {
        // Each authored Item advances one finite ordinary Play. Caller batching
        // and fresh-host partitioning do not introduce a second runtime loop.
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                std::slice::from_ref(input),
                &mut output,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        let kernel = report.kernel.unwrap();
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallRequested));
        assert!(kernel
            .kernel_sign
            .iter()
            .any(|event| event.kind == conduit_kernel::KernelEventKind::HostCallCompleted));
    }
    assert_eq!(output.0.len(), count);
    output.0
}

fn record(ty: &StructuredInfoType, values: &[(&str, StructuredInfoValue)]) -> StructuredInfoValue {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    StructuredInfoValue::record(
        ty.clone(),
        fields
            .iter()
            .map(|f| {
                StructuredFieldValue::new(
                    f.name(),
                    values
                        .iter()
                        .find(|(n, _)| *n == f.name())
                        .unwrap()
                        .1
                        .clone(),
                )
                .unwrap()
            })
            .collect(),
    )
    .unwrap()
}
fn field_type<'a>(ty: &'a StructuredInfoType, name: &str) -> &'a StructuredInfoType {
    let StructuredInfoTypeShape::Record { fields, .. } = ty.shape() else {
        panic!("record")
    };
    fields
        .iter()
        .find(|f| f.name() == name)
        .unwrap()
        .value_type()
}
fn input(
    ty: &StructuredInfoType,
    cursor: &StructuredInfoValue,
    sample: i128,
    cancelled: bool,
) -> Vec<u8> {
    record(
        ty,
        &[
            ("cursor", cursor.clone()),
            (
                "sample",
                StructuredInfoValue::leaf(
                    field_type(ty, "sample").clone(),
                    sample.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            (
                "cancelled",
                StructuredInfoValue::leaf(
                    field_type(ty, "cancelled").clone(),
                    vec![u8::from(cancelled)],
                )
                .unwrap(),
            ),
        ],
    )
    .canonical_bytes()
    .unwrap()
}
fn outcome(bytes: &[u8], expected: &str) -> StructuredInfoValue {
    let value = StructuredInfoValue::from_canonical_bytes(bytes).unwrap();
    let StructuredInfoValueShape::Variant { tag, payload } = value.shape() else {
        panic!("outcome")
    };
    assert_eq!(tag, expected);
    payload.clone()
}
#[test]
fn normal_resonator_plans_preserve_exact_cursor_and_boundaries_across_fresh_hosts() {
    let source = fixture::source();
    let prepare_programs = fixture::programs("dsp/resonator-prepare");
    let step_programs = fixture::programs("dsp/resonator-step");
    let request = fixture::request(
        &prepare_programs[0].input_type,
        "",
        (1000, 1),
        (100, 1),
        8000,
    );
    let (mut prepare_host, prepare_plan) = prepare(&source, "dsp/resonator-prepare", "value");
    assert_eq!(
        prepare_plan.source_document_id,
        parse_syntax_document(&source).source_document_id()
    );
    let initial = outcome(
        &run(
            &mut prepare_host,
            &prepare_plan,
            "value",
            vec![request.canonical_bytes().unwrap()],
        )[0],
        "started",
    );
    assert_eq!(
        outcome(
            &run(
                &mut prepare_host,
                &prepare_plan,
                "value",
                vec![request.canonical_bytes().unwrap()]
            )[0],
            "started"
        ),
        initial
    );
    let (mut host, plan) = prepare(&source, "dsp/resonator-step", "value");
    assert_eq!(plan.source_document_id, prepare_plan.source_document_id);
    let mut cursor = initial.clone();
    let (mut y1, mut y2) = (0i128, 0i128);
    let mut expected_bytes = Vec::new();
    for frame in 0..64 {
        let sample = if frame == 0 { 32767 } else { 0 };
        let expected = (sample * 16384 + 22278 * y1 - 15146 * y2) / 16384;
        let bytes = run(
            &mut host,
            &plan,
            "value",
            vec![input(&step_programs[0].input_type, &cursor, sample, false)],
        )
        .remove(0);
        let advanced = outcome(&bytes, "advanced");
        assert_eq!(integer(&advanced, "sample"), expected);
        cursor = field(&advanced, "cursor").clone();
        assert_eq!(field(&cursor, "request"), &request);
        assert_eq!(integer(&cursor, "y1"), expected);
        assert_eq!(integer(&cursor, "y2"), y1);
        y2 = y1;
        y1 = expected;
        expected_bytes.push(bytes);
    }
    assert_eq!(
        u32::from_le_bytes(leaf(field(&cursor, "remaining")).try_into().unwrap()),
        0
    );
    let bytes = run(
        &mut host,
        &plan,
        "value",
        vec![input(&step_programs[0].input_type, &cursor, 0, false)],
    )
    .remove(0);
    assert_eq!(outcome(&bytes, "exhausted"), cursor);
    let mut resumed = initial;
    for range in [0..17, 17..43, 43..64] {
        let mut fresh = StdHost::from_advertisement(host.advertisement().clone()).unwrap();
        for frame in range {
            let sample = if frame == 0 { 32767 } else { 0 };
            let bytes = run(
                &mut fresh,
                &plan,
                "value",
                vec![input(&step_programs[0].input_type, &resumed, sample, false)],
            )
            .remove(0);
            assert_eq!(bytes, expected_bytes[frame]);
            resumed = field(&outcome(&bytes, "advanced"), "cursor").clone();
        }
    }
    assert_eq!(resumed, cursor);
    let bytes = run(
        &mut host,
        &plan,
        "value",
        vec![input(&step_programs[0].input_type, &cursor, 0, true)],
    )
    .remove(0);
    assert_eq!(outcome(&bytes, "cancelled"), cursor);
    let bytes = run(
        &mut host,
        &plan,
        "value",
        vec![input(
            &step_programs[0].input_type,
            &cursor,
            i128::MAX,
            false,
        )],
    )
    .remove(0);
    outcome(&bytes, "invalid_cursor");
}
