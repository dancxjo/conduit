//! Fixed raw Return capture reservation on the retained original IMAGE owner.
use super::*;
use crate::body_execution::member_return_workspace::{backing_bytes, MemberReturnFrameRefusal};
impl ImageRootPreparationOwner {
    pub(crate) fn reserve_member_return_frame(
        &self,
    ) -> Result<(ImageMemoryLease, [u8; 32]), MemberReturnFrameRefusal> {
        self.require_active()
            .map_err(|_| MemberReturnFrameRefusal::Unavailable)?;
        let memory = self
            .state
            .counter
            .reserve(backing_bytes()?)
            .map_err(|_| MemberReturnFrameRefusal::Unavailable)?;
        self.require_active()
            .map_err(|_| MemberReturnFrameRefusal::Unavailable)?;
        Ok((memory, self.state.scope))
    }
    pub(crate) fn member_return_frame_is_current(
        &self,
        memory: &ImageMemoryLease,
        scope: &[u8; 32],
    ) -> bool {
        self.state.active.load(std::sync::atomic::Ordering::Acquire)
            && self.state.scope == *scope
            && self.state.counter.owns(memory)
    }
}
