use super::*;
use conduit_core::{CheckedFormId, ExpandedFormId, PlannedStateBoundary, SourceDocumentId};

fn admitted(bytes: u32, continuation: StateContinuation) -> AdmittedStateGraph {
    let states = vec![PlannedStateBoundary {
        state_id: StateId::from("retained"),
        gear_id: GearId::from("cell"),
        value_kind: KindId::from("fixture/bounded-bytes@1"),
        initial_value: vec![0],
        maximum_value_bytes: bytes,
        continuation,
    }];
    AdmittedStateGraph {
        form_identity: FormIdentity {
            source_document_id: SourceDocumentId::from("fixture-source"),
            checked_form_id: CheckedFormId::from("fixture-checked"),
            expanded_form_id: ExpandedFormId::from("fixture-expanded"),
        },
        startup_order: vec![GearId::from("cell")],
        resources: state_resource_budget(&states).unwrap(),
        states,
    }
}

#[test]
fn raw_byte_domain_count_includes_empty_and_absent_candidates() {
    let graph = admitted(1, StateContinuation::MaximumTransitions(5));
    let report = graph.analyze_value_storage(66_306).unwrap();
    assert_eq!(report.form_identity, graph.form_identity);
    assert_eq!(
        report.enumeration,
        RepresentationEnumeration::WithinBudget {
            representations: 257 * 258
        }
    );
    assert_eq!(report.resources.retained_value_bytes, 2);
    assert_eq!(report.domains[0].state_id, graph.states[0].state_id);
    assert_eq!(report.domains[0].value_kind, graph.states[0].value_kind);
    assert_eq!(
        graph.analyze_value_storage(66_305).unwrap().enumeration,
        RepresentationEnumeration::ExceedsBudget {
            maximum_representations: 66_305
        }
    );
}

#[test]
fn huge_finite_capacity_is_retained_without_materializing_or_enumerating_it() {
    let graph = admitted(u32::MAX, StateContinuation::ExternallyBounded);
    let report = graph.analyze_value_storage(u64::MAX).unwrap();
    assert_eq!(report.domains[0].maximum_value_bytes, u32::MAX);
    assert_eq!(
        report.resources.retained_value_bytes,
        u64::from(u32::MAX) * 2
    );
    assert_eq!(
        report.enumeration,
        RepresentationEnumeration::ExceedsBudget {
            maximum_representations: u64::MAX
        }
    );
    assert_eq!(graph.states[0].initial_value, [0]);
}

#[test]
fn continuation_and_fuel_do_not_change_retained_value_cardinality() {
    let finite = admitted(1, StateContinuation::MaximumTransitions(1));
    let continuous = admitted(1, StateContinuation::ExternallyBounded);
    let a = finite.analyze_value_storage(100_000).unwrap();
    let b = continuous.analyze_value_storage(100_000).unwrap();
    assert_eq!(a.enumeration, b.enumeration);
    assert_ne!(a.domains[0].continuation, b.domains[0].continuation);
}

#[test]
fn invalid_mutated_bound_is_not_reported_as_a_finite_proof() {
    let mut graph = admitted(1, StateContinuation::ExternallyBounded);
    graph.states[0].maximum_value_bytes = 0;
    assert_eq!(
        graph.analyze_value_storage(100),
        Err(StatePlanError::ZeroValueBound)
    );
}

#[test]
fn composition_preserves_each_domain_and_multiplies_raw_representations() {
    let mut graph = admitted(1, StateContinuation::ExternallyBounded);
    let mut second = graph.states[0].clone();
    second.state_id = StateId::from("other-state");
    second.gear_id = GearId::from("other-gear");
    graph.states.push(second);
    let report = graph.analyze_value_storage(u64::MAX).unwrap();
    assert_eq!(report.domains.len(), 2);
    assert_eq!(report.resources.retained_value_bytes, 4);
    assert_eq!(
        report.enumeration,
        RepresentationEnumeration::WithinBudget {
            representations: 66_306u64.pow(2)
        }
    );
    assert_eq!(
        graph.analyze_value_storage(0).unwrap().enumeration,
        RepresentationEnumeration::ExceedsBudget {
            maximum_representations: 0
        }
    );
}

#[test]
fn analysis_retains_the_identity_validated_by_ordinary_graph_admission() {
    let mut form = conduit_form::parse_with_startup(
        "form signal-demo {\n pulse: flow/pulse(count = 2, period-ms = 0, initial = false)\n show: presentation/show\n pulse > show\n}\n",
        &conduit_signal::signal_startup_catalog(),
        &conduit_signal::signal_profile_catalog(),
    ).unwrap();
    let graph = crate::state_delay::admit_state_graph(&form, vec![]).unwrap();
    let report = graph.analyze_value_storage(1).unwrap();
    assert_eq!(report.form_identity, form.identity());
    assert!(report.domains.is_empty());
    assert_eq!(
        report.enumeration,
        RepresentationEnumeration::WithinBudget { representations: 1 }
    );
    form.checked_form_id = CheckedFormId::from("forged");
    assert_eq!(
        crate::state_delay::admit_state_graph(&form, vec![]),
        Err(crate::state_delay::StateGraphError::InvalidForm)
    );
}
