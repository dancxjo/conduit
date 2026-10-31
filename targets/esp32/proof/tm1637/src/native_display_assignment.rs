//! Bounded display transfer staging; no software transfer, native possession or
//! Body start. The caller supplies independently linked acceptance facts.
use alloc::{boxed::Box, vec::Vec};
use conduit_core as c;
use conduit_environment_runtime::{
    appliance::{assigned::ResidentSourceIdentity, assignment_transfer as t},
    display_output::*,
};
pub struct StagedDisplayAssignment {
    pub admitted: Box<ValidatedAssignedDisplay>,
    wire: Vec<u8>,
    seal: [u8; 32],
}
impl StagedDisplayAssignment {
    pub fn seal(&self) -> [u8; 32] {
        self.seal
    }
    pub fn wire(&self) -> &[u8] {
        &self.wire
    }
    /// Root admits staging backing plus control reserve independently of the
    /// future kernel. Failure returns no staged state and consumes no owner.
    pub fn stage(
        wire: &[u8],
        profile: &AssignedDisplayProfile<'_>,
        resident: &[c::AssignedRecord<'_>],
        scope: AssignedDisplayScope<'_>,
        available_bytes: usize,
    ) -> Result<Self, &'static str> {
        if wire.len() > t::MAXIMUM_WIRE_BYTES {
            return Err("display-transfer-capacity");
        }
        let needed = t::MAXIMUM_RECORDS * core::mem::size_of::<c::AssignedRecord<'_>>()
            + core::mem::size_of::<ValidatedAssignedDisplay>()
            + wire.len()
            + 4096;
        if available_bytes < needed {
            return Err("display-staging-capacity");
        }
        let mut owned = Vec::new();
        owned
            .try_reserve_exact(wire.len())
            .map_err(|_| "display-staging-capacity")?;
        owned.extend_from_slice(wire);
        let mut slots = Vec::new();
        slots
            .try_reserve_exact(t::MAXIMUM_RECORDS)
            .map_err(|_| "display-staging-capacity")?;
        slots.resize(
            t::MAXIMUM_RECORDS,
            c::AssignedRecord {
                tag: 0,
                payload: &[],
            },
        );
        let decoded = t::decode(
            &owned,
            ResidentSourceIdentity::from_expanded(&profile.source.expanded_plot_id),
            resident,
            &mut slots,
        )
        .map_err(|_| "display-transfer")?;
        let admitted = ValidatedAssignedDisplay::admit(
            AssignedDisplayInput {
                header: decoded.header,
                records: decoded.records,
                maxima: c::AssignedPlanMaxima {
                    encoded_bytes: 16 * 1024,
                    runtime_state_bytes: 32 * 1024,
                    counts: [32; c::ASSIGNED_PLAN_COUNT_KINDS],
                },
            },
            profile,
            scope,
        )
        .map_err(|_| "display-assignment")?;
        Ok(Self {
            admitted: Box::new(admitted),
            seal: c::assigned_plan_payload_digest(&owned),
            wire: owned,
        })
    }
}
