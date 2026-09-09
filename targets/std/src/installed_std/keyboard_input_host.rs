//! Adapter-side completion of exact installed keyboard host requests.

use super::InstalledScheduler;
use conduit_kernel::scheduler::HostOperationRequest;
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome};

pub(super) struct KeyboardInputHost<'a> {
    adapter: Option<&'a mut dyn crate::hosted_keyboard::HostedKeyboardAdapter>,
    keyboard_source: bool,
    pending_keyboard: Option<HostOperationRequest>,
    pending_button: Option<HostOperationRequest>,
    button_encoder: conduit_semantic_catalog::PreparedButtonTransitionEncoder,
    button_held: bool,
    button_sequence: u64,
}

#[derive(Clone, Copy)]
pub(super) enum InputRequestKind {
    Keyboard,
    SpaceButton,
}

impl<'a> KeyboardInputHost<'a> {
    pub(super) fn new(
        adapter: Option<&'a mut dyn crate::hosted_keyboard::HostedKeyboardAdapter>,
        keyboard_source: bool,
    ) -> Self {
        Self {
            adapter,
            keyboard_source,
            pending_keyboard: None,
            pending_button: None,
            button_encoder: conduit_semantic_catalog::PreparedButtonTransitionEncoder::new(
                "button/primary",
            )
            .expect("reviewed button transition encoder"),
            button_held: false,
            button_sequence: 0,
        }
    }

    pub(super) fn accept(
        &mut self,
        request: HostOperationRequest,
        input: &[u8],
        kind: InputRequestKind,
    ) -> Result<(), String> {
        if !input.is_empty() {
            return Err("keyboard input request carries unexpected bytes".into());
        }
        if self.adapter.is_none() {
            return Err("planned keyboard has no admitted Host adapter".into());
        }
        let pending = match kind {
            InputRequestKind::Keyboard => &mut self.pending_keyboard,
            InputRequestKind::SpaceButton => &mut self.pending_button,
        };
        if pending.replace(request).is_some() {
            return Err("input source has two pending requests for one semantic port".into());
        }
        Ok(())
    }

    pub(super) fn cancel(&mut self) {
        self.pending_keyboard = None;
        self.pending_button = None;
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending_keyboard.is_some() || self.pending_button.is_some()
    }

    pub(super) fn poll(&mut self, scheduler: &mut InstalledScheduler) -> Result<bool, String> {
        if !self.is_pending() {
            return Ok(false);
        }
        if self.keyboard_source && self.pending_keyboard.is_none() {
            return Ok(false);
        }
        let adapter = self
            .adapter
            .as_deref_mut()
            .ok_or_else(|| "pending keyboard request lost its Host adapter".to_string())?;
        match adapter.poll_next() {
            crate::hosted_keyboard::HostedKeyboardPoll::Pending => return Ok(false),
            crate::hosted_keyboard::HostedKeyboardPoll::Event(event) => {
                if let Some(request) = self.pending_keyboard.take() {
                    let encoded = event.encode();
                    self.complete_value(
                        scheduler,
                        request,
                        &encoded,
                        conduit_human::KEY_EVENT_ENCODED_LEN as u32,
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
                        )?;
                    }
                }
                return Ok(true);
            }
            crate::hosted_keyboard::HostedKeyboardPoll::Cancelled => {
                self.complete_all(scheduler, HostOperationDisposition::Cancelled, None)?;
            }
            crate::hosted_keyboard::HostedKeyboardPoll::Failed(detail) => {
                self.complete_all(
                    scheduler,
                    HostOperationDisposition::Failed,
                    Some(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostOperationFailed,
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
        request: HostOperationRequest,
        encoded: &[u8],
        maximum: u32,
    ) -> Result<(), String> {
        let value = scheduler
            .store_host_value(encoded)
            .map_err(|error| format!("store portable input event: {error:?}"))?;
        self.complete_stored_value(scheduler, request, value, maximum)
    }

    fn complete_stored_value(
        &mut self,
        scheduler: &mut InstalledScheduler,
        request: HostOperationRequest,
        value: conduit_kernel::ValueRef,
        maximum: u32,
    ) -> Result<(), String> {
        let output = BoundedValueRef::new(value, maximum)
            .map_err(|error| format!("bound portable input event: {error:?}"))?;
        scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(output),
                    failure: None,
                },
            )
            .map_err(|error| format!("complete keyboard-backed input operation: {error:?}"))
    }

    fn complete_all(
        &mut self,
        scheduler: &mut InstalledScheduler,
        disposition: HostOperationDisposition,
        failure: Option<conduit_kernel::Failure>,
    ) -> Result<(), String> {
        for request in [self.pending_keyboard.take(), self.pending_button.take()]
            .into_iter()
            .flatten()
        {
            scheduler
                .complete_host_operation(
                    request.node,
                    request.request,
                    HostOperationOutcome {
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
        request: HostOperationRequest,
        code: conduit_kernel::FailureCode,
        detail: u16,
    ) -> Result<bool, String> {
        scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Failed,
                    output: None,
                    failure: Some(conduit_kernel::Failure { code, detail }),
                },
            )
            .map_err(|error| format!("fail keyboard-backed input operation: {error:?}"))?;
        Ok(true)
    }
}
