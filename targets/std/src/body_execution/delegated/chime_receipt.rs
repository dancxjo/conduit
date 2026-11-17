//! Finite wire representation of actual cue-call facts, never a grant.
use super::*;
use conduit_kernel::{Failure, FailureCode, HostCallDisposition, NodeId, RequestId};
#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum Disposition {
    Completed,
    Denied,
    Cancelled,
    Failed,
}
#[derive(Serialize, Deserialize)]
struct Wire {
    node: u16,
    request: Option<u32>,
    disposition: Option<Disposition>,
    failure: Option<DelegatedKernelFailure>,
    kernel_accepted: bool,
}
impl Serialize for StartupChimeCallReceipt {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        Wire {
            node: self.node.0,
            request: self.request.map(|r| r.0),
            disposition: self.disposition.map(|d| match d {
                HostCallDisposition::Completed => Disposition::Completed,
                HostCallDisposition::Denied => Disposition::Denied,
                HostCallDisposition::Cancelled => Disposition::Cancelled,
                HostCallDisposition::Failed => Disposition::Failed,
            }),
            failure: self.failure.map(Into::into),
            kernel_accepted: self.kernel_accepted,
        }
        .serialize(serializer)
    }
}
impl<'de> Deserialize<'de> for StartupChimeCallReceipt {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let value = Wire::deserialize(deserializer)?;
        Ok(Self {
            node: NodeId(value.node),
            request: value.request.map(RequestId),
            disposition: value.disposition.map(|d| match d {
                Disposition::Completed => HostCallDisposition::Completed,
                Disposition::Denied => HostCallDisposition::Denied,
                Disposition::Cancelled => HostCallDisposition::Cancelled,
                Disposition::Failed => HostCallDisposition::Failed,
            }),
            failure: value.failure.map(|f| Failure {
                detail: f.detail,
                code: match f.code {
                    DelegatedKernelFailureCode::InvalidInput => FailureCode::InvalidInput,
                    DelegatedKernelFailureCode::InvalidPort => FailureCode::InvalidPort,
                    DelegatedKernelFailureCode::InvalidLifecycle => FailureCode::InvalidLifecycle,
                    DelegatedKernelFailureCode::StorageExhausted => FailureCode::StorageExhausted,
                    DelegatedKernelFailureCode::StateCapacityExhausted => {
                        FailureCode::StateCapacityExhausted
                    }
                    DelegatedKernelFailureCode::WorkBudgetExhausted => {
                        FailureCode::WorkBudgetExhausted
                    }
                    DelegatedKernelFailureCode::IdentityCapacityExhausted => {
                        FailureCode::IdentityCapacityExhausted
                    }
                    DelegatedKernelFailureCode::HostCallDenied => FailureCode::HostCallDenied,
                    DelegatedKernelFailureCode::HostCallFailed => FailureCode::HostCallFailed,
                    DelegatedKernelFailureCode::Cancelled => FailureCode::Cancelled,
                },
            }),
            kernel_accepted: value.kernel_accepted,
        })
    }
}
