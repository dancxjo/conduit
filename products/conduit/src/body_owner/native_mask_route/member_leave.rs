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

impl Owner {
    pub(in crate::durable_host) fn bind_native_carrier_presence(
        &mut self,
        id: &conduit_core::PlanId,
        deadline: std::time::Instant,
    ) -> Result<(), String> {
        let route = self
            .pending_native_mask
            .as_mut()
            .filter(|route| route.seal.route_plan_id == *id)
            .ok_or("member carrier selected route was replaced")?;
        if std::time::Instant::now() >= deadline {
            self.retire_native_route_id(id);
            return Err("member carrier presence expired".into());
        }
        route.carrier_presence_deadline = Some(deadline);
        Ok(())
    }
    pub(in crate::durable_host) fn retire_native_route_id(&mut self, id: &conduit_core::PlanId) {
        let Some(route) = self
            .pending_native_mask
            .as_ref()
            .filter(|route| route.seal.route_plan_id == *id)
        else {
            return;
        };
        if let Some(wardrobe) = &mut self.face_wardrobe {
            wardrobe.forget_show_for(&route.seal.route_plan_id);
        }
        self.pending_native_mask = None;
    }
}
