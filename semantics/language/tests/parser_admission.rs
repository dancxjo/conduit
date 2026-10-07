use conduit_core::*;
use conduit_language as admitted;
use conduit_plot::rust_binding::NativeRustBinding;
#[path = "common/parser_fixture.rs"]
mod fixture;
use fixture::*;

fn retype(ty: &StructuredInfoType, value: &StructuredInfoValue) -> StructuredInfoValue {
    match (ty.shape(), value.shape()) {
        (
            StructuredInfoTypeShape::Record { fields, .. },
            StructuredInfoValueShape::Record(values),
        ) => record(
            ty,
            fields
                .iter()
                .map(|f| {
                    (
                        f.name(),
                        retype(
                            f.value_type(),
                            values
                                .iter()
                                .find(|v| v.name() == f.name())
                                .unwrap()
                                .value(),
                        ),
                    )
                })
                .collect(),
        ),
        _ => value.clone(),
    }
}
fn beam(f: &mut Fixture) -> StructuredInfoValue {
    let state = f.initial(4);
    let candidate_type = f.ty("LanguageParserHypothesis");
    let candidate = record(
        candidate_type,
        vec![
            ("state", retype(field_type(candidate_type, "state"), &state)),
            (
                "identity",
                number(field_type(candidate_type, "identity"), 1),
            ),
            (
                "score",
                StructuredInfoValue::leaf(
                    field_type(candidate_type, "score").clone(),
                    100_i64.to_le_bytes().to_vec(),
                )
                .unwrap(),
            ),
            (
                "active",
                StructuredInfoValue::leaf(field_type(candidate_type, "active").clone(), vec![1])
                    .unwrap(),
            ),
        ],
    );
    record(
        f.ty("LanguageParserAdmittedBeam"),
        vec![
            ("basis", f.basis("analysis/1")),
            ("candidate0", candidate.clone()),
            ("candidate1", candidate.clone()),
            ("candidate2", candidate.clone()),
            ("candidate3", candidate),
        ],
    )
}
#[test]
fn recursive_native_admission_refuses_forged_graph_and_foreign_basis() {
    let mut f = Fixture::new();
    let good = beam(&mut f);
    assert!(admitted::LanguageParserAdmittedBeam::from_structured(good.clone()).is_ok());
    let foreign = replace(&good, "basis", f.basis("analysis/foreign"));
    assert!(admitted::LanguageParserAdmittedBeam::from_structured(foreign).is_err());
    for name in ["candidate0", "candidate1", "candidate2", "candidate3"] {
        let candidate = field(&good, name);
        let state = field(candidate, "state");
        let invalid = replace(
            state,
            "committed",
            number(field_type(state.value_type(), "committed"), 1),
        );
        let forged = replace(&good, name, replace(candidate, "state", invalid));
        assert!(
            admitted::LanguageParserAdmittedBeam::from_structured(forged).is_err(),
            "{name}"
        );
    }
}

#[path = "common/parser_kernel.rs"]
mod parser_kernel;
#[test]
fn admitted_survivors_drive_the_source_agreement_graph() {
    let mut f = Fixture::new();
    let mut good = beam(&mut f);
    let initial = f.initial(4);
    let result = f.step(&initial, "right_arc", "root", "analysis/1");
    let root = field(&result, "state");
    for name in ["candidate0", "candidate1", "candidate2", "candidate3"] {
        let candidate = field(&good, name);
        let state = retype(field_type(candidate.value_type(), "state"), root);
        let candidate = replace(candidate, "state", state);
        good = replace(&good, name, candidate);
    }
    let ty = f.ty("LanguageParserAdmittedAgreementQuery");
    let request = record(
        ty,
        vec![
            ("beam", good),
            ("dependent", number(field_type(ty, "dependent"), 0)),
        ],
    );
    let admitted =
        admitted::LanguageParserAdmittedAgreementQuery::from_structured(request).unwrap();
    let source = format!(
        "{}\n{}\n{}\n{}",
        include_str!("../identity.conduit"),
        include_str!("../types.conduit"),
        include_str!("../parser.conduit"),
        include_str!("../parser_beam.conduit")
    );
    let mut run = parser_kernel::Execution::prepare(source, "language-parser-admitted-agreement");
    let boundary = &run.kernel.definition().boundary;
    let input_port = boundary.input_fronts[0].external_port.clone();
    let output_port = boundary.output_fronts[0].external_port.clone();
    let input = ValuePayload {
        value_kind: input_port.value_kind,
        encoded: admitted
            .into_structured()
            .unwrap()
            .canonical_bytes()
            .unwrap(),
    };
    let mut output = ValuePayload {
        value_kind: output_port.value_kind,
        encoded: Vec::with_capacity(MAXIMUM_STRUCTURED_CANONICAL_BYTES),
    };
    run.kernel.start().unwrap();
    assert!(matches!(
        run.kernel
            .admit_input(&input_port.port_id, 0, &input)
            .unwrap(),
        conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { .. }
    ));
    run.kernel.close_input(&input_port.port_id).unwrap();
    let agreement = (0..4000)
        .find_map(|_| {
            run.step();
            run.kernel
                .output_into(&output_port.port_id, &mut output)
                .unwrap()
                .map(|_| StructuredInfoValue::from_canonical_bytes(&output.encoded).unwrap())
        })
        .expect("bounded source agreement output");
    assert_eq!(count(field(&agreement, "survivors")), 4);
    assert_eq!(
        field(&agreement, "agrees").shape(),
        StructuredInfoValueShape::Leaf(&[1])
    );
    assert_eq!(count(field(field(&agreement, "reference"), "head")), 4);
}
