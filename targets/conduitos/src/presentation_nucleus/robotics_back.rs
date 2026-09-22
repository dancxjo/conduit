//! Fixed-storage Backs for the canonical PREWAKE robotics simulation.

use conduit_core::Scalar;
use conduit_kernel::scheduler::{StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{Failure, FailureCode, PortId, ValueRef};

const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum RoboticsDriveEffect {
    Projected { linear: Scalar, angular: Scalar },
    Suppressed,
    Cancelled,
}

pub(super) struct RoboticsSourceBack {
    pub(super) availability: conduit_semantic_catalog::RoboticsSimulationAvailability,
    pub(super) values: [Option<ValueRef>; 2],
    pub(super) next: usize,
    pub(super) cancelled: bool,
}

impl RoboticsSourceBack {
    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if self.cancelled {
            return fail(FailureCode::Cancelled, 47);
        }
        match self.availability {
            conduit_semantic_catalog::RoboticsSimulationAvailability::Missing => {
                return fail(FailureCode::InvalidInput, 40);
            }
            conduit_semantic_catalog::RoboticsSimulationAvailability::Stale => {
                return fail(FailureCode::InvalidInput, 41);
            }
            conduit_semantic_catalog::RoboticsSimulationAvailability::Fresh => {}
        }
        let Some(value) = self.values.get(self.next).copied().flatten() else {
            return StepOutcome::Complete;
        };
        let port = PortId(u16::try_from(self.next).expect("robotics has at most two outputs"));
        if !io.output_ready(port) {
            return StepOutcome::Await;
        }
        if io.send(port, value).is_err() {
            return fail(FailureCode::InvalidLifecycle, 42);
        }
        self.next = self.next.saturating_add(1);
        if self.values.get(self.next).copied().flatten().is_some() {
            StepOutcome::Progress
        } else {
            StepOutcome::Complete
        }
    }

    pub(super) fn cancel(&mut self) {
        self.cancelled = true;
    }
}

pub(super) struct RoboticsDriveBack {
    linear: Option<Scalar>,
    angular: Option<Scalar>,
    closed: [bool; 2],
    effect: Option<RoboticsDriveEffect>,
}

pub(super) struct RoboticsDiscardBack {
    consumed: bool,
}

impl RoboticsDiscardBack {
    pub(super) const fn new() -> Self {
        Self { consumed: false }
    }

    pub(super) fn step(&mut self, io: &mut StepIo<PORTS>) -> StepOutcome {
        if !self.consumed {
            if io.input(PortId(0)).is_some() {
                if io.consume(PortId(0)).is_err() {
                    return fail(FailureCode::InvalidLifecycle, 48);
                }
                self.consumed = true;
                return StepOutcome::Progress;
            }
            return StepOutcome::Await;
        }
        if io.input_closed(PortId(0)) {
            if io.consume_closed(PortId(0)).is_err() {
                return fail(FailureCode::InvalidLifecycle, 48);
            }
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
}

impl RoboticsDriveBack {
    pub(super) const fn new() -> Self {
        Self {
            linear: None,
            angular: None,
            closed: [false; 2],
            effect: None,
        }
    }

    pub(super) fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        for index in 0..2 {
            let port = PortId(index as u16);
            if let Some(value) = io.input(port) {
                let Some(canonical) = input_bytes.input(port) else {
                    return fail(FailureCode::InvalidInput, 46);
                };
                if let Err(failure) = self.accept(port, value, canonical) {
                    return StepOutcome::Fail(failure);
                }
                if io.consume(port).is_err() {
                    return fail(FailureCode::InvalidLifecycle, 44);
                }
                return if self.effect.is_some() {
                    StepOutcome::Complete
                } else {
                    StepOutcome::Progress
                };
            }
        }
        for index in 0..2 {
            let port = PortId(index as u16);
            if !self.closed[index] && io.input_closed(port) {
                if io.consume_closed(port).is_err() {
                    return fail(FailureCode::InvalidPort, 43);
                }
                self.closed[index] = true;
                if self.closed.iter().all(|closed| *closed) {
                    self.effect = Some(RoboticsDriveEffect::Suppressed);
                    return StepOutcome::Complete;
                }
                return StepOutcome::Progress;
            }
        }
        StepOutcome::Await
    }

    fn accept(&mut self, port: PortId, value: ValueRef, canonical: &[u8]) -> Result<(), Failure> {
        let index = usize::from(port.0);
        if index >= self.closed.len() || self.closed[index] {
            return Err(failure(FailureCode::InvalidInput, 45));
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
            _ => return Err(failure(FailureCode::InvalidInput, 46)),
        };
        decoded.map_err(|_| failure(FailureCode::InvalidInput, 46))?;
        if let (Some(linear), Some(angular)) = (self.linear, self.angular) {
            self.effect = Some(RoboticsDriveEffect::Projected { linear, angular });
        }
        Ok(())
    }

    pub(super) fn cancel(&mut self) {
        self.effect = Some(RoboticsDriveEffect::Cancelled);
    }
    pub(super) const fn effect(&self) -> Option<RoboticsDriveEffect> {
        self.effect
    }
}

fn failure(code: FailureCode, detail: u16) -> Failure {
    Failure { code, detail }
}
fn fail(code: FailureCode, detail: u16) -> StepOutcome {
    StepOutcome::Fail(failure(code, detail))
}
