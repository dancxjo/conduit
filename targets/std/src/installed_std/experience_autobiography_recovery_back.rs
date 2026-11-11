//! Bounded pairing of pending experience with actual result or explicit loss.
use super::back::{BackBudget, BackFactory, InstalledBack};
use conduit_core::PlannedGear;
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};
use conduit_semantic_catalog::{
    BoundedExperienceAutobiographyRecovery, EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES,
};
const PENDING: PortId = PortId(0);
const RESULT: PortId = PortId(1);
const TERMINAL: PortId = PortId(2);
const NEXT: PortId = PortId(0);
const LOSS: PortId = PortId(1);
const RESPONSE: PortId = PortId(2);
pub(super) static FACTORY: BackFactory = BackFactory {
    implementation_id: conduit_std_offers::EXPERIENCE_AUTOBIOGRAPHY_RECOVERY_IMPLEMENTATION,
    budget,
    prepare,
};
pub(super) struct ExperienceAutobiographyRecoveryBack {
    codec: BoundedExperienceAutobiographyRecovery,
    terminal: [u8; conduit_core::TERMINAL_INFO_ENCODED_LEN],
    response: Vec<u8>,
    staged: bool,
    staged_loss: bool,
    closed: [bool; 3],
    complete: bool,
}
impl<const PORTS: usize> StepBack<PORTS> for ExperienceAutobiographyRecoveryBack {
    fn terminal_transductions(&self) -> [Option<AssignedTerminalTransduction>; PORTS] {
        core::array::from_fn(|index| {
            (index < 3).then_some(AssignedTerminalTransduction {
                input: PortId(index as u16),
                output: NEXT,
                normal_close: if index == 2 {
                    AssignedNormalCloseTransduction::NotAccepted
                } else {
                    AssignedNormalCloseTransduction::Consume
                },
                abnormal: if index == 2 {
                    AssignedAbnormalTransduction::Recover
                } else {
                    AssignedAbnormalTransduction::NotAccepted
                },
                cancellation: AssignedCancellationTransduction::NotCancellable,
            })
        })
    }
    fn step(&mut self, io: &mut StepIo<PORTS>, bytes: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.complete {
            return StepOutcome::Complete;
        }
        // A projected abnormal closes its source's normal result Cord too.
        // Consume that exposed close before committing the separate terminal.
        if !self.closed[1] && io.input_closed(RESULT) && bytes.input(RESULT).is_none() {
            if io.consume_closed(RESULT).is_err() {
                return failure(20);
            }
            self.closed[1] = true;
            return StepOutcome::Progress;
        }
        let abnormal = io.input_abnormal(TERMINAL);
        let terminal = abnormal
            .as_ref()
            .map(|value| value.as_slice())
            .or_else(|| bytes.input(TERMINAL));
        if let (Some(pending), Some(terminal)) = (bytes.input(PENDING), terminal) {
            if bytes.input(RESULT).is_some() {
                return failure(23);
            }
            if !io.output_ready(NEXT) || !io.output_ready(LOSS) {
                return StepOutcome::Await;
            }
            let length = match self.codec.recover(pending, terminal) {
                Ok(output) => output.len() as u32,
                Err(_) => return failure(24),
            };
            if !io.output_ready_for(NEXT, length)
                || !io.output_ready_for(LOSS, terminal.len() as u32)
            {
                return StepOutcome::Await;
            }
            self.terminal.copy_from_slice(terminal);
            if io.consume(PENDING).is_err()
                || (if abnormal.is_some() {
                    io.consume_abnormal(TERMINAL).map(|_| ())
                } else {
                    io.consume(TERMINAL).map(|_| ())
                })
                .is_err()
                || io.send_prepared(NEXT, length).is_err()
                || io.send_prepared(LOSS, terminal.len() as u32).is_err()
            {
                return failure(25);
            }
            self.staged = true;
            self.staged_loss = true;
            // Complete recovery now, even when live pending input stays open.
            // Consume laws make endpoint loss explicit; no further model work
            // is fabricated while independent Body work keeps running.
            return StepOutcome::Complete;
        }
        if let (Some(pending), Some(result)) = (bytes.input(PENDING), bytes.input(RESULT)) {
            if !io.output_ready(NEXT) || !io.output_ready(RESPONSE) {
                return StepOutcome::Await;
            }
            let length = match self.codec.execute(pending, result) {
                Ok(output) => output.len() as u32,
                Err(_) => return failure(26),
            };
            if !io.output_ready_for(NEXT, length)
                || !io.output_ready_for(RESPONSE, result.len() as u32)
            {
                return StepOutcome::Await;
            }
            self.response.clear();
            self.response.extend_from_slice(result);
            if io.consume(PENDING).is_err()
                || io.consume(RESULT).is_err()
                || io.send_prepared(NEXT, length).is_err()
                || io.send_prepared(RESPONSE, result.len() as u32).is_err()
            {
                return failure(25);
            }
            self.staged = true;
            self.staged_loss = false;
            return StepOutcome::Progress;
        }
        // Normal result closure may precede the projected loss value. Wait for
        // both result and terminal paths before declaring an unmatched pending.
        // Terminal is a projected Value; normal source closure carries no
        // terminal payload and must not be consumed as its abnormal outcome.
        for (index, port) in [PENDING, RESULT].into_iter().enumerate() {
            if !self.closed[index] && io.input_closed(port) && io.input_abnormal(port).is_none() {
                if io.consume_closed(port).is_err() {
                    return failure(20);
                }
                self.closed[index] = true;
                return StepOutcome::Progress;
            }
        }
        if self.closed[0] && (bytes.input(RESULT).is_some() || terminal.is_some())
            || self.closed[1] && self.closed[2] && bytes.input(PENDING).is_some()
        {
            return failure(21);
        }
        if self.closed[0] && self.closed[1] {
            self.complete = true;
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        if !self.staged {
            return None;
        }
        if port == NEXT {
            Some(self.codec.output())
        } else if port == RESPONSE && !self.staged_loss {
            Some(&self.response)
        } else if port == LOSS && self.staged_loss {
            Some(&self.terminal)
        } else {
            None
        }
    }
    fn step_committed(&mut self) {
        if self.staged && self.staged_loss {
            self.complete = true;
        }
        self.staged = false;
        self.staged_loss = false;
        self.response.clear();
    }
    fn cancel(&mut self) {
        self.staged = false;
        self.staged_loss = false;
        self.complete = true;
    }
}
fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}
fn budget(gear: &PlannedGear) -> Result<BackBudget, String> {
    let contract = conduit_semantic_catalog::experience_autobiography_recovery_contract();
    let expected = contract
        .terminal_transductions()
        .cloned()
        .collect::<Vec<_>>();
    if gear.terminal_transductions != expected {
        return Err("autobiography recovery terminal laws differ from installed contract".into());
    }
    // The shared pure admission helper excludes terminal-capable backs. Check
    // their exact laws above, then reuse every other identity/authority check.
    let mut pure = gear.clone();
    pure.terminal_transductions.clear();
    super::episode_loop_back::exact_pure_gear(
        &pure,
        &conduit_std_offers::experience_autobiography_recovery_offer(),
    )?;
    Ok(BackBudget {
        value_items: 2,
        value_bytes: EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as u32
            + conduit_semantic_catalog::EXPERIENCE_AUTOBIOGRAPHY_MAX_RESPONSE_BYTES as u32,
        maximum_value_bytes: EXPERIENCE_AUTOBIOGRAPHY_MAX_BYTES as u32,
        host_requests: 0,
        sign_items: 24,
    })
}
fn prepare(
    gear: &PlannedGear,
    _: &mut conduit_kernel::HostedValueStore,
) -> Result<InstalledBack, String> {
    budget(gear)?;
    Ok(InstalledBack::ExperienceAutobiographyRecovery(
        ExperienceAutobiographyRecoveryBack {
            codec: BoundedExperienceAutobiographyRecovery::prepare(),
            terminal: [0; conduit_core::TERMINAL_INFO_ENCODED_LEN],
            response: Vec::with_capacity(
                conduit_semantic_catalog::EXPERIENCE_AUTOBIOGRAPHY_MAX_RESPONSE_BYTES,
            ),
            staged: false,
            staged_loss: false,
            closed: [false; 3],
            complete: false,
        },
    ))
}

