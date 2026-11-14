//! Carry checked policy intent into a Rust Root without a runtime config parser.
use crate::{CheckedHostConfiguration, HostPreparationAuthorizer};

/// An expression of `Option<conduit_body::HostPreparationDelegation>`.
/// No untrusted source text, issuer key, grant or request is interpolated.
pub fn rust_preparation_delegation(checked: &CheckedHostConfiguration) -> String {
    let Some(policy) = checked.configuration().preparation else {
        return "None".into();
    };
    match policy.authorizer {
        HostPreparationAuthorizer::SeedOwner => format!(
            "Some(conduit_body::HostPreparationDelegation{{authorizer:conduit_body::HostPreparationAuthorizer::SeedOwner,maximum_assignment_bytes:{},maximum_runtime_state_bytes:{}}})",
            policy.maximum_assignment_bytes, policy.maximum_runtime_state_bytes
        ),
    }
}

/// Explicit execution intent only. Absence emits no default permission.
pub fn rust_execution_delegation(checked: &CheckedHostConfiguration) -> String {
    let Some(policy) = checked.configuration().execution else {
        return "None".into();
    };
    let continuation = match policy.continuation {
        None => "None".into(),
        Some(c) => format!("Some(conduit_body::ContinuationCeilings{{assignment_bytes:{},runtime_bytes:{},durable_bytes:{},operation_slots:{},active_worksets:{}}})", c.assignment_bytes,c.runtime_bytes,c.durable_bytes,c.operation_slots,c.active_worksets),
    };
    match policy.authorizer {
        conduit_body::HostExecutionAuthorizer::SeedOwner => format!(
            "Some(conduit_body::HostExecutionDelegation{{authorizer:conduit_body::HostExecutionAuthorizer::SeedOwner,maximum_active_worksets:{},continuation:{}}})",
            policy.maximum_active_worksets, continuation
        ),
    }
}
