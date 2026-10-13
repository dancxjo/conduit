//! Exact pending Play value custody, Fore delivery and retirement.
use super::*;

impl Play {
    pub(super) fn accept_instruction_request(
        &mut self,
        request: HostCallRequest,
        host: &HostAdvertisement,
    ) -> Result<(), TerminalError> {
        if self.instruction_request.is_some()
            || Some(request.input.value) != self.acknowledged_show_value
            || self
                .scheduler
                .host_value(request.input.value)
                .map_err(debug_error)?
                != serde_json::to_vec(&self.show).map_err(debug_error)?
        {
            return Err(error(
                "instruction Source did not receive the original acknowledged Show occurrence",
            ));
        }
        self.instruction_admission
            .as_mut()
            .ok_or_else(|| error("instruction Source has no admission"))?
            .accept(request, host)
            .map_err(error)?;
        self.instruction_request = Some(request);
        Ok(())
    }
    pub(super) fn complete_call(&mut self, bytes: Option<&[u8]>) -> Result<(), TerminalError> {
        let output = bytes
            .map(|bytes| {
                if bytes.len() > MAX_TERMINAL_VALUE_BYTES as usize {
                    return Err(TerminalError::DocumentPressure);
                }
                let value = self
                    .scheduler
                    .store_host_value(bytes)
                    .map_err(debug_error)?;
                BoundedValueRef::new(value, bytes.len() as u32).map_err(debug_error)
            })
            .transpose()?;
        if !self.awaiting_input && self.acknowledged_show_value.is_none() {
            self.acknowledged_show_value = output.map(|value| value.value);
        }
        self.scheduler
            .complete_host_call(
                self.request.node,
                self.request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output,
                    failure: None,
                },
            )
            .map_err(debug_error)
    }
    pub(super) fn take_output(
        &mut self,
        port: &LoweredForePort,
    ) -> Result<Option<Vec<u8>>, TerminalError> {
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(port.endpoint, port.cord)
            .map_err(debug_error)?
        else {
            return Ok(None);
        };
        let bytes = self
            .scheduler
            .host_value(offer.value)
            .map_err(debug_error)?
            .to_vec();
        self.scheduler
            .remote_egress_accept(port.endpoint, port.cord, offer.sequence)
            .map_err(debug_error)?;
        self.scheduler
            .remote_egress_delivered(port.endpoint, port.cord, offer.sequence)
            .map_err(debug_error)?;
        Ok(Some(bytes))
    }
    pub(super) fn finish(
        &mut self,
        expected: Option<&[u8]>,
    ) -> Result<Option<Vec<u8>>, TerminalError> {
        let mut observed = None;
        for _ in 0..64 {
            if self.scheduler.next_host_request().is_some() {
                return Err(error("unexpected terminal Host Call"));
            }
            if let Some(actual) = self.take_output(&self.interaction_fore.clone())? {
                if observed.is_some() || Some(actual.as_slice()) != expected {
                    return Err(error("terminal interaction Fore mismatch"));
                }
                observed = Some(actual);
            }
            match self.scheduler.step().map_err(debug_error)? {
                SchedulerStatus::Drained => {
                    if observed.is_some() != expected.is_some() {
                        return Err(error("terminal interaction delivery absent"));
                    }
                    for port in [&self.show_fore, &self.interaction_fore] {
                        if self
                            .scheduler
                            .remote_egress_terminal_disposition(port.endpoint, port.cord)
                            .map_err(debug_error)?
                            != Some(RemoteTerminalDisposition::NormalClose)
                        {
                            return Err(error("terminal Fore did not close normally"));
                        }
                    }
                    self.retired = true;
                    return Ok(observed);
                }
                SchedulerStatus::Progress { .. } => {}
                _ => return Err(error("terminal Mask failed to drain")),
            }
        }
        Err(error("terminal Mask drain work bound exhausted"))
    }
}

impl HostedTerminalMaskExecution {
    /// Internal Source occurrence only after the renderer's actual Show ACK.
    /// Returning this request does not authorize input or acknowledge its effect.
    pub(crate) fn pending_instruction_request(
        &mut self,
        host: &HostAdvertisement,
    ) -> Result<HostCallRequest, TerminalError> {
        self.validate_current_host(host)?;
        let play = self
            .pending
            .as_mut()
            .ok_or_else(|| error("instruction has no live Mask Play"))?;
        if !play.awaiting_input || play.instruction_admission.is_none() {
            return Err(error(
                "instruction requires an acknowledged current Parsec Show",
            ));
        }
        if let Some(request) = play.instruction_request {
            return Ok(request);
        }
        for _ in 0..64 {
            if let Some(request) = play.scheduler.next_host_request() {
                if Some(request.node) != play.instruction_node {
                    return Err(error("unexpected Host Call before instruction Source"));
                }
                play.accept_instruction_request(request, host)?;
                return Ok(request);
            }
            step(&mut play.scheduler)?;
        }
        Err(error("instruction Source request work bound exhausted"))
    }
}
