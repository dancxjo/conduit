//! Fixed-storage operations for the canonical PREWAKE robotics simulation.

use conduit_core::Scalar;
use conduit_kernel::{Failure, FailureCode, OperationAction, OperationInput, PortId, ValueRef};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoboticsDriveEffect {
    Projected { linear: Scalar, angular: Scalar },
    Suppressed,
    Cancelled,
}

pub(super) struct RoboticsSourceOperation {
    pub(super) availability: conduit_semantic_catalog::RoboticsSimulationAvailability,
    pub(super) values: [Option<ValueRef>; 2],
    pub(super) next: usize,
    pub(super) cancelled: bool,
}

impl RoboticsSourceOperation {
    pub(super) fn start(&self) -> OperationAction {
        if self.cancelled {
            return fail(FailureCode::Cancelled, 47);
        }
        match self.availability {
            conduit_semantic_catalog::RoboticsSimulationAvailability::Fresh => {
                self.emit_or_complete()
            }
            conduit_semantic_catalog::RoboticsSimulationAvailability::Missing => {
                fail(FailureCode::InvalidInput, 40)
            }
            conduit_semantic_catalog::RoboticsSimulationAvailability::Stale => {
                fail(FailureCode::InvalidInput, 41)
            }
        }
    }

    pub(super) fn resume(&self, _input: OperationInput) -> OperationAction {
        fail(FailureCode::InvalidLifecycle, 42)
    }

    pub(super) fn advance(&mut self) -> OperationAction {
        self.next = self.next.saturating_add(1);
        self.emit_or_complete()
    }

    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }

    fn emit_or_complete(&self) -> OperationAction {
        self.values
            .get(self.next)
            .copied()
            .flatten()
            .map_or(OperationAction::Complete, |value| OperationAction::Emit {
                port: PortId(u16::try_from(self.next).expect("robotics has at most two outputs")),
                value,
            })
    }
}

pub(super) struct RoboticsDriveOperation {
    linear: Option<Scalar>,
    angular: Option<Scalar>,
    closed: [bool; 2],
    effect: Option<RoboticsDriveEffect>,
}

pub(super) struct RoboticsDiscardOperation {
    consumed: bool,
}

impl RoboticsDiscardOperation {
    pub(super) const fn new() -> Self {
        Self { consumed: false }
    }

    pub(super) const fn start(&self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Value {
                port: PortId(0), ..
            } if !self.consumed => {
                self.consumed = true;
                OperationAction::Await
            }
            OperationInput::Closed { port: PortId(0) } if self.consumed => {
                OperationAction::Complete
            }
            _ => fail(FailureCode::InvalidLifecycle, 48),
        }
    }
}

impl RoboticsDriveOperation {
    pub(super) const fn new() -> Self {
        Self {
            linear: None,
            angular: None,
            closed: [false; 2],
            effect: None,
        }
    }

    pub(super) const fn start(&self) -> OperationAction {
        OperationAction::Await
    }

    pub(super) fn resume(&mut self, input: OperationInput) -> OperationAction {
        match input {
            OperationInput::Closed { port } => {
                let index = usize::from(port.0);
                if index >= self.closed.len() || self.closed[index] {
                    return fail(FailureCode::InvalidPort, 43);
                }
                self.closed[index] = true;
                if self.closed.iter().all(|closed| *closed) {
                    self.effect = Some(RoboticsDriveEffect::Suppressed);
                    OperationAction::Complete
                } else {
                    OperationAction::Await
                }
            }
            _ => fail(FailureCode::InvalidLifecycle, 44),
        }
    }

    pub(super) fn resume_value(
        &mut self,
        port: PortId,
        value: ValueRef,
        canonical: &[u8],
    ) -> OperationAction {
        let index = usize::from(port.0);
        if index >= self.closed.len() || self.closed[index] {
            return fail(FailureCode::InvalidInput, 45);
        }
        let decoded = match index {
            0 if self.linear.is_none()
                && value.byte_len == conduit_core::SCALAR_ENCODED_LEN as u32 =>
            {
                Scalar::decode(canonical).map(|decoded| self.linear = Some(decoded))
            }
            1 if self.angular.is_none()
                && value.byte_len == conduit_core::SCALAR_ENCODED_LEN as u32 =>
            {
                Scalar::decode(canonical).map(|decoded| self.angular = Some(decoded))
            }
            _ => return fail(FailureCode::InvalidInput, 46),
        };
        if decoded.is_err() {
            return fail(FailureCode::InvalidInput, 46);
        }
        match (self.linear, self.angular) {
            (Some(linear), Some(angular)) => {
                self.effect = Some(RoboticsDriveEffect::Projected { linear, angular });
                OperationAction::Complete
            }
            _ => OperationAction::Await,
        }
    }

    pub(super) fn cancel(&mut self) {
        self.effect = Some(RoboticsDriveEffect::Cancelled);
    }

    pub(super) const fn effect(&self) -> Option<RoboticsDriveEffect> {
        self.effect
    }
}

fn fail(code: FailureCode, detail: u16) -> OperationAction {
    OperationAction::Fail(Failure { code, detail })
}
