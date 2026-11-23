//! Retained custody of one complete, IMAGE-funded Body set on one exact member.
//! Receipts report facts over an independently authenticated owner channel;
//! deserializing a receipt cannot construct a prepared ticket or commit witness.
use super::*;
use crate::installed_std::body_kernel::BodyExecutionState;
use crate::kernel_preparation::{ImageMemoryLease, KernelResourceReservation};
use conduit_core::{ActivePlayIdentity, BootId, HostId, OfferGeneration, PlanId};
use serde::{Deserialize, Serialize};
mod chime_receipt;
mod operations;
mod owner_acknowledgment;
mod preparation;
#[cfg(test)]
mod tests;

pub struct DelegatedBodyRequest<'a> {
    pub wake: &'a Wake,
    pub plan: &'a BodyPlan,
    pub global_play: &'a BodyPlayIdentity,
    pub control: &'a RunControl,
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedBodyPreparationReceipt {
    pub preparation_id: String,
    pub global_play: BodyPlayIdentity,
    pub member_play: ActivePlayIdentity,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub plan_id: PlanId,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DelegatedKernelFailureCode {
    InvalidInput,
    InvalidPort,
    InvalidLifecycle,
    StorageExhausted,
    StateCapacityExhausted,
    WorkBudgetExhausted,
    IdentityCapacityExhausted,
    HostCallDenied,
    HostCallFailed,
    Cancelled,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedKernelFailure {
    pub code: DelegatedKernelFailureCode,
    pub detail: u16,
}
impl From<conduit_kernel::Failure> for DelegatedKernelFailure {
    fn from(value: conduit_kernel::Failure) -> Self {
        use conduit_kernel::FailureCode as F;
        use DelegatedKernelFailureCode as D;
        Self {
            detail: value.detail,
            code: match value.code {
                F::InvalidInput => D::InvalidInput,
                F::InvalidPort => D::InvalidPort,
                F::InvalidLifecycle => D::InvalidLifecycle,
                F::StorageExhausted => D::StorageExhausted,
                F::StateCapacityExhausted => D::StateCapacityExhausted,
                F::WorkBudgetExhausted => D::WorkBudgetExhausted,
                F::IdentityCapacityExhausted => D::IdentityCapacityExhausted,
                F::HostCallDenied => D::HostCallDenied,
                F::HostCallFailed => D::HostCallFailed,
                F::Cancelled => D::Cancelled,
            },
        }
    }
}
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct DelegatedBodyTerminalReceipt {
    pub preparation_id: String,
    pub global_play: BodyPlayIdentity,
    pub member_play: ActivePlayIdentity,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub plan_id: PlanId,
    pub terminal: TerminalDisposition,
    pub kernel_failure: Option<DelegatedKernelFailure>,
    pub failure: Option<String>,
    pub cleanup_failure: Option<String>,
    pub kernel_signs: usize,
    pub startup_chimes: Vec<StartupChimeCallReceipt>,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DelegatedBodyProgress {
    Prepared,
    Running,
    Terminal,
}

/// This value retains actual kernel and original counter custody across requests.
/// Dropping a ticket does not return its Host's resource reservations. The actor
/// must retain it through unknown outcomes and explicitly release after terminal.
#[must_use]
pub struct PreparedDelegatedBody {
    receipt: DelegatedBodyPreparationReceipt,
    terminal: DelegatedBodyTerminalReceipt,
    scope: super::startup::PreparedStartupScope,
    kernel: Option<BodyKernel<'static>>,
    execution: Option<BodyExecutionState<'static, 'static>>,
    reservations: Vec<KernelResourceReservation>,
    memory: ImageMemoryLease,
    committed: bool,
    finished: bool,
    cancelled: bool,
    released: bool,
}
impl PreparedDelegatedBody {
    pub fn receipt(&self) -> &DelegatedBodyPreparationReceipt {
        &self.receipt
    }
    pub fn terminal_receipt(&self) -> Option<&DelegatedBodyTerminalReceipt> {
        self.finished.then_some(&self.terminal)
    }
}

/// Allocation-free exact receipt backing before any copied identity is built.
/// Terminal and preparation receipts coexist; diagnostic strings are moved from
/// the already measured kernel result, never cloned into a second result.
pub(crate) fn preparation_backing_bytes(
    plan: &BodyPlan,
    global: &BodyPlayIdentity,
    host: &conduit_core::HostAdvertisement,
) -> Result<usize, String> {
    let mut bytes = core::mem::size_of::<PreparedDelegatedBody>();
    let lengths = [
        64,
        global.active_play_id.as_str().len(),
        global.body_id.as_str().len(),
        global.wake_id.as_str().len(),
        global.plan_id.as_str().len(),
        64,
        plan.plan_id.as_str().len(),
        host.host_id.as_str().len(),
        host.boot_id.as_str().len(),
        host.host_id.as_str().len(),
        host.boot_id.as_str().len(),
        plan.plan_id.as_str().len(),
    ];
    for length in lengths {
        bytes = bytes
            .checked_add(
                length
                    .checked_mul(2)
                    .ok_or("delegated receipt backing overflow")?,
            )
            .ok_or("delegated receipt backing overflow")?;
    }
    Ok(bytes)
}
