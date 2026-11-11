use super::*;
impl<S: NorStorage> RecordServiceHostCall<S> {
    pub fn invoke(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        input: &[u8],
        output: &mut [u8],
    ) -> Result<usize, ServiceRefusal> {
        use ServiceRefusal as E;
        let operation = self.check_binding(node, call)?;
        if self.terminal
            || self
                .operations
                .iter()
                .any(|op| op.inspection().lifecycle == c::CapabilityLifecycle::Revoked)
        {
            return Err(E::Terminal);
        }
        if output.len() < self.routes[operation].maximum_output_bytes as usize {
            return Err(E::OutputPressure);
        }
        if let Some(previous) = self.cached {
            if request == previous.request {
                if operation != previous.operation || input != &self.input[..previous.input_len] {
                    return Err(E::StaleRequest);
                }
                return Ok(self.copy_result(operation, output));
            }
            if !previous.committed {
                return Err(E::CompletionPending);
            }
        }
        if request != RequestId(self.next_request) || self.next_request == u32::MAX {
            return Err(E::StaleRequest);
        }
        if input.len() > self.routes[operation].maximum_input_bytes as usize {
            return Err(E::InvalidInput);
        }
        let requested_index = if operation == 1 {
            let value = c::decode_count(input).map_err(|_| E::InvalidInput)?;
            let index = u32::try_from(value).map_err(|_| E::InvalidInput)?;
            if let Some(remaining) = self.ready.provider.remaining_slots() {
                if index > self.ready.provider.total_slots() - remaining {
                    return Err(E::InvalidInput);
                }
            }
            Some(index)
        } else {
            None
        };
        let lease = self.operations[operation]
            .authorize()
            .map_err(E::Capability)?;
        // From here a failed codec/complete cannot authorize a second media attempt.
        self.terminal = true;
        self.input[..input.len()].copy_from_slice(input);
        let encoded = if operation == 0 {
            let outcome = if self.append_failed || !self.ready.provider.recovered() {
                AppendOutcome::RecoveryRequired
            } else if self.ready.provider.remaining_slots() == Some(0) {
                AppendOutcome::Capacity
            } else {
                self.custody = Some(AppendCustody {
                    request,
                    committed_index: None,
                    kernel_committed: false,
                });
                match self.ready.provider.append(input) {
                    Ok(index) => {
                        self.custody
                            .as_mut()
                            .expect("retained attempt")
                            .committed_index = Some(index);
                        AppendOutcome::Recorded(index)
                    }
                    Err(_) => {
                        self.append_failed = true;
                        AppendOutcome::Uncertain
                    }
                }
            };
            self.ready.append.encode(outcome).map(|bytes| bytes.len())
        } else {
            let index = requested_index.expect("validated read Count");
            let outcome = if let Some(remaining) = self.ready.provider.remaining_slots() {
                let captured_end = self.ready.provider.total_slots() - remaining;
                if index == captured_end {
                    ReadResult::End(captured_end)
                } else {
                    match self.ready.provider.read(DurableRecordIndex(index)) {
                        Ok(RecordRead::Committed { payload, .. }) => ReadResult::Recorded {
                            index,
                            captured_end,
                            record: payload,
                        },
                        Ok(RecordRead::Vacant { .. }) => ReadResult::Gap {
                            index,
                            captured_end,
                            reason: GapReason::Vacant,
                        },
                        Ok(RecordRead::Gap { reason, .. }) => ReadResult::Gap {
                            index,
                            captured_end,
                            reason: match reason {
                                RecordGap::UnpublishedOrDamaged => GapReason::UnpublishedOrDamaged,
                                RecordGap::Corrupt => GapReason::Corrupt,
                            },
                        },
                        Err(_) => ReadResult::Unavailable,
                    }
                }
            } else {
                ReadResult::RecoveryRequired
            };
            self.ready.read.encode(outcome).map(|bytes| bytes.len())
        };
        let length = encoded.as_ref().copied().unwrap_or(0) as u32;
        if let Err(error) = self.operations[operation].complete(lease, length) {
            let _ = self.operations[operation].revoke();
            return Err(E::Capability(error));
        }
        encoded.map_err(E::Codec)?;
        self.cached = Some(Cached {
            request,
            operation,
            input_len: input.len(),
            committed: false,
        });
        self.terminal = false;
        Ok(self.copy_result(operation, output))
    }
    /// Only the actual kernel's acceptance of this exact node/call/request and
    /// encoded outcome may invoke this. Neither media nor transport is that proof.
    pub fn acknowledge_kernel_completion(
        &mut self,
        node: NodeId,
        call: HostCallId,
        request: RequestId,
        encoded_outcome: &[u8],
    ) -> Result<(), ServiceRefusal> {
        let operation = self.check_binding(node, call)?;
        if self.terminal {
            return Err(ServiceRefusal::Terminal);
        }
        let previous = self.cached.ok_or(ServiceRefusal::StaleRequest)?;
        if previous.request != request
            || previous.operation != operation
            || self.encoded(operation) != encoded_outcome
        {
            return Err(ServiceRefusal::StaleRequest);
        }
        if !previous.committed {
            self.next_request = request
                .0
                .checked_add(1)
                .ok_or(ServiceRefusal::StaleRequest)?;
            self.cached.as_mut().expect("checked cache").committed = true;
            if operation == 0 {
                if let Some(custody) = self.custody.as_mut().filter(|c| c.request == request) {
                    custody.kernel_committed = true;
                }
            }
        }
        Ok(())
    }
    pub fn pending_outcome(&self) -> Option<(HostCallId, RequestId, &[u8], bool)> {
        let cached = self.cached?;
        Some((
            self.routes[cached.operation].call,
            cached.request,
            self.encoded(cached.operation),
            cached.committed,
        ))
    }
    fn encoded(&self, operation: usize) -> &[u8] {
        if operation == 0 {
            self.ready.append.encoded()
        } else {
            self.ready.read.encoded()
        }
    }
    fn copy_result(&self, operation: usize, output: &mut [u8]) -> usize {
        let bytes = self.encoded(operation);
        output[..bytes.len()].copy_from_slice(bytes);
        bytes.len()
    }
    fn check_binding(&self, node: NodeId, call: HostCallId) -> Result<usize, ServiceRefusal> {
        if node != self.node {
            return Err(ServiceRefusal::WrongBinding);
        }
        self.routes
            .iter()
            .position(|route| route.call == call)
            .ok_or(ServiceRefusal::WrongBinding)
    }
}
