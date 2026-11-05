//! A selected delegation is finite intent, not an additional resource budget.
use crate::{ConfigurationDiagnostic as D, HostConfiguration};

pub(super) fn check(configuration: &HostConfiguration, diagnostics: &mut Vec<D>) {
    let Some(policy) = configuration.preparation else {
        return;
    };
    if !policy.has_finite_bounds() {
        diagnostics.push(D::PreparationDelegation {
            detail: "preparation byte ceilings must be positive".into(),
        });
    }
    for (field, requested, maximum) in [
        (
            "maximum_assignment_bytes",
            u64::from(policy.maximum_assignment_bytes),
            configuration.limits.buffered_bytes,
        ),
        (
            "maximum_runtime_state_bytes",
            u64::from(policy.maximum_runtime_state_bytes),
            configuration.limits.static_memory_bytes,
        ),
    ] {
        if requested > maximum {
            diagnostics.push(D::LimitExceeded {
                field,
                requested,
                maximum,
            });
        }
    }
}
