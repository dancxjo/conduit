//! Bounded continuity truth carried by admitted Parts.

use crate::{BodyId, PartId};
use alloc::{string::String, vec::Vec};
use conduit_core::{BootId, PlanId};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartContinuityClaim {
    pub body_id: BodyId,
    pub part_id: PartId,
    pub membership_generation: u64,
    pub continuity_proof: String,
    pub workload_revision: u64,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CurrentPartRuntime {
    pub part_id: PartId,
    pub boot_id: BootId,
    pub plan_id: Option<PlanId>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DurableBodyContinuity {
    pub body_id: BodyId,
    maximum_parts: usize,
    claims: Vec<PartContinuityClaim>,
    current: Vec<CurrentPartRuntime>,
    extinct: bool,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum BodyContinuityRefusal {
    InvalidProof,
    StaleClaim,
    PartCapacityExhausted,
    BodyExtinct,
    UnknownPart,
    PartsRemain,
}

impl DurableBodyContinuity {
    pub fn establish(
        initial: PartContinuityClaim,
        runtime: CurrentPartRuntime,
        maximum_parts: usize,
    ) -> Result<Self, BodyContinuityRefusal> {
        if maximum_parts == 0
            || initial.body_id.as_str().is_empty()
            || initial.part_id != runtime.part_id
            || initial.continuity_proof.is_empty()
        {
            return Err(BodyContinuityRefusal::InvalidProof);
        }
        Ok(Self {
            body_id: initial.body_id.clone(),
            maximum_parts,
            claims: alloc::vec![initial],
            current: alloc::vec![runtime],
            extinct: false,
        })
    }
    pub fn admit(
        &mut self,
        claim: PartContinuityClaim,
        runtime: CurrentPartRuntime,
    ) -> Result<(), BodyContinuityRefusal> {
        if self.extinct {
            return Err(BodyContinuityRefusal::BodyExtinct);
        }
        if claim.body_id != self.body_id
            || claim.part_id != runtime.part_id
            || claim.continuity_proof.is_empty()
        {
            return Err(BodyContinuityRefusal::InvalidProof);
        }
        if self.claims.iter().any(|old| {
            old.part_id == claim.part_id && old.membership_generation >= claim.membership_generation
        }) {
            return Err(BodyContinuityRefusal::StaleClaim);
        }
        if self.claims.len() == self.maximum_parts {
            return Err(BodyContinuityRefusal::PartCapacityExhausted);
        }
        self.claims.push(claim);
        self.current.push(runtime);
        Ok(())
    }
    /// Record that the current Boot for an admitted Part is no longer observed.
    /// The durable membership claim remains valid and the Body keeps existing.
    pub fn observe_offline(&mut self, part: &PartId) -> Result<(), BodyContinuityRefusal> {
        if !self.claims.iter().any(|claim| &claim.part_id == part) {
            return Err(BodyContinuityRefusal::UnknownPart);
        }
        if !self.current.iter().any(|runtime| &runtime.part_id == part) {
            return Err(BodyContinuityRefusal::UnknownPart);
        }
        self.current.retain(|runtime| &runtime.part_id != part);
        Ok(())
    }
    /// Explicitly revoke a Part's durable membership without declaring the
    /// Body extinct. Extinction is a separate biographical fact.
    pub fn revoke_part(&mut self, part: &PartId) -> Result<(), BodyContinuityRefusal> {
        if !self.claims.iter().any(|claim| &claim.part_id == part) {
            return Err(BodyContinuityRefusal::UnknownPart);
        }
        self.claims.retain(|claim| &claim.part_id != part);
        self.current.retain(|runtime| &runtime.part_id != part);
        Ok(())
    }
    /// Declare the Body extinct only after every Part membership has been
    /// explicitly revoked.
    pub fn declare_extinct(&mut self) -> Result<(), BodyContinuityRefusal> {
        if !self.claims.is_empty() {
            return Err(BodyContinuityRefusal::PartsRemain);
        }
        self.extinct = true;
        Ok(())
    }
    pub fn reboot(
        &mut self,
        part: &PartId,
        fresh_boot: BootId,
    ) -> Result<(), BodyContinuityRefusal> {
        if self.extinct {
            return Err(BodyContinuityRefusal::BodyExtinct);
        }
        let runtime = self
            .current
            .iter_mut()
            .find(|runtime| &runtime.part_id == part)
            .ok_or(BodyContinuityRefusal::UnknownPart)?;
        runtime.boot_id = fresh_boot;
        runtime.plan_id = None;
        Ok(())
    }
    pub fn is_continuing(&self) -> bool {
        !self.extinct
    }
    pub fn current_parts(&self) -> usize {
        self.current.len()
    }
    pub fn admitted_parts(&self) -> usize {
        self.claims.len()
    }
    pub const fn is_extinct(&self) -> bool {
        self.extinct
    }
}
