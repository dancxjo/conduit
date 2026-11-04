//! Retire only the explicitly departing member's transient presentation route.
use super::*;
impl Owner {
    pub(in crate::durable_host) fn retire_native_member_route(
        &mut self,
        credential: &MembershipCredential,
    ) {
        let Some(route) = self
            .pending_native_mask
            .as_ref()
            .filter(|route| route.credential == *credential)
        else {
            return;
        };
        if let Some(wardrobe) = &mut self.face_wardrobe {
            wardrobe.forget_show_for(&route.seal.route_plan_id);
        }
        self.pending_native_mask = None;
    }
}
