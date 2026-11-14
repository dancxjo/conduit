//! Focused compiled resident expression proof.
use super::*;

#[test]
fn compiler_work_matches_runtime_for_each_paths_and_retained_integer_contracts() {
    let d = checked("type Byte = U8 in 1..=7\ntype Samples = {\n values: collection Byte = 2\n}\nplot copy (\n input: Samples...| >> output: Samples...|\n) = {values: .values}\n");
    let p = program(&d, "copy");
    let sequence = member(&p.input_type, "values");
    let StructuredInfoTypeShape::Collection { element, .. } = sequence.shape() else {
        panic!("collection")
    };
    let StructuredInfoTypeShape::Nominal { representation, .. } = element.shape() else {
        panic!("retained nominal integer contract")
    };
    let values = [[0, 1], [1, 7], [7, 8], [2, 3]]
        .into_iter()
        .map(|bytes| {
            let elements = bytes
                .into_iter()
                .map(|byte| {
                    StructuredInfoValue::nominal(
                        element.clone(),
                        leaf(representation.clone(), byte),
                    )
                    .unwrap()
                })
                .collect();
            record(
                &p.input_type,
                vec![(
                    "values",
                    StructuredInfoValue::collection(sequence.clone(), elements).unwrap(),
                )],
            )
        })
        .collect();
    corpus(&d, &p, values, "structured-each-contracted-work");
}
