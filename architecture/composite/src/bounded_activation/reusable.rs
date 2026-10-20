//! Reuse finite Value ownership without replacing the activation controller.
use super::*;
use crate::reusable_value_pool::ValueReuseError;
use conduit_core::ActivePlayId;

impl From<ValueReuseError> for BoundedActivationError {
    fn from(error: ValueReuseError) -> Self {
        match error {
            ValueReuseError::InvalidBound => Self::PlannedContractMismatch,
            ValueReuseError::InvalidLifecycle => Self::InvalidLifecycle,
            ValueReuseError::Child(error) => Self::Refused(error),
        }
    }
}
impl BoundedActivationHost {
    pub fn bind_parent_play(
        &mut self,
        parent: &ActivePlayId,
    ) -> Result<(), BoundedActivationError> {
        let activation = self
            .activation_id
            .as_deref()
            .ok_or(BoundedActivationError::InvalidLifecycle)?;
        if self.parent_play.is_some()
            || self.active.is_some()
            || !self.receipts.is_empty()
            || self.pending_input_terminal.is_some()
            || self.state != BoundedActivationState::Idle
        {
            return Err(BoundedActivationError::InvalidLifecycle);
        }
        if let Some(pool) = &mut self.reusable {
            pool.bind(parent, activation)?;
        } else {
            for (index, child) in self.ready.iter_mut().rev().enumerate() {
                child
                    .bind_parent_play(parent, activation, index as u16)
                    .map_err(BoundedActivationError::Refused)?;
            }
        }
        self.parent_play = Some(parent.clone());
        Ok(())
    }
    pub(crate) fn has_admitted_children(&self) -> bool {
        let expected = usize::from(self.contract.maximum_items);
        self.reusable.as_ref().map_or(
            self.ready.len() == expected
                && self.ready.capacity() == expected
                && self.receipts.capacity() == expected,
            |pool| pool.has_admitted_capacity(expected),
        )
    }
    pub(super) fn retire_child(&mut self, child: KernelCompositeHost) {
        if let Some(pool) = &mut self.reusable {
            pool.retire(child);
        } else {
            self.receipts.push(child);
        }
    }
    pub(super) fn activate_reusable(
        &mut self,
        sequence: u64,
        value: &ValuePayload,
    ) -> Result<BoundedActivationAdmission, BoundedActivationError> {
        use BoundedActivationError::{InvalidLifecycle, PlannedContractMismatch};
        if self.parent_play.is_none()
            || self.pending_input_terminal.is_some()
            || !matches!(
                self.state,
                BoundedActivationState::Idle
                    | BoundedActivationState::Succeeded { .. }
                    | BoundedActivationState::Active { .. }
            )
        {
            return Err(InvalidLifecycle);
        }
        let pool = self.reusable.as_ref().ok_or(InvalidLifecycle)?;
        if let Some(active) = &self.active {
            let (_, index) = active.parent_play_binding().ok_or(InvalidLifecycle)?;
            return if sequence == u64::from(index) + 1 {
                Ok(BoundedActivationAdmission::Full { sequence })
            } else {
                Err(InvalidLifecycle)
            };
        }
        if pool.remaining() == 0 {
            return if sequence == u64::from(self.contract.maximum_items) {
                Ok(BoundedActivationAdmission::MaximumItemsExceeded {
                    sequence,
                    maximum_items: self.contract.maximum_items,
                })
            } else {
                Err(InvalidLifecycle)
            };
        }
        let host = pool.host().ok_or(InvalidLifecycle)?;
        let (_, index) = host.parent_play_binding().ok_or(InvalidLifecycle)?;
        if sequence != u64::from(index) {
            return Err(InvalidLifecycle);
        }
        let port = host
            .definition()
            .internal_plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.fore_ports)
            .find(|front| {
                front.front_port_id == self.contract.input_port
                    && front.direction == conduit_core::PortDirection::Input
            })
            .ok_or(PlannedContractMismatch)?;
        if value.value_kind != self.contract.input_value_kind
            || value.encoded.len() > port.byte_capacity as usize
        {
            return Err(PlannedContractMismatch);
        }
        let mut child = self.reusable.as_mut().ok_or(InvalidLifecycle)?.take()?;
        let outcome = child
            .start()
            .and_then(|()| child.admit_input(&self.contract.input_port, sequence, value));
        let error = match outcome {
            Ok(RemoteIngressOutcome::Accepted { .. }) => {
                self.active = Some(child);
                self.input_close_pending = true;
                self.output_completed = false;
                self.state = BoundedActivationState::Active { sequence };
                return Ok(BoundedActivationAdmission::Accepted { sequence });
            }
            Ok(RemoteIngressOutcome::Full { .. }) => KernelCompositeError::InvalidLifecycle,
            Err(error) => error,
        };
        // An unexpected prepared-ingress failure terminates this owner. Keep
        // its full evidence and cancellation failures; never retry or reprepare.
        let _ = child.cancel();
        self.retire_child(child);
        self.state = BoundedActivationState::Faulted {
            sequence,
            fault: BoundedActivationFault::Execution(error.clone()),
        };
        Err(BoundedActivationError::Refused(error))
    }
}
