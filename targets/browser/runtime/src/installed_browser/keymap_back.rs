//! Prioritize newly observed retained selection before the next key event.
use conduit_kernel::{
    scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome},
    BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallId, PortId, RequestId,
    ValueRef,
};
pub(super) struct KeymapBack {
    pending: Option<RequestId>,
    next: u32,
    selection: Option<ValueRef>,
    require_selection: bool,
}
impl KeymapBack {
    #[cfg(test)]
    pub(super) fn new() -> Self {
        Self::with_selection(false)
    }
    pub(super) fn with_selection(require_selection: bool) -> Self {
        Self {
            pending: None,
            next: 0,
            selection: None,
            require_selection,
        }
    }
}
impl<const PORTS: usize> StepBack<PORTS> for KeymapBack {
    fn step(&mut self, io: &mut StepIo<PORTS>, _: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if self.pending != Some(request)
                || outcome.disposition != HostCallDisposition::Completed
            {
                return StepOutcome::Fail(outcome.failure.unwrap_or(Failure {
                    code: FailureCode::InvalidLifecycle,
                    detail: 43,
                }));
            }
            if outcome.output.is_some() && !io.output_ready(PortId(0)) {
                return StepOutcome::Await;
            }
            io.consume_host_completion().expect("observed completion");
            if let Some(output) = outcome.output {
                io.send(PortId(0), output.value).expect("ready output");
            }
            self.pending = None;
            return StepOutcome::Progress;
        }
        if self.pending.is_some() {
            return StepOutcome::Await;
        }
        if let Some(value) = io
            .input(PortId(1))
            .filter(|value| Some(*value) != self.selection)
        {
            let Ok(input) = BoundedValueRef::new(value, 32) else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 43,
                });
            };
            io.consume(PortId(1)).expect("present selection");
            io.request_host_call(RequestId(self.next), HostCallId(0), input)
                .expect("selection call");
            self.selection = Some(value);
            self.pending = Some(RequestId(self.next));
            self.next = self.next.saturating_add(1);
            return StepOutcome::Progress;
        }
        if self.require_selection && self.selection.is_none() {
            return if io.input_closed(PortId(1)) {
                StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 44,
                })
            } else {
                StepOutcome::Await
            };
        }
        if let Some(value) = io.input(PortId(0)) {
            let Ok(input) =
                BoundedValueRef::new(value, conduit_human::KEY_EVENT_ENCODED_LEN as u32)
            else {
                return StepOutcome::Fail(Failure {
                    code: FailureCode::InvalidInput,
                    detail: 43,
                });
            };
            io.consume(PortId(0)).expect("present key");
            io.request_host_call(RequestId(self.next), HostCallId(1), input)
                .expect("key call");
            self.pending = Some(RequestId(self.next));
            self.next = self.next.saturating_add(1);
            return StepOutcome::Progress;
        }
        if io.input_closed(PortId(0)) {
            io.consume_closed(PortId(0)).expect("key closure");
            return StepOutcome::Complete;
        }
        StepOutcome::Await
    }
    fn cancel(&mut self) {
        self.pending = None;
        self.selection = None;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn a_connected_saved_selection_must_restore_before_any_key_is_interpreted() {
        let mut back = KeymapBack::with_selection(true);
        let key = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: 3,
        };
        let mut io = StepIo::test_frame([Some(key), None], [false; 2], [Some(48), None], None, 8);
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Await
        );
        assert!(io.test_host_request().is_none());
    }
    #[test]
    fn selection_is_requested_before_an_already_available_key() {
        let mut back = KeymapBack::new();
        let key = ValueRef {
            slot: 1,
            generation: 1,
            byte_len: conduit_human::KEY_EVENT_ENCODED_LEN as u32,
        };
        let selection = ValueRef {
            slot: 2,
            generation: 1,
            byte_len: 2,
        };
        let mut io = StepIo::test_frame(
            [Some(key), Some(selection)],
            [false; 2],
            [Some(4), None],
            None,
            8,
        );
        assert_eq!(
            back.step(&mut io, &StepInputBytes::test_frame([None; 2], None)),
            StepOutcome::Progress
        );
        let (request, operation, input) = io.test_host_request().unwrap();
        assert_eq!(request, RequestId(0));
        assert_eq!(operation, HostCallId(0));
        assert_eq!(input.value, selection);
    }
}
