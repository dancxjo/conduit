//! Small checked Source proof: preparation metadata, not runtime admission.
use super::*;
use conduit_embedded_build::ResidentExecutionStorage;

const SOURCE: &str = r#"
type Number = {
    value: U64
}
plot increment (
    input: Number...| >> output: Number...|
) = {value: .value + 1}
plot transform (
    input: Number...| >> output: Number...|
) = {value: (.value * 2) + 3}
plot three (
    >> first: Number...|
    >> second: Number...|
    >> third: Number...|
    first-result: Number...| >>
    second-result: Number...| >>
    third-result: Number...| >>
) {
    a: increment
    b: increment
    c: transform
    first >> a.input
    second >> b.input
    third >> c.input
    a.output >> first-result
    b.output >> second-result
    c.output >> third-result
}
"#;

#[test]
fn repeated_resident_owners_keep_exact_unequal_work_and_original_aggregate_limit() {
    let workset = compile(
        SOURCE.as_bytes(),
        &format!("{:x}", Sha256::digest(SOURCE.as_bytes())),
        &["three".into()],
        None,
    )
    .unwrap();
    let bounds = crate::resident_expression_bounds(1000);
    let storage = ResidentExecutionStorage::SharedWorkspaceSchema;
    let prepared = workset
        .prepare_selected_residents_with_exact_work(bounds, storage, 1000)
        .unwrap();
    assert_eq!(prepared.len(), 3);
    let repeated = prepared
        .iter()
        .filter(|owner| owner.kind_id() == prepared[0].kind_id())
        .collect::<Vec<_>>();
    assert_eq!(repeated.len(), 2);
    assert_eq!(
        repeated[0].computation_work(),
        repeated[1].computation_work()
    );
    assert_ne!(repeated[0].artifact_id(), repeated[1].artifact_id());
    assert_ne!(
        repeated[0].offer_description().capability_id,
        repeated[1].offer_description().capability_id
    );
    assert_eq!(
        prepared
            .iter()
            .map(|owner| &owner.offer_description().capability_id)
            .collect::<std::collections::BTreeSet<_>>()
            .len(),
        prepared.len()
    );
    let mut total = 0u32;
    let work = prepared
        .iter()
        .map(|owner| {
            let work = u32::from(owner.computation_work());
            let requirements = &owner.offer_description().resource_requirements;
            assert_eq!(requirements.len(), 1);
            assert_eq!(
                requirements[0].class_id.as_str(),
                conduit_core::COOPERATIVE_STEP_WORK_CLASS_ID
            );
            assert_eq!(requirements[0].units, work);
            total = total.checked_add(work).unwrap();
            work
        })
        .collect::<Vec<_>>();
    assert!(work.iter().max() > work.iter().min());
    assert!((1..=1000).contains(&total));
    let exact = workset
        .prepare_selected_residents_with_exact_work(bounds, storage, total)
        .unwrap();
    assert_eq!(exact.len(), prepared.len());
    for (before, after) in prepared.iter().zip(&exact) {
        assert_eq!(before.offer_description(), after.offer_description());
    }
    let refusal = workset
        .prepare_selected_residents_with_exact_work(bounds, storage, total - 1)
        .err()
        .unwrap();
    assert!(refusal.contains("original pool"), "{refusal}");
    assert!(
        refusal.contains(&format!("requires {total} units")),
        "{refusal}"
    );
}