#[cfg(test)]
#[allow(dead_code)]
#[path = "../../../../semantics/catalog/tests/common/experience_evidence.rs"]
mod fixture;
#[cfg(test)]
mod tests {
    use super::*;
    use conduit_human::*;
    use conduit_kernel::ValueRef;
    use conduit_plot::rust_binding::NativeRustBinding;
    fn pending() -> Vec<u8> {
        ExperienceAutobiographyPending::new(
            0,
            fixture::request(&["seen"], vec![fixture::observation("seen")]),
            conduit_semantic_catalog::initial_experience_autobiography_state(),
            true,
        )
        .unwrap()
        .encode()
        .unwrap()
    }
    fn back() -> ExperienceAutobiographyRecoveryBack {
        ExperienceAutobiographyRecoveryBack {
            codec: BoundedExperienceAutobiographyRecovery::prepare(),
            terminal: [0; conduit_core::TERMINAL_INFO_ENCODED_LEN],
            response: Vec::with_capacity(
                conduit_semantic_catalog::EXPERIENCE_AUTOBIOGRAPHY_MAX_RESPONSE_BYTES,
            ),
            staged: false,
            staged_loss: false,
            closed: [false; 3],
            complete: false,
        }
    }
    fn reference(slot: u32, bytes: &[u8]) -> ValueRef {
        ValueRef {
            slot: slot as u16,
            generation: 0,
            byte_len: bytes.len() as u32,
        }
    }
    #[test]
    fn projected_loss_pairs_pending_and_publishes_state_and_exact_loss_atomically() {
        let pending = pending();
        let terminal = conduit_core::TerminalInfo::new(
            conduit_core::TerminalCategory::ProviderOrResourceLoss,
            [9; 32],
        )
        .encode();
        let mut back = back();
        let values = [
            Some(reference(0, &pending)),
            None,
            Some(reference(1, &terminal)),
        ];
        let bytes = StepInputBytes::test_frame(
            [Some(pending.as_slice()), None, Some(terminal.as_slice())],
            None,
        );
        let mut close = StepIo::test_frame(values, [false, true, false], [None; 3], None, 8);
        assert_eq!(back.step(&mut close, &bytes), StepOutcome::Progress);
        assert!(!close.test_consumed(PENDING));
        assert!(!close.test_consumed(TERMINAL));
        StepBack::<3>::step_committed(&mut back);
        let mut pressure = StepIo::test_frame(
            values,
            [false, true, false],
            [Some(65_536), None, None],
            None,
            8,
        );
        assert_eq!(back.step(&mut pressure, &bytes), StepOutcome::Await);
        assert!(!pressure.test_consumed(PENDING));
        assert!(!pressure.test_consumed(TERMINAL));
        assert!(StepBack::<3>::prepared_output(&back, NEXT).is_none());
        let mut ready = StepIo::test_frame(
            values,
            [false, true, false],
            [Some(65_536), Some(34), None],
            None,
            8,
        );
        assert_eq!(back.step(&mut ready, &bytes), StepOutcome::Complete);
        assert!(ready.test_consumed(PENDING));
        assert!(ready.test_consumed(TERMINAL));
        assert_eq!(
            StepBack::<3>::prepared_output(&back, LOSS),
            Some(terminal.as_slice())
        );
        let state = ExperienceAutobiographyState::decode(
            StepBack::<3>::prepared_output(&back, NEXT).unwrap(),
        )
        .unwrap();
        assert_eq!(*state.completed(), 1);
        assert!(state.records().get().as_slice().is_empty());
        assert!(!back.complete, "output staging must not commit loss state");
        StepBack::<3>::step_committed(&mut back);
        assert!(back.complete);
        assert!(StepBack::<3>::prepared_output(&back, LOSS).is_none());
    }
    #[test]
    fn normal_result_close_waits_for_the_explicit_terminal_path() {
        let pending = pending();
        let mut back = back();
        let mut closed = StepIo::test_frame(
            [Some(reference(0, &pending)), None, None],
            [false, true, false],
            [Some(65_536), Some(34), None],
            None,
            8,
        );
        assert_eq!(
            back.step(
                &mut closed,
                &StepInputBytes::test_frame([Some(pending.as_slice()), None, None], None)
            ),
            StepOutcome::Progress
        );
        assert!(closed.test_consumed_closed(RESULT));
        assert!(!closed.test_consumed(PENDING));
        let terminal = conduit_core::TerminalInfo::new(
            conduit_core::TerminalCategory::ProviderOrResourceLoss,
            [4; 32],
        )
        .encode();
        let mut loss = StepIo::test_frame(
            [
                Some(reference(0, &pending)),
                None,
                Some(reference(1, &terminal)),
            ],
            [false, true, false],
            [Some(65_536), Some(34), None],
            None,
            8,
        );
        assert_eq!(
            back.step(
                &mut loss,
                &StepInputBytes::test_frame(
                    [Some(pending.as_slice()), None, Some(terminal.as_slice())],
                    None
                )
            ),
            StepOutcome::Complete
        );
        assert_eq!(
            StepBack::<3>::prepared_output(&back, LOSS),
            Some(terminal.as_slice())
        );
    }
}
