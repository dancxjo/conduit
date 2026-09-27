//! Exact admission for an explicitly requested retry.
//!
//! Planning may prove that one retry is permitted. This module deliberately
//! contains no retry loop, attempt counter, backoff, or implicit replan hook.

use alloc::boxed::Box;
use conduit_core::{
    derive_transformation_eligibility, Back, Kind, RetainedRetryEvidence, RetryAuthorization,
    RetryOperationIdentity, SignId, TransformationRefusal,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RetryAdmissionBasis {
    EffectFree,
    RetainedEffect {
        supporting_sign_id: SignId,
        evidence: Box<RetainedRetryEvidence>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RetryAdmission {
    pub operation: RetryOperationIdentity,
    pub basis: RetryAdmissionBasis,
}

pub fn admit_explicit_retry(
    kind: &Kind,
    back: &Back,
    operation: &RetryOperationIdentity,
    evidence: &RetainedRetryEvidence,
) -> Result<RetryAdmission, TransformationRefusal> {
    let eligibility = derive_transformation_eligibility(kind, back)?;
    let authorization = eligibility.require_retry_with_evidence(operation, evidence)?;
    let basis = match authorization {
        RetryAuthorization::EffectFree { .. } => RetryAdmissionBasis::EffectFree,
        RetryAuthorization::RetainedEffect { evidence, .. } => {
            RetryAdmissionBasis::RetainedEffect {
                supporting_sign_id: evidence.supporting_sign.sign_id.clone(),
                evidence: Box::new(evidence.clone()),
            }
        }
    };
    Ok(RetryAdmission {
        operation: operation.clone(),
        basis,
    })
}
