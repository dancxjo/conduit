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
