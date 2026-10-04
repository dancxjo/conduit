//! Shared bounded sampling of an exact current value on each trigger.
//! Input 0 replaces retained bytes; input 1 triggers output 0. Host factories
//! own contract validation and admission. This Back uses only the sole kernel.
use alloc::vec::Vec;
use conduit_kernel::{
    scheduler::{
        AssignedAbnormalTransduction, AssignedCancellationTransduction,
        AssignedNormalCloseTransduction, AssignedTerminalTransduction, StepBack, StepInputBytes,
        StepIo, StepOutcome,
    },
    Failure, FailureCode, PortId,
};

pub struct CurrentSampleBack {
    current: Vec<u8>,
    current_len: usize,
    has_current: bool,
    candidate: Vec<u8>,
    candidate_len: Option<usize>,
    output_staged: bool,
    terminal: bool,
}

impl<const PORTS: usize> StepBack<PORTS> for CurrentSampleBack {
    fn terminal_transduction(&self) -> Option<AssignedTerminalTransduction> {
        Some(AssignedTerminalTransduction {
            input: PortId(1),
            output: PortId(0),
            normal_close: AssignedNormalCloseTransduction::NotAccepted,
            abnormal: AssignedAbnormalTransduction::PropagateAfterDrain,
            cancellation: AssignedCancellationTransduction::NotCancellable,
        })
    }

    fn step(&mut self, io: &mut StepIo<PORTS>, inputs: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if self.terminal {
            return StepOutcome::Complete;
        }
        // A replacement presented with the trigger wins the tie. Commit it in
        // this transaction and leave the trigger for the next, so output never
        // exposes uncommitted private state.
        if let Some(reference) = io.input(PortId(0)) {
            let Some(bytes) = inputs.input(PortId(0)) else {
                return fail(901);
            };
            if bytes.len() != reference.byte_len as usize || bytes.len() > self.candidate.len() {
                return fail(902);
            }
            self.candidate[..bytes.len()].copy_from_slice(bytes);
            self.candidate_len = Some(bytes.len());
            io.consume(PortId(0))
                .expect("present current/sample current");
            return StepOutcome::Progress;
        }
        if let Some(terminal) = io.input_abnormal(PortId(1)) {
            io.consume_abnormal(PortId(1))
                .expect("present current/sample trigger terminal");
            self.terminal = true;
            return StepOutcome::Abnormal {
                port: PortId(0),
                terminal,
            };
        }
        if let Some(reference) = io.input(PortId(1)) {
            let Some(trigger) = inputs.input(PortId(1)) else {
                return fail(903);
            };
            if usize::try_from(reference.byte_len).ok() != Some(trigger.len()) {
                return fail(904);
            }
            if !self.has_current {
                return StepOutcome::Await;
            }
            if !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume(PortId(1))
                .expect("present current/sample trigger");
            self.output_staged = true;
            io.send_prepared(PortId(0), self.current_len as u32)
                .expect("ready admitted current/sample output");
            return StepOutcome::Progress;
        }
        StepOutcome::Await
    }

    fn prepared_output(&self, port: PortId) -> Option<&[u8]> {
        (port == PortId(0) && self.output_staged && self.has_current)
            .then_some(&self.current[..self.current_len])
    }

    fn step_committed(&mut self) {
        if let Some(len) = self.candidate_len.take() {
            self.current[..len].copy_from_slice(&self.candidate[..len]);
            self.current_len = len;
            self.has_current = true;
        }
        self.output_staged = false;
    }

    fn cancel(&mut self) {
        self.current_len = 0;
        self.has_current = false;
        self.candidate_len = None;
        self.terminal = true;
    }
}

impl CurrentSampleBack {
    /// Allocate the exact retained and candidate buffers before Play.
    /// The installed factory admits the value contract and finite envelope.
    pub fn prepare(maximum_bytes: u32) -> Self {
        let maximum = maximum_bytes as usize;
        Self {
            current: vec![0; maximum],
            current_len: 0,
            has_current: false,
            candidate: vec![0; maximum],
            candidate_len: None,
            output_staged: false,
            terminal: false,
        }
    }
}

fn fail(detail: u16) -> StepOutcome {
    StepOutcome::Fail(Failure {
        code: FailureCode::InvalidInput,
        detail,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_kernel::ValueRef;

    fn reference(slot: u16, bytes: &[u8]) -> ValueRef {
        ValueRef {
            slot,
            generation: 1,
            byte_len: bytes.len() as u32,
        }
    }

    #[test]
    fn current_update_commits_before_each_trigger_stages_the_exact_large_value() {
        let mut operation = CurrentSampleBack::prepare(4_096);
        let current = vec![0x5a; 4_096];
        let mut io = StepIo::test_frame(
            [None, Some(reference(2, &[]))],
            [false; 2],
            [Some(4_096), None],
            None,
            8,
        );
        let inputs = StepInputBytes::test_frame([None, Some(&[])], None);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Await);
        assert!(!io.test_consumed(PortId(1)));

        let mut io = StepIo::test_frame(
            [Some(reference(1, &current)), Some(reference(2, &[]))],
            [false; 2],
            [Some(4_096), None],
            None,
            8,
        );
        let inputs = StepInputBytes::test_frame([Some(&current), Some(&[])], None);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert!(io.test_consumed(PortId(0)));
        assert!(!io.test_consumed(PortId(1)));
        <CurrentSampleBack as StepBack<2>>::step_committed(&mut operation);

        let mut io = StepIo::test_frame(
            [None, Some(reference(2, &[]))],
            [false; 2],
            [Some(4_096), None],
            None,
            8,
        );
        let inputs = StepInputBytes::test_frame([None, Some(&[])], None);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(io.test_prepared_output(), Some((PortId(0), 4_096)));
        assert_eq!(
            <CurrentSampleBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
            Some(current.as_slice())
        );
        <CurrentSampleBack as StepBack<2>>::step_committed(&mut operation);

        let trigger = [7_u8];
        let mut io = StepIo::test_frame(
            [None, Some(reference(3, &trigger))],
            [false; 2],
            [Some(4_096), None],
            None,
            8,
        );
        let inputs = StepInputBytes::test_frame([None, Some(&trigger)], None);
        assert_eq!(operation.step(&mut io, &inputs), StepOutcome::Progress);
        assert_eq!(io.test_prepared_output(), Some((PortId(0), 4_096)));
        assert_eq!(
            <CurrentSampleBack as StepBack<2>>::prepared_output(&operation, PortId(0)),
            Some(current.as_slice())
        );
    }
}
