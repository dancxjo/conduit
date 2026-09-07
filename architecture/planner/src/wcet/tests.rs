use super::*;

fn facts(id: &str, capacity: u64, operations: Option<u64>, wcet_us: Option<u32>) -> TimingFacts {
    TimingFacts {
        realization_id: id.into(),
        finite_capacity: capacity,
        resource_units: 1,
        maximum_operations: operations,
        wcet_us,
        basis_id: wcet_us.map(|_| format!("basis/{id}")),
    }
}

fn dependency(id: &str, timing: TimingFacts) -> TimingDependency {
    TimingDependency {
        dependency_id: id.into(),
        facts: timing,
    }
}

fn region(dependencies: Vec<TimingDependency>, deadline_us: u32) -> DeadlineRegion {
    DeadlineRegion {
        region_id: "motor-step".into(),
        deadline_us,
        maximum_resource_units: 4,
        dependencies,
    }
}

#[test]
fn finite_capacity_does_not_imply_wcet() {
    let error = admit_deadline_region(&region(
        vec![dependency(
            "parser",
            facts("parser", 16 * 1024 * 1024, Some(1_000_000), None),
        )],
        50,
    ))
    .unwrap_err();
    assert_eq!(
        error,
        WcetRefusal::MissingWcet {
            dependency_id: "parser".into()
        }
    );
}

#[test]
fn bounded_dependencies_compose_with_exact_basis() {
    let admission = admit_deadline_region(&region(
        vec![
            dependency("sample", facts("sample", 8, Some(12), Some(20))),
            dependency("control", facts("control", 4, Some(7), Some(30))),
        ],
        50,
    ))
    .unwrap();
    assert_eq!(admission.total_wcet_us, 50);
    assert_eq!(admission.total_resource_units, 2);
    assert_eq!(admission.basis_ids, ["basis/sample", "basis/control"]);
}

#[test]
fn large_finite_work_remains_valid_outside_deadline_region() {
    let work = facts("compiler", u64::MAX, Some(u64::MAX), None);
    assert!(work.finite_capacity > 0);
    assert_eq!(work.maximum_operations, Some(u64::MAX));
    assert_eq!(
        admit_deadline_region(&region(vec![dependency("compiler", work)], u32::MAX)),
        Err(WcetRefusal::MissingWcet {
            dependency_id: "compiler".into()
        })
    );
}

#[test]
fn replan_rejects_a_timing_regression_and_preserves_admission() {
    let original = region(
        vec![dependency("controller", facts("v1", 8, Some(10), Some(40)))],
        100,
    );
    let admission = admit_deadline_region(&original).unwrap();
    let replacement = region(
        vec![dependency(
            "controller",
            facts("v2", 8, Some(10), Some(101)),
        )],
        100,
    );
    assert_eq!(
        validate_replan(&admission, &replacement),
        Err(WcetRefusal::DeadlineExceeded {
            required_us: 101,
            deadline_us: 100
        })
    );
    assert_eq!(admission.total_wcet_us, 40);
    assert_eq!(admission.basis_ids, ["basis/v1"]);
}

#[test]
fn replan_must_keep_region_identity_and_deadline() {
    let admission = admit_deadline_region(&region(
        vec![dependency("controller", facts("v1", 8, Some(10), Some(40)))],
        100,
    ))
    .unwrap();
    let mut replacement = region(
        vec![dependency("controller", facts("v2", 8, Some(10), Some(40)))],
        100,
    );
    replacement.region_id = "other-region".into();
    assert_eq!(
        validate_replan(&admission, &replacement),
        Err(WcetRefusal::ReplanViolatesAdmission)
    );
}

#[test]
fn replan_must_keep_the_admitted_resource_ceiling() {
    let admission = admit_deadline_region(&region(
        vec![dependency("controller", facts("v1", 8, Some(10), Some(40)))],
        100,
    ))
    .unwrap();
    let mut replacement = region(
        vec![dependency("controller", facts("v2", 8, Some(10), Some(40)))],
        100,
    );
    replacement.maximum_resource_units = 5;
    assert_eq!(
        validate_replan(&admission, &replacement),
        Err(WcetRefusal::ReplanViolatesAdmission)
    );
}
