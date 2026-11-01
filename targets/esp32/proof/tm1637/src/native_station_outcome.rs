//! Observations retained by one exact native Play, never transport authority.
use conduit_net::StationAttemptFailure;

#[derive(Clone, Copy, Default)]
pub(super) enum StationOutcome {
    #[default]
    Unobserved,
    Attempt {
        result: Result<(), StationAttemptFailure>,
        at: u64,
    },
    Detached {
        lost_at: u64,
    },
}
// Preserve the previous retained-state budget on both MCU and hosted targets.
const _: () = assert!(
    core::mem::size_of::<StationOutcome>()
        <= core::mem::size_of::<Option<(Result<(), StationAttemptFailure>, u64)>>()
);
impl StationOutcome {
    pub(super) fn attempt(result: Result<(), StationAttemptFailure>, at: u64) -> Self {
        Self::Attempt { result, at }
    }
    /// Caller observes the actual provider and service owner in this Play.
    /// Configuration retirement alone is not evidence of attachment loss.
    pub(super) fn detached(&mut self, now: u64) -> bool {
        if let Self::Attempt { result: Ok(()), at } = *self {
            if now >= at {
                *self = Self::Detached { lost_at: now };
                return true;
            }
        }
        false
    }
    // The original join Signs remain separately owned and immutable. This is
    // only the selection-policy observation, whose clock advances on loss.
    pub(super) fn policy_outcome(&self) -> Option<(bool, u64)> {
        match *self {
            Self::Unobserved => None,
            Self::Attempt { result, at } => Some((result.is_ok(), at)),
            Self::Detached { lost_at } => Some((false, lost_at)),
        }
    }
}
