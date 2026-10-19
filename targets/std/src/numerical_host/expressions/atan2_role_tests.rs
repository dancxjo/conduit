//! Typed difference quantities through ordinary Source, sealed Plan and installed kernel.
use crate::{
    ExternalForeDelivery, ExternalForeInput, ExternalForeOutputAdapter, StdHost, TimerAdapter,
};
use conduit_core::*;
use conduit_plot::*;
use std::{collections::BTreeMap, time::Duration};
const SOURCE: &str = "type Components = {\n y: Quantity\n x: Quantity\n}\nplot bearing (\n >> value: Components <= 4096B\n result: MathematicalAngle >>\n) = (math/atan2(.y, .x))\n";
struct NoTimer;
impl TimerAdapter for NoTimer {
    fn wait(&mut self, _: Duration) {
        panic!("no clock effect")
    }
}
struct Outputs(Vec<ExternalForeDelivery>);
impl ExternalForeOutputAdapter for Outputs {
    fn deliver(&mut self, value: ExternalForeDelivery) -> Result<(), String> {
        if value.track == ConnectionTrack::Payload {
            self.0.push(value);
        }
        Ok(())
    }
}
fn prepared() -> (StdHost, Plan, PortableExpressionProgram) {
    let syntax = parse_syntax_document(SOURCE);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &StartupCatalog::new()).unwrap();
    let authored =
        expand_canonical_plot_for_authoring(&checked, "bearing", &ProfileCatalog::new()).unwrap();
    let ConfigurationValue::Text(encoded) = &authored.expanded.gears[0].configuration[0].value
    else {
        panic!("canonical program")
    };
    let program = PortableExpressionProgram::from_canonical_hex(encoded).unwrap();
    let mut host = StdHost::new();
    host.prepare_expression_realizations(&authored.expanded)
        .unwrap();
    let hosts = [host.advertisement().clone()];
    let placements =
        conduit_planner::default_expanded_placements(&authored.expanded, &hosts).unwrap();
    let boundaries = [
        (PortDirection::Input, "value"),
        (PortDirection::Output, "result"),
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
                byte_capacity: 4096,
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
            connection_byte_capacity: 4096,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
        &boundaries,
    )
    .unwrap();
    assert!(verify_plan(&plan));
    assert_eq!(plan.source_document_id, syntax.source_document_id());
    assert_eq!(
        plan.fragments[0].placements[0].implementation_id.as_str(),
        "std/kernel-pure-expression@1"
    );
    (host, plan, program)
}
fn packet(program: &PortableExpressionProgram, y: &str, x: &str) -> Vec<u8> {
    let StructuredInfoTypeShape::Record { fields, .. } = program.input_type.shape() else {
        panic!("record input")
    };
    let values = fields
        .iter()
        .map(|f| {
            let value = Quantity::parse_plot_literal(if f.name() == "y" {
                y
            } else {
                assert_eq!(f.name(), "x");
                x
            })
            .unwrap();
            StructuredFieldValue::new(
                f.name(),
                StructuredInfoValue::leaf(f.value_type().clone(), value.encode().to_vec()).unwrap(),
            )
            .unwrap()
        })
        .collect();
    StructuredInfoValue::record(program.input_type.clone(), values)
        .unwrap()
        .canonical_bytes()
        .unwrap()
}
#[test]
fn installed_difference_roles_keep_exact_turn_axes_and_quadrants_without_kernel_allocation() {
    let (mut host, plan, program) = prepared();
    for (y, x, n, d) in [(9, 5, 1, 8), (-9, 5, -1, 8), (9, 0, 1, 4), (0, -5, 1, 2)] {
        let y = format!("TemperatureDelta({y}, °F)");
        let x = format!("TemperatureDelta({x}, K)");
        let input = packet(&program, &y, &x);
        let mut outputs = Outputs(Vec::with_capacity(1));
        let report = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: input,
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap();
        assert_eq!(outputs.0.len(), 1);
        assert_eq!(outputs.0[0].front_port_id.as_str(), "result");
        let angle =
            MathematicalAngle::decode(&outputs.0[0].bytes, &mut ExactNumericWork::new(100_000_000))
                .unwrap();
        assert_eq!(angle.unit(), Unit::Turn);
        assert_eq!(
            angle.coordinate().as_rational(),
            Some(ExactRational::ratio(n, d, &mut ExactNumericWork::new(100_000_000)).unwrap())
        );
        let kernel = report.kernel.as_ref().expect("real kernel Play");
        assert_eq!(kernel.post_play_start_allocations, 0);
        assert_eq!(
            kernel.value_allocation_capacity_before,
            kernel.value_allocation_capacity_after
        );
        println!(
            "ATAN2_DELTA source={} checked={} expanded={} plan={} y={y} x={x} result={n}/{d} report={report:?}",
            plan.source_document_id.as_str(),
            plan.checked_plot_id.as_str(),
            plan.expanded_plot_id.as_str(),
            plan.plan_id.as_str()
        );
    }
}
#[test]
fn installed_point_mixed_and_zero_inputs_refuse_without_output_then_valid_input_recovers() {
    let (mut host, plan, program) = prepared();
    for (y, x, detail) in [
        ("21°C", "22°C", 21),
        ("TemperatureDelta(1, K)", "21°C", 21),
        ("TemperatureDelta(0, °F)", "TemperatureDelta(0, K)", 22),
        ("3m", "4s", 20),
    ] {
        let mut outputs = Outputs(Vec::with_capacity(1));
        let refusal = host
            .run_external_plot_to(
                plan.fragments[0].clone(),
                &[ExternalForeInput {
                    front_port_id: "value".into(),
                    track: ConnectionTrack::Payload,
                    bytes: packet(&program, y, x),
                }],
                &mut outputs,
                &mut Vec::new(),
                &mut NoTimer,
            )
            .unwrap_err();
        assert!(outputs.0.is_empty());
        assert_eq!(refusal, format!("installed kernel step: BackFailed(Failure {{ code: HostCallFailed, detail: {detail} }})"));
        println!("ATAN2_ROLE_REFUSAL {y}, {x}: {refusal}");
    }
    let mut outputs = Outputs(Vec::with_capacity(1));
    let report = host
        .run_external_plot_to(
            plan.fragments[0].clone(),
            &[ExternalForeInput {
                front_port_id: "value".into(),
                track: ConnectionTrack::Payload,
                bytes: packet(
                    &program,
                    "TemperatureDelta(9, °F)",
                    "TemperatureDelta(5, K)",
                ),
            }],
            &mut outputs,
            &mut Vec::new(),
            &mut NoTimer,
        )
        .unwrap();
    assert_eq!(outputs.0.len(), 1);
    let angle =
        MathematicalAngle::decode(&outputs.0[0].bytes, &mut ExactNumericWork::new(100_000_000))
            .unwrap();
    assert_eq!(angle.unit(), Unit::Turn);
    assert_eq!(
        angle.coordinate().as_rational(),
        Some(ExactRational::ratio(1, 8, &mut ExactNumericWork::new(100_000_000)).unwrap())
    );
    let kernel = report.kernel.as_ref().unwrap();
    assert_eq!(kernel.post_play_start_allocations, 0);
    assert_eq!(
        kernel.value_allocation_capacity_before,
        kernel.value_allocation_capacity_after
    );
}
