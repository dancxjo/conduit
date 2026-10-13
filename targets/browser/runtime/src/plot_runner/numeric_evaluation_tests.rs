//! Actual checked source → selected numerical offer → Plan → browser kernel.
use super::*;
fn requirement(representation: &str) -> String {
    format!(
        r#"{{ semantics: mathematical(empty), representation: {representation}(empty),
        rounding: nearestEven(empty), accuracy: exact(empty),
        budget: {{ storageBytes: 1048576, workSteps: 100000, limbs: 128, symbolicNodes: 128 }},
        parameters: [], reproducible: true,
        exceptions: {{ invalid:false, divisionByZero:false, overflow:false, underflow:false, inexact:false }} }}"#
    )
}
#[test]
fn checked_numeric_computations_execute_selected_reviewed_back() {
    for (expression, representation) in [
        ("0.1 + 0.2 == 0.3", "rational"),
        ("41 + 1 == 42", "checkedInteger"),
        ("math/sin(90°) == 1", "symbolic"),
    ] {
        let source=format!("plot numeric {{\n operation: math/evaluate(value = {expression}, requirement = {})\n show: presentation/bool-value\n operation.result >> show.value\n}}.",requirement(representation));
        let (mut session, effect) =
            TourSession::prepare("browser/numeric-proof", "boot/numeric-proof", &source, 1)
                .unwrap_or_else(|error| panic!("{error}\n{source}"));
        let TourHostEffect::Manifestation(effect) = effect else {
            panic!("Boolean manifestation")
        };
        assert_eq!(effect.text.as_deref(), Some("true"));
        let placement = session.fragments[0]
            .placements
            .iter()
            .find(|placement| {
                placement.implementation_id.as_str()
                    == crate::installed_browser::numeric_evaluation::IMPLEMENTATION
            })
            .unwrap();
        assert_eq!(
            placement.artifact_id,
            conduit_core::numeric_realization::numeric_engine_artifact_id()
        );
        assert!(placement
            .realization_properties
            .iter()
            .any(|property| property.profile()
                == &conduit_core::numeric_realization_property_profile()));
        let capacities = session.scheduler.values().allocation_capacities();
        let TourProgress::Receipt(receipt) = session.advance().unwrap() else {
            panic!("completed receipt")
        };
        assert_eq!(receipt.disposition, "completed");
        assert_eq!(
            session.scheduler.values().allocation_capacities(),
            capacities
        );
    }
}
