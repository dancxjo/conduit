//! Adapter-side completion of exact installed keyboard host requests.

use super::keyboard_input_binding::{InputRealization, KeyboardRequestAdmission};
use super::keyboard_input_completion::CompletedInputEvent;
use super::InstalledScheduler;
use conduit_kernel::scheduler::HostCallRequest;
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallOutcome};

pub(super) struct KeyboardInputHost<'a> {
    adapter: Option<&'a mut dyn crate::hosted_keyboard::HostedKeyboardAdapter>,
    keyboard_source: bool,
    button_source: bool,
    pending_keyboard: Option<HostCallRequest>,
    pending_button: Option<HostCallRequest>,
    button_encoder: conduit_semantic_catalog::PreparedButtonTransitionEncoder,
    button_held: bool,
    button_sequence: u64,
    terminal: bool,
    request_admission: Option<KeyboardRequestAdmission>,
    physical_event: u64,
    input_view_epoch: Option<u64>,
}

#[derive(Clone, Copy)]
pub(super) enum InputRequestKind {
    Keyboard,
    SpaceButton,
}

#[derive(Clone, Copy)]
struct InputEventContext {
    physical_event: u64,
    input_view_epoch: Option<u64>,
}

impl<'a> KeyboardInputHost<'a> {
    pub(super) fn new<'b>(
        adapter: Option<&'a mut dyn crate::hosted_keyboard::HostedKeyboardAdapter>,
        contracts: impl IntoIterator<Item = &'b conduit_core::HostCallContractId>,
    ) -> Self {
        let mut keyboard_source = false;
        let mut button_source = false;
        for contract in contracts {
            keyboard_source |=
                contract.as_str() == conduit_std_offers::NEXT_KEY_EVENT_HOST_CALL_CONTRACT;
            button_source |=
                contract.as_str() == conduit_std_offers::button::NEXT_TRANSITION_HOST_CALL;
        }
        Self {
            adapter,
            keyboard_source,
            button_source,
            pending_keyboard: None,
            pending_button: None,
            button_encoder: conduit_semantic_catalog::PreparedButtonTransitionEncoder::new(
                "button/primary",
            )
            .expect("reviewed button transition encoder"),
            button_held: false,
            button_sequence: 0,
            terminal: false,
            request_admission: None,
            physical_event: 0,
            input_view_epoch: None,
        }
    }

    pub(super) fn install_request_admission(
        &mut self,
        admission: KeyboardRequestAdmission,
    ) -> Result<(), String> {
        if self.terminal || self.is_pending() || self.request_admission.is_some() {
            return Err("input origin must be installed once before accepting requests".into());
        }
        self.request_admission = Some(admission);
        Ok(())
    }

    /// Snapshot supplied by the private current-Show owner before each poll.
    /// The stamp itself never grants current Face, map or input authority.
    pub(super) fn set_input_view_epoch(&mut self, epoch: Option<u64>) {
        self.input_view_epoch = if self.terminal { None } else { epoch };
    }

    pub(super) fn claim_completed_event(
        &mut self,
        node: conduit_kernel::NodeId,
        call: conduit_kernel::HostCallId,
        value: conduit_kernel::ValueRef,
    ) -> Result<&CompletedInputEvent, String> {
        self.request_admission
            .as_mut()
            .ok_or("completed input origin is absent")?
            .completions
            .claim(node, call, value)
    }

    pub(super) fn acknowledge_completed_event(
        &mut self,
        node: conduit_kernel::NodeId,
        call: conduit_kernel::HostCallId,
        value: conduit_kernel::ValueRef,
    ) -> Result<CompletedInputEvent, String> {
        self.request_admission
            .as_mut()
            .ok_or("completed input origin is absent")?
            .completions
            .acknowledge(node, call, value)
    }

    #[cfg(test)]
    pub(super) fn has_request_origin(&self, realization: InputRealization) -> bool {
        self.request_admission
            .as_ref()
            .is_some_and(|admission| admission.is_current(realization))
    }

    pub(super) fn accept(
        &mut self,
        request: HostCallRequest,
        input: &[u8],
        kind: InputRequestKind,
    ) -> Result<(), String> {
        let result = self.accept_inner(request, input, kind);
        if result.is_err() {
            if let Some(admission) = &mut self.request_admission {
                admission.revoke();
            }
        }
        result
    }

    fn accept_inner(
        &mut self,
        request: HostCallRequest,
        input: &[u8],
        kind: InputRequestKind,
    ) -> Result<(), String> {
        if self.terminal {
            return Err("keyboard input session is terminal".into());
        }
        if !input.is_empty() {
            return Err("keyboard input request carries unexpected bytes".into());
        }
        if self.adapter.is_none() {
            return Err("planned keyboard has no admitted host adapter".into());
        }
        let pending = match kind {
            InputRequestKind::Keyboard => &mut self.pending_keyboard,
            InputRequestKind::SpaceButton => &mut self.pending_button,
        };
        if pending.is_some() {
            return Err("input source has two pending requests for one semantic port".into());
        }
        if let Some(admission) = &mut self.request_admission {
            admission.accept(
                match kind {
                    InputRequestKind::Keyboard => InputRealization::RawKeyboard,
                    InputRequestKind::SpaceButton => InputRealization::SpaceButton,
                },
                request,
            )?;
        }
        *pending = Some(request);
        Ok(())
    }

    pub(super) fn cancel(&mut self) {
        if let Some(admission) = &mut self.request_admission {
            admission.revoke();
        }
        self.terminal = true;
        self.input_view_epoch = None;
        self.pending_keyboard = None;
        self.pending_button = None;
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending_keyboard.is_some() || self.pending_button.is_some()
    }

    pub(super) fn poll(&mut self, scheduler: &mut InstalledScheduler) -> Result<bool, String> {
        let result = self.poll_inner(scheduler);
        if result.is_err() {
            self.cancel();
        }
        result
    }

    fn poll_inner(&mut self, scheduler: &mut InstalledScheduler) -> Result<bool, String> {
        if self.terminal || !self.is_pending() {
            return Ok(false);
        }
        // One physical event fans out to all admitted input realizations. Do
        // not consume it until each source can retain its own completion.
        if (self.keyboard_source && self.pending_keyboard.is_none())
            || (self.button_source && self.pending_button.is_none())
        {
            return Ok(false);
        }
        if let Some(admission) = &self.request_admission {
            if admission.completions.enabled() {
                admission.completions.require_capacity(
                    usize::from(self.pending_keyboard.is_some())
                        + usize::from(self.pending_button.is_some()),
                )?;
            }
        }
        let adapter = self
            .adapter
            .as_deref_mut()
            .ok_or_else(|| "pending keyboard request lost its host adapter".to_string())?;
        match adapter.poll_next() {
            crate::hosted_keyboard::HostedKeyboardPoll::Pending => return Ok(false),
            crate::hosted_keyboard::HostedKeyboardPoll::Event(event) => {
                let physical_event = self.physical_event;
                let context = InputEventContext {
                    physical_event,
                    input_view_epoch: self.input_view_epoch,
                };
                self.physical_event = physical_event
                    .checked_add(1)
                    .ok_or("physical input event ordinal exhausted")?;
                if let Some(request) = self.pending_keyboard.take() {
                    let encoded = event.encode();
                    self.complete_value(
                        scheduler,
                        request,
                        &encoded,
                        conduit_human::KEY_EVENT_ENCODED_LEN as u32,
                        InputRealization::RawKeyboard,
                        context,
                    )?;
                }
                if event.usage() == 0x2c {
                    if let Some(request) = self.pending_button.take() {
                        let pressed = event.transition() == conduit_human::KeyTransition::Pressed;
                        if pressed == self.button_held {
                            return self.complete_failure(
                                scheduler,
                                request,
                                conduit_kernel::FailureCode::InvalidInput,
                                1,
                            );
                        }
                        let sequence = self.button_sequence;
                        let Some(next) = sequence.checked_add(1) else {
                            return self.complete_failure(
                                scheduler,
                                request,
                                conduit_kernel::FailureCode::StorageExhausted,
                                2,
                            );
                        };
                        self.button_held = pressed;
                        self.button_sequence = next;
                        let encoded =
                            self.button_encoder
                                .encode(pressed, sequence)
                                .map_err(|error| {
                                    format!("encode portable button transition: {error:?}")
                                })?;
                        let value = scheduler.store_host_value(encoded).map_err(|error| {
                            format!("store portable button transition: {error:?}")
                        })?;
                        self.complete_stored_value(
                            scheduler,
                            request,
                            value,
                            conduit_semantic_catalog::BUTTON_TRANSITION_MAXIMUM_BYTES,
                            InputRealization::SpaceButton,
                            context,
                        )?;
                    }
                }
                return Ok(true);
            }
            crate::hosted_keyboard::HostedKeyboardPoll::Cancelled => {
                self.complete_all(scheduler, HostCallDisposition::Cancelled, None)?;
            }
            crate::hosted_keyboard::HostedKeyboardPoll::Failed(detail) => {
                self.complete_all(
                    scheduler,
                    HostCallDisposition::Failed,
                    Some(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostCallFailed,
                        detail,
                    }),
                )?;
            }
        }
        Ok(true)
    }

    fn complete_value(
        &mut self,
        scheduler: &mut InstalledScheduler,
        request: HostCallRequest,
        encoded: &[u8],
        maximum: u32,
        realization: InputRealization,
        context: InputEventContext,
    ) -> Result<(), String> {
        let value = scheduler
            .store_host_value(encoded)
            .map_err(|error| format!("store portable input event: {error:?}"))?;
        self.complete_stored_value(scheduler, request, value, maximum, realization, context)
    }

    fn complete_stored_value(
        &mut self,
        scheduler: &mut InstalledScheduler,
        request: HostCallRequest,
        value: conduit_kernel::ValueRef,
        maximum: u32,
        realization: InputRealization,
        context: InputEventContext,
    ) -> Result<(), String> {
        let descriptor = match &self.request_admission {
            Some(admission) if admission.completions.enabled() => {
                admission.completions.require_capacity(1)?;
                Some(admission.completed_origin_index(realization, request)?)
            }
            _ => None,
        };
        let output = BoundedValueRef::new(value, maximum)
            .map_err(|error| format!("bound portable input event: {error:?}"))?;
        scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete keyboard-backed input operation: {error:?}"))?;
        if let Some(descriptor) = descriptor {
            self.request_admission
                .as_mut()
                .ok_or("completed input origin lost")?
                .completions
                .record(CompletedInputEvent {
                    descriptor,
                    realization,
                    request,
                    value,
                    physical_event: context.physical_event,
                    input_view_epoch: context.input_view_epoch,
                })?;
        }
        Ok(())
    }

    fn complete_all(
        &mut self,
        scheduler: &mut InstalledScheduler,
        disposition: HostCallDisposition,
        failure: Option<conduit_kernel::Failure>,
    ) -> Result<(), String> {
        self.terminal = true;
        self.input_view_epoch = None;
        if let Some(admission) = &mut self.request_admission {
            admission.revoke();
        }
        for request in [self.pending_keyboard.take(), self.pending_button.take()]
            .into_iter()
            .flatten()
        {
            scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition,
                        output: None,
                        failure,
                    },
                )
                .map_err(|error| format!("complete keyboard-backed input operation: {error:?}"))?;
        }
        Ok(())
    }

    fn complete_failure(
        &mut self,
        scheduler: &mut InstalledScheduler,
        request: HostCallRequest,
        code: conduit_kernel::FailureCode,
        detail: u16,
    ) -> Result<bool, String> {
        self.cancel();
        scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Failed,
                    output: None,
                    failure: Some(conduit_kernel::Failure { code, detail }),
                },
            )
            .map_err(|error| format!("fail keyboard-backed input operation: {error:?}"))?;
        Ok(true)
    }
}

impl Drop for KeyboardInputHost<'_> {
    fn drop(&mut self) {
        self.cancel();
    }
}

#[cfg(test)]
#[path = "keyboard_input_host_tests.rs"]
mod tests;
