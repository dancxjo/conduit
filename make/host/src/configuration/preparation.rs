//! A selected delegation is finite intent, not an additional resource budget.
use crate::{ConfigurationDiagnostic as D, HostConfiguration};

pub(super) fn check(configuration: &HostConfiguration, diagnostics: &mut Vec<D>) {
    if let Some(execution) = configuration.execution {
        if configuration.preparation.is_none() || execution.maximum_active_worksets == 0 {
            diagnostics.push(D::PreparationDelegation {
                detail:
                    "execution requires selected preparation and positive active-workset ceiling"
                        .into(),
            });
        }
        if u64::from(execution.maximum_active_worksets)
            > u64::from(configuration.limits.active_instances)
        {
            diagnostics.push(D::LimitExceeded {
                field: "maximum_active_worksets",
                requested: u64::from(execution.maximum_active_worksets),
                maximum: u64::from(configuration.limits.active_instances),
            });
        }
    }
    if let Some(execution) = configuration.execution {
        if let Some(continuation) = execution.continuation {
            if !continuation.has_finite_bounds() {
                diagnostics.push(D::PreparationDelegation {
                    detail: "continuation ceilings must all be positive".into(),
                });
            }
            // Policy ceilings confer no physical storage or native possession.
            // The selected provider and current admission must fund those too.
            for (field, requested, maximum) in [
                (
                    "continuation.assignment_bytes",
                    u64::from(continuation.assignment_bytes),
                    configuration
                        .preparation
                        .map_or(0, |p| u64::from(p.maximum_assignment_bytes)),
                ),
                (
                    "continuation.runtime_bytes",
                    u64::from(continuation.runtime_bytes),
                    configuration.limits.heap_arena_bytes,
                ),
                (
                    "continuation.operation_slots",
                    u64::from(continuation.operation_slots),
                    u64::from(configuration.limits.operation_slots),
                ),
                (
                    "continuation.active_worksets",
                    u64::from(continuation.active_worksets),
                    u64::from(execution.maximum_active_worksets),
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
    }
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
