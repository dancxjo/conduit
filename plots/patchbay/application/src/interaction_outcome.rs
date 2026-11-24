//! Kernel disposition for one canonical owner interaction.
use crate::{PatchbayApplicationRequest, PatchbayCommandRefusal};
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, HostCallOutcome};

/// Correlate transport completion with the actual owner's result. The
/// installing owner separately retains any returned typed request.
pub fn patchbay_interaction_outcome(
    result: &Result<Option<PatchbayApplicationRequest>, PatchbayCommandRefusal>,
) -> HostCallOutcome {
    let (disposition, failure) = match result {
        Ok(_) => (HostCallDisposition::Completed, None),
        Err(PatchbayCommandRefusal::WrongOwner | PatchbayCommandRefusal::InvalidContribution) => (
            HostCallDisposition::Denied,
            Some(Failure {
                code: FailureCode::HostCallDenied,
                detail: 71,
            }),
        ),
        Err(PatchbayCommandRefusal::Domain(
            crate::PatchbayApplicationRefusal::RevisionExhausted,
        )) => (
            HostCallDisposition::Failed,
            Some(Failure {
                code: FailureCode::IdentityCapacityExhausted,
                detail: 71,
            }),
        ),
        Err(_) => (
            HostCallDisposition::Failed,
            Some(Failure {
                code: FailureCode::InvalidInput,
                detail: 71,
            }),
        ),
    };
    HostCallOutcome {
        disposition,
        output: None,
        failure,
    }
}
