//! Actual owned checkpoint work, including the selected chunk preparation cost.
use crate::native_body_checkpoint::incremental::Operation;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Phase {
    Staging,
    Prepare,
    Capture,
    FrozenChunk,
    ReplayChunk,
    Physical(Operation),
    Finish,
    Complete,
    Failed,
}
