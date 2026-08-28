//! Adapter-side completion of exact installed keyboard host requests.

use super::InstalledScheduler;
use conduit_kernel::scheduler::HostOperationRequest;
use conduit_kernel::{BoundedValueRef, HostOperationDisposition, HostOperationOutcome};

pub(super) struct KeyboardInputHost<'a> {
    adapter: Option<&'a mut dyn crate::hosted_keyboard::HostedKeyboardAdapter>,
    pending: Option<HostOperationRequest>,
}

impl<'a> KeyboardInputHost<'a> {
    pub(super) fn new(
        adapter: Option<&'a mut dyn crate::hosted_keyboard::HostedKeyboardAdapter>,
    ) -> Self {
        Self {
            adapter,
            pending: None,
        }
    }

    pub(super) fn accept(
        &mut self,
        request: HostOperationRequest,
        input: &[u8],
    ) -> Result<(), String> {
        if !input.is_empty() {
            return Err("keyboard input request carries unexpected bytes".into());
        }
        if self.adapter.is_none() {
            return Err("planned keyboard has no admitted Host adapter".into());
        }
        if self.pending.replace(request).is_some() {
            return Err("keyboard source has two pending host requests".into());
        }
        Ok(())
    }

    pub(super) fn cancel(&mut self) {
        self.pending = None;
    }

    pub(super) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }

    pub(super) fn poll(&mut self, scheduler: &mut InstalledScheduler) -> Result<bool, String> {
        let Some(request) = self.pending else {
            return Ok(false);
        };
        let adapter = self
            .adapter
            .as_deref_mut()
            .ok_or_else(|| "pending keyboard request lost its Host adapter".to_string())?;
        let outcome = match adapter.poll_next() {
            crate::hosted_keyboard::HostedKeyboardPoll::Pending => return Ok(false),
            crate::hosted_keyboard::HostedKeyboardPoll::Event(event) => {
                let value = scheduler
                    .store_host_value(&event.encode())
                    .map_err(|error| format!("store portable keyboard event: {error:?}"))?;
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(value, conduit_human::KEY_EVENT_ENCODED_LEN as u32)
                            .map_err(|error| format!("bound portable keyboard event: {error:?}"))?,
                    ),
                    failure: None,
                }
            }
            crate::hosted_keyboard::HostedKeyboardPoll::Cancelled => HostOperationOutcome {
                disposition: HostOperationDisposition::Cancelled,
                output: None,
                failure: None,
            },
            crate::hosted_keyboard::HostedKeyboardPoll::Failed(detail) => HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(conduit_kernel::Failure {
                    code: conduit_kernel::FailureCode::HostOperationFailed,
                    detail,
                }),
            },
        };
        scheduler
            .complete_host_operation(request.node, request.request, outcome)
            .map_err(|error| format!("complete keyboard host operation: {error:?}"))?;
        self.pending = None;
        Ok(true)
    }
}
