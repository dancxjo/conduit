//! Acquired indicator resources beneath ordinary admitted Host operations.
//! These identities describe cooperative provider binding, not hostile-code confinement.
use conduit_core::{ActivePlayIdentity, BootId, HostId, InfoBool, OfferGeneration, ResourcePoolId};
use conduit_kernel::RequestId;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct IndicatorBinding {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub pool_id: ResourcePoolId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
#[repr(u16)]
pub enum IndicatorFailure {
    Lost = 1,
    Timeout = 2,
    StaleIdentity = 3,
    MalformedReceipt = 4,
    WrongState = 5,
    Cancelled = 6,
    InvalidInput = 7,
}

pub struct IndicatorRequest<'a> {
    pub play: &'a ActivePlayIdentity,
    pub request: RequestId,
    pub state: InfoBool,
}

/// The provider owns an already acquired, exact resource. It must bound its
/// effect/acknowledgment work and return success only after the requested state
/// is acknowledged. Device/protocol evidence belongs to provider inspection,
/// never to the portable Boolean. No retry or replan is implied.
pub trait HostedIndicatorAdapter: Send {
    fn binding(&self) -> &IndicatorBinding;
    fn present(&mut self, request: IndicatorRequest<'_>) -> Result<(), IndicatorFailure>;
}
