//! Host-owned admission for one selected dynamic local-model pool member.
//!
//! The common pool kernel owns selection. This boundary binds that selection
//! to the std Host's current capability/resource ledger and owns the exact
//! finite semantic sessions until terminal release.

use crate::{
    kernel_preparation::KernelResourceReservation, pool_member_sessions::PoolMemberSessions,
    StdHost,
};
use conduit_core::{PlacementId, Plan, PoolSelectionEvidence};

pub struct AdmittedLocalModelPoolMember {
    sessions: PoolMemberSessions,
    reservation: Option<KernelResourceReservation>,
}

impl AdmittedLocalModelPoolMember {
    pub fn sessions(&self) -> &PoolMemberSessions {
        &self.sessions
    }

    pub fn sessions_mut(&mut self) -> &mut PoolMemberSessions {
        &mut self.sessions
    }
}

impl StdHost {
    pub fn prepare_local_model_pool_member(
        &mut self,
        plan: &Plan,
        selection: PoolSelectionEvidence,
        consumer_placement_id: &PlacementId,
    ) -> Result<AdmittedLocalModelPoolMember, String> {
        selection
            .validate(plan)
            .map_err(|error| format!("validate pool member selection: {error:?}"))?;
        let pool = plan
            .fragments
            .first()
            .and_then(|fragment| {
                fragment
                    .shared_pools
                    .iter()
                    .find(|pool| pool.pool_id == selection.pool_id)
            })
            .ok_or_else(|| "selected shared pool is absent from the Plan".to_string())?;
        let realization = pool
            .realization_envelope
            .get(usize::from(selection.selected_realization.ok_or_else(
                || "selected pool realization is absent".to_string(),
            )?))
            .ok_or_else(|| "selected pool realization is outside the Plan".to_string())?;
        let advertisement = self.advertisement().clone();
        if realization.host_id != advertisement.host_id
            || realization.boot_id != advertisement.boot_id
            || realization.offer_generation != advertisement.offer_generation
        {
            return Err("selected pool realization is stale for this Host".into());
        }
        let reservation = self.kernel_resources.reserve_pool_member(
            &advertisement,
            plan.plan_id.clone(),
            &realization.capability_id,
            &realization.resources,
        )?;
        let sessions = match PoolMemberSessions::prepare(
            plan,
            selection,
            consumer_placement_id,
            &advertisement.host_id,
        ) {
            Ok(sessions) => sessions,
            Err(error) => {
                self.kernel_resources.release(reservation)?;
                return Err(error);
            }
        };
        Ok(AdmittedLocalModelPoolMember {
            sessions,
            reservation: Some(reservation),
        })
    }

    pub fn release_local_model_pool_member(
        &mut self,
        mut member: AdmittedLocalModelPoolMember,
    ) -> Result<(), String> {
        let reservation = member
            .reservation
            .take()
            .ok_or_else(|| "pool member reservation was already released".to_string())?;
        self.kernel_resources.release(reservation)
    }
}
