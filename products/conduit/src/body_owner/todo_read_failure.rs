//! Public refusal codes for the exact canonical checkpoint read Host Call.
//! Provider detail is interpreted only after the Owner admitted that read Back.
use conduit_kernel::{Failure, FailureCode};
use conduit_std_host::{
    todo_checkpoint_read_call::read_failure_detail, todo_durable_resource::Refusal,
};

pub(super) fn code(failure: &Failure) -> &'static str {
    match failure.code {
        FailureCode::Cancelled => "todo-committed-read-cancelled",
        FailureCode::HostCallDenied => "todo-committed-authority-changed",
        FailureCode::HostCallFailed => match failure.detail {
            detail if detail == read_failure_detail(&Refusal::Missing) => {
                "todo-committed-checkpoint-missing"
            }
            detail if detail == read_failure_detail(&Refusal::Inaccessible) => {
                "todo-committed-inaccessible"
            }
            detail if detail == read_failure_detail(&Refusal::Corrupt) => "todo-committed-corrupt",
            detail if detail == read_failure_detail(&Refusal::StaleRevision) => {
                "todo-committed-stale-version"
            }
            detail if detail == read_failure_detail(&Refusal::WrongAuthority) => {
                "todo-committed-authority-changed"
            }
            detail if detail == read_failure_detail(&Refusal::InvalidBinding) => {
                "todo-committed-provider-changed"
            }
            detail if detail == read_failure_detail(&Refusal::InvalidState) => {
                "todo-committed-invalid-state"
            }
            detail if detail == read_failure_detail(&Refusal::UnknownOutcome) => {
                "todo-committed-read-outcome-unknown"
            }
            detail if detail == read_failure_detail(&Refusal::Storage) => {
                "todo-committed-storage-unavailable"
            }
            detail if detail == read_failure_detail(&Refusal::MigrationRequired) => {
                "todo-committed-migration-required"
            }
            _ => "todo-committed-read-failed",
        },
        _ => "todo-committed-read-failed",
    }
}
