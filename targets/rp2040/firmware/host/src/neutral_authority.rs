//! Original Issuers and exact independently selected native operation ceilings.
use alloc::{string::String, vec::Vec};
use conduit_body::NativePreparationContext;
use conduit_core::AuthorityGrant;
use conduit_i2c_transaction::{contract::*, installation::I2C_AUTHORITY, I2cProvider};
use conduit_mcu_host_runtime::{
    execution::Policy,
    execution_authority::{Issuer, NativeFacts, Operation, Refusal},
};
use conduit_monotonic_wait::clock_at::{
    contract::*,
    installation::CLOCK_AUTHORITY,
    owner::{MonotonicDeadlineProvider, MAXIMUM_POLL_STEPS},
};
pub struct Operations {
    pub bus: Operation<'static>,
    pub clock: Operation<'static>,
}
impl Operations {
    pub fn selected() -> Result<Self, Refusal> {
        let bus = crate::config::I2C_CALL_MAXIMUM_OPERATIONS;
        let clock = crate::config::CLOCK_AT_MAXIMUM_CALL_OPERATIONS;
        if bus == 0 || clock == 0 {
            return Err(Refusal::Bounds);
        }
        Ok(Self {
            bus: Operation {
                kind: I2C_KIND,
                authority: I2C_AUTHORITY,
                call: I2C_CALL,
                maximum_parameter_bytes: I2C_MAXIMUM_BYTES,
                maximum_result_bytes: I2C_MAXIMUM_BYTES,
                maximum_work_units: 1,
                maximum_operations: bus,
            },
            clock: Operation {
                kind: CLOCK_KIND,
                authority: CLOCK_AUTHORITY,
                call: CLOCK_CALL,
                maximum_parameter_bytes: CLOCK_MAXIMUM_BYTES,
                maximum_result_bytes: CLOCK_MAXIMUM_BYTES,
                maximum_work_units: MAXIMUM_POLL_STEPS,
                maximum_operations: clock,
            },
        })
    }
}
pub struct Issuers {
    pub bus: Issuer,
    pub clock: Issuer,
}
impl Issuers {
    pub const fn new() -> Self {
        Self {
            bus: Issuer::new(),
            clock: Issuer::new(),
        }
    }
    pub fn authorize<I: I2cProvider, D: MonotonicDeadlineProvider>(
        &mut self,
        context: NativePreparationContext<'_>,
        policy: Policy<'_>,
        equipment: &crate::neutral_equipment::Equipment<I, D>,
        operations: &Operations,
        maximum_bytes: usize,
    ) -> Result<(), Refusal> {
        self.bus.authorize(
            context,
            policy,
            &NativeFacts::i2c(equipment.bus.as_ref().ok_or(Refusal::Scope)?),
            &operations.bus,
            "rp2040/source/i2c-authority",
            maximum_bytes,
        )?;
        let remaining = maximum_bytes
            .checked_sub(self.bus.retained_backing_bytes().ok_or(Refusal::Bounds)?)
            .ok_or(Refusal::Bounds)?;
        self.clock.authorize(
            context,
            policy,
            &NativeFacts::clock_at(equipment.clock.as_ref().ok_or(Refusal::Scope)?),
            &operations.clock,
            "rp2040/source/clock-authority",
            remaining,
        )
    }
    /// Copy only originals retained by these Issuers for authenticated return.
    pub fn copy_grants(&self, maximum_bytes: usize) -> Result<Vec<AuthorityGrant>, Refusal> {
        let mut result = Vec::new();
        let mut budget = maximum_bytes;
        result
            .try_reserve_exact(2)
            .map_err(|_| Refusal::Allocation)?;
        budget = budget
            .checked_sub(
                result
                    .capacity()
                    .checked_mul(core::mem::size_of::<AuthorityGrant>())
                    .ok_or(Refusal::Bounds)?,
            )
            .ok_or(Refusal::Bounds)?;
        for grant in [self.bus.grant(), self.clock.grant()] {
            let grant = grant.ok_or(Refusal::Scope)?;
            let mut copy = |text: &str| -> Result<String, Refusal> {
                if text.len() > budget {
                    return Err(Refusal::Bounds);
                }
                let mut s = String::new();
                s.try_reserve_exact(text.len())
                    .map_err(|_| Refusal::Allocation)?;
                budget = budget.checked_sub(s.capacity()).ok_or(Refusal::Bounds)?;
                s.push_str(text);
                Ok(s)
            };
            result.push(AuthorityGrant {
                grant_id: copy(grant.grant_id.as_str())?.into(),
                contract_id: copy(grant.contract_id.as_str())?.into(),
                host_call_contract_id: copy(grant.host_call_contract_id.as_str())?.into(),
                subject_kind: copy(grant.subject_kind.as_str())?.into(),
                host_id: copy(grant.host_id.as_str())?.into(),
                boot_id: copy(grant.boot_id.as_str())?.into(),
                capability_id: copy(grant.capability_id.as_str())?.into(),
            });
        }
        Ok(result)
    }
}
