//! Raw storage only, accessible while the sole actual Host is loaned to Play.
//! No parser, Core, Source or descendant preparation authority is supplied.
use super::*;
use crate::body_execution::ImageRootPreparationOwner;
impl ImageRootPreparationOwner {
    pub fn prepare_member_return_frame(
        &self,
    ) -> Result<MemberReturnFrame, MemberReturnFrameRefusal> {
        let (memory, scope) = self.reserve_member_return_frame()?;
        let frame = MemberReturnFrame::allocate(scope, memory)?;
        if !self.member_return_frame_is_current(&frame.memory, &frame.scope) {
            return Err(MemberReturnFrameRefusal::WrongOriginalOwner);
        }
        Ok(frame)
    }
}
impl MemberReturnFrame {
    pub fn capture_with_root<'a>(
        &'a mut self,
        owner: &ImageRootPreparationOwner,
        reader: &mut impl Read,
    ) -> Result<CapturedMemberReturnFrame<'a>, MemberReturnFrameRefusal> {
        if !owner.member_return_frame_is_current(&self.memory, &self.scope) {
            return Err(MemberReturnFrameRefusal::WrongOriginalOwner);
        }
        let captured = self.capture_bounded(reader)?;
        if !owner.member_return_frame_is_current(&captured.frame.memory, &captured.frame.scope) {
            return Err(MemberReturnFrameRefusal::WrongOriginalOwner);
        }
        Ok(captured)
    }
}
