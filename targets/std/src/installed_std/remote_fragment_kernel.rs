//! Generic std-kernel preparation and remote-Cord lifecycle access.
//!
//! This owns no transport. An admitted Line driver moves the exact offered
//! bytes and reports acceptance/delivery through these bounded methods.

use super::{
    kernel_preparation::KernelTables, preparation, supports, InstalledScheduler, MAX_CORDS,
    MAX_NODES, MAX_QUEUE_SLOTS, ROUTE_SLOTS, ROUTE_TARGETS,
};
use crate::remote_cord_sessions::RemoteCordSessions;
use conduit_core::{
    bind_active_play, kind_id, HostAdvertisement, HostOperationContractId, PlanFragment,
};
use conduit_kernel::scheduler::{HostOperationRequest, RemoteIngressOutcome, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, CordId, HostOperationDisposition, HostOperationOutcome, HostedSignLog,
    HostedValueStore, RemoteEndpointId,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection,
};
use conduit_wire::SessionMessage;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteValueTransfer {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteHostWork {
    pub request: HostOperationRequest,
    pub contract_id: HostOperationContractId,
    pub maximum_output_bytes: u32,
    pub input: Vec<u8>,
}

pub struct InstalledRemoteFragment {
    scheduler: InstalledScheduler,
    lowered: LoweredPlanFragment,
    placements: Vec<conduit_core::PlannedGear>,
    sessions: RemoteCordSessions,
    text_output_buffer: Vec<u8>,
    model_output_buffer: Vec<u8>,
    recognized_turn_commit_hosts:
        Vec<Option<super::recognized_turn_commit_operation::RecognizedTurnCommitHost>>,
    speech_window_hosts:
        Vec<Option<super::speech_recognition_adapter_operation::SpeechWindowToClipHost>>,
    speech_result_stream_hosts:
        Vec<Option<super::speech_recognition_adapter_operation::SpeechResultToEventStreamHost>>,
    generated_speech_commit_hosts:
        Vec<Option<super::generated_speech_commit_operation::GeneratedSpeechCommitHost>>,
    body_chat_prompt_hosts: Vec<Option<super::body_chat_prompt_operation::BodyChatPromptHost>>,
    pending_body_context: Option<HostOperationRequest>,
    delivered_body_context: Option<[u8; 32]>,
}

impl InstalledRemoteFragment {
    pub fn prepare(
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        play_sequence: u64,
    ) -> Result<Self, String> {
        if advertisement.host_id != fragment.host_id || advertisement.boot_id != fragment.boot_id {
            return Err("remote fragment preparation requires its exact Host and Boot".into());
        }
        if !supports(fragment) {
            return Err("remote fragment contains an uninstalled std implementation".into());
        }
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower remote std fragment: {error:?}"))?;
        validate_profile(&lowered)?;
        let sessions = RemoteCordSessions::prepare(fragment, &lowered)?;

        let mut value_items = 0_u16;
        let mut value_bytes = 0_u32;
        let mut maximum_value_bytes = super::TICK_ENCODED_LEN;
        let mut sign_items = 32_u16;
        for placement in &fragment.placements {
            let budget = preparation::operation_budget(placement)?;
            value_items = value_items
                .checked_add(budget.value_items)
                .ok_or_else(|| "remote fragment value item budget overflow".to_string())?;
            value_bytes = value_bytes
                .checked_add(budget.value_bytes)
                .ok_or_else(|| "remote fragment value byte budget overflow".to_string())?;
            sign_items = sign_items
                .checked_add(budget.sign_items)
                .ok_or_else(|| "remote fragment Sign item budget overflow".to_string())?;
            maximum_value_bytes = maximum_value_bytes.max(budget.maximum_value_bytes);
        }
        for endpoint in &lowered.remote_endpoints {
            if endpoint.direction == RemoteCordDirection::Ingress {
                let cord = exact_cord(&lowered, endpoint.cord)?;
                value_items = value_items
                    .checked_add(cord.item_capacity)
                    .ok_or_else(|| "remote ingress value item budget overflow".to_string())?;
                value_bytes = value_bytes
                    .checked_add(cord.byte_capacity)
                    .ok_or_else(|| "remote ingress value byte budget overflow".to_string())?;
                maximum_value_bytes = maximum_value_bytes.max(cord.byte_capacity);
            }
        }
        let mut values =
            HostedValueStore::new(value_items.max(1), maximum_value_bytes, value_bytes.max(1))
                .map_err(|error| format!("remote fragment value store: {error:?}"))?;
        let play = bind_active_play(
            &fragment.plan_id,
            &fragment.host_id,
            &fragment.boot_id,
            play_sequence,
        );
        let drivers =
            preparation::prepare_operations(fragment, &lowered, &mut values, &play, None)?;
        let tables = KernelTables::prepare(&[&lowered])?;
        let sign_bytes = u32::from(sign_items)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "remote fragment Sign byte budget overflow".to_string())?;
        let remote_sign_items = remote_sign_capacity(&lowered)?;
        let remote_sign_bytes = conduit_kernel::remote_sign_storage_bytes(remote_sign_items)
            .ok_or_else(|| "remote fragment remote Sign byte budget overflow".to_string())?;
        let signs = HostedSignLog::new_with_remote_storage(
            sign_items,
            sign_bytes,
            remote_sign_items,
            remote_sign_bytes,
        )
        .map_err(|error| format!("remote fragment Sign store: {error:?}"))?;
        let scheduler = tables.install(drivers, values, signs)?;
        let recognized_turn_commit_hosts =
            super::recognized_turn_commit_operation::prepare_hosts(fragment);
        let speech_window_hosts =
            super::speech_recognition_adapter_operation::prepare_window_hosts(fragment);
        let speech_result_stream_hosts =
            super::speech_recognition_adapter_operation::prepare_result_hosts(fragment);
        let generated_speech_commit_hosts =
            super::generated_speech_commit_operation::prepare_hosts(fragment)?;
        let body_chat_prompt_hosts = super::body_chat_prompt_operation::prepare_hosts(fragment);
        Ok(Self {
            scheduler,
            lowered,
            placements: fragment.placements.clone(),
            sessions,
            text_output_buffer: Vec::with_capacity(super::contract::MAX_TEXT_BYTES as usize),
            model_output_buffer: Vec::with_capacity(
                conduit_ai::MAXIMUM_MODEL_RESULT_ENVELOPE_BYTES as usize,
            ),
            recognized_turn_commit_hosts,
            speech_window_hosts,
            speech_result_stream_hosts,
            generated_speech_commit_hosts,
            body_chat_prompt_hosts,
            pending_body_context: None,
            delivered_body_context: None,
        })
    }

    pub fn sessions(&self) -> &RemoteCordSessions {
        &self.sessions
    }
    pub fn sessions_mut(&mut self) -> &mut RemoteCordSessions {
        &mut self.sessions
    }
    pub fn step(&mut self) -> Result<SchedulerStatus, String> {
        self.scheduler
            .step()
            .map_err(|error| format!("step remote std fragment: {error:?}"))
    }
    pub fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }
    pub fn describe_host_request(
        &self,
        request: HostOperationRequest,
    ) -> Result<RemoteHostWork, String> {
        let operation = self
            .lowered
            .host_operations
            .iter()
            .find(|operation| {
                operation.node == request.node && operation.operation == request.operation
            })
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote std host input: {error:?}"))?
            .to_vec();
        Ok(RemoteHostWork {
            request,
            contract_id: operation.contract_id.clone(),
            maximum_output_bytes: operation.binding.maximum_output_bytes,
            input,
        })
    }
    pub fn complete_host_operation(
        &mut self,
        request: HostOperationRequest,
        outcome: HostOperationOutcome,
    ) -> Result<(), String> {
        self.scheduler
            .complete_host_operation(request.node, request.request, outcome)
            .map_err(|error| format!("complete remote std host operation: {error:?}"))
    }
    pub fn complete_portable_host_operation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<bool, String> {
        let operation = self
            .lowered
            .host_operations
            .iter()
            .find(|operation| {
                operation.node == request.node && operation.operation == request.operation
            })
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let contract = operation.contract_id.as_str();
        let maximum_output_bytes = operation.binding.maximum_output_bytes;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote std text input: {error:?}"))?;
        if contract == conduit_std_offers::BODY_CONVERSATION_CONTEXT_OPERATION {
            if !input.is_empty() {
                return Err("remote Body context request carries unexpected bytes".into());
            }
            if self.pending_body_context.replace(request).is_some() {
                return Err("remote Body context has two pending requests".into());
            }
            return Ok(true);
        }
        let (disposition, output) = if contract
            == conduit_std_offers::TEXT_UPPER_HOST_OPERATION_CONTRACT
            && operation.target_kind.as_ref()
                == Some(&kind_id(
                    conduit_std_offers::TEXT_UPPER_HOST_OPERATION_TARGET,
                )) {
            super::text_operations::uppercase_utf8(input, &mut self.text_output_buffer)?;
            (
                HostOperationDisposition::Completed,
                Some(self.text_output_buffer.as_slice()),
            )
        } else if contract == conduit_std_offers::RECOGNIZED_TURN_COMMIT_OPERATION {
            let output = self
                .recognized_turn_commit_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote recognized-turn request has no admitted host".to_string())?
                .execute(input)?;
            (HostOperationDisposition::Completed, output)
        } else if matches!(
            contract,
            conduit_std_offers::SPEECH_WINDOW_PUSH_OPERATION
                | conduit_std_offers::SPEECH_WINDOW_CLOSE_OPERATION
        ) {
            let host = self
                .speech_window_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote speech-window request has no admitted host".to_string())?;
            if contract == conduit_std_offers::SPEECH_WINDOW_PUSH_OPERATION {
                host.push(input)?;
                (HostOperationDisposition::Completed, None)
            } else {
                let output = host.close()?;
                (HostOperationDisposition::Completed, Some(output))
            }
        } else if contract == conduit_std_offers::SPEECH_RESULT_TO_EVENT_OPERATION {
            let output = self
                .speech_result_stream_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote speech-result adapter has no admitted host".to_string())?
                .execute(input)?;
            (HostOperationDisposition::Completed, Some(output))
        } else if matches!(
            contract,
            conduit_std_offers::GENERATED_SPEECH_PUSH_OPERATION
                | conduit_std_offers::GENERATED_SPEECH_DRAIN_OPERATION
                | conduit_std_offers::GENERATED_SPEECH_CLOSE_OPERATION
        ) {
            let output = self
                .generated_speech_commit_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote generated-speech request has no admitted host".to_string())?
                .execute(contract, input)?;
            (HostOperationDisposition::Completed, output)
        } else if matches!(
            contract,
            conduit_std_offers::BODY_CHAT_MESSAGE_OPERATION
                | conduit_std_offers::BODY_CHAT_RESPONSE_OPERATION
                | conduit_std_offers::BODY_CHAT_CONTEXT_OPERATION
        ) {
            let output = self
                .body_chat_prompt_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote Body Chat request has no admitted host".to_string())?
                .execute(contract, input)?;
            (HostOperationDisposition::Completed, output)
        } else if contract == conduit_std_offers::COMMITTED_TURN_TO_TEXT_OPERATION {
            match conduit_tongues::project_encoded_committed_turn_text(input) {
                Ok(text) => {
                    self.text_output_buffer.clear();
                    self.text_output_buffer.extend_from_slice(&text);
                    (
                        HostOperationDisposition::Completed,
                        Some(self.text_output_buffer.as_slice()),
                    )
                }
                Err(_) => (HostOperationDisposition::Denied, None),
            }
        } else if matches!(
            contract,
            conduit_std_offers::MODEL_RESULT_TO_TEXT_OPERATION
                | conduit_std_offers::GENERATED_CHUNK_TO_TEXT_OPERATION
        ) {
            let projected = if contract == conduit_std_offers::GENERATED_CHUNK_TO_TEXT_OPERATION {
                conduit_ai::project_encoded_generated_chunk_text(input)
            } else {
                conduit_ai::project_generated_text(input)
            };
            match projected {
                Ok(text) => {
                    self.text_output_buffer.clear();
                    self.text_output_buffer.extend_from_slice(&text);
                    (
                        HostOperationDisposition::Completed,
                        Some(self.text_output_buffer.as_slice()),
                    )
                }
                Err(_) => (HostOperationDisposition::Denied, None),
            }
        } else {
            return Ok(false);
        };
        let output = output
            .map(|bytes| self.scheduler.store_host_value(bytes))
            .transpose()
            .map_err(|error| format!("store remote portable host output: {error:?}"))?
            .map(|value| BoundedValueRef::new(value, maximum_output_bytes))
            .transpose()
            .map_err(|error| format!("bound remote portable host output: {error:?}"))?;
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition,
                    output,
                    failure: None,
                },
            )
            .map_err(|error| format!("complete remote portable host operation: {error:?}"))?;
        Ok(true)
    }
    pub(crate) fn poll_body_conversation_context(
        &mut self,
        source: Option<&crate::BodyConversationContextSource>,
    ) -> Result<bool, String> {
        let Some(request) = self.pending_body_context else {
            return Ok(false);
        };
        let source =
            source.ok_or_else(|| "remote Body context lost its admitted source".to_string())?;
        let poll = source.poll_after(self.delivered_body_context, |encoded| {
            self.scheduler.store_host_value(encoded)
        });
        let outcome = match poll {
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Pending => {
                return Ok(false)
            }
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Lost => {
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Cancelled,
                    output: None,
                    failure: None,
                }
            }
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Current {
                fingerprint,
                value,
            } => {
                self.delivered_body_context = Some(fingerprint);
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(
                            value
                                .map_err(|error| format!("store remote Body context: {error:?}"))?,
                            conduit_body::MAXIMUM_BODY_CONVERSATION_CONTEXT_BYTES as u32,
                        )
                        .map_err(|error| format!("bound remote Body context: {error:?}"))?,
                    ),
                    failure: None,
                }
            }
        };
        self.pending_body_context = None;
        self.complete_host_operation(request, outcome)?;
        Ok(true)
    }
    pub(crate) fn complete_voice_provider_host_operation<F>(
        &mut self,
        request: HostOperationRequest,
        speech_recognition: Option<&mut crate::hosted_speech_recognition::WhisperSpeechAdapter>,
        mut local_model: Option<
            &mut (dyn crate::hosted_local_model::HostedLocalModelAdapter + 'static),
        >,
        speech_synthesis: Option<&mut crate::hosted_speech::PiperSpeechAdapter>,
        cancelled: F,
    ) -> Result<bool, String>
    where
        F: Fn() -> bool + Copy,
    {
        let operation = self
            .lowered
            .host_operations
            .iter()
            .find(|operation| {
                operation.node == request.node && operation.operation == request.operation
            })
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let contract = operation.contract_id.as_str();
        let maximum_output_bytes = operation.binding.maximum_output_bytes;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote voice host input: {error:?}"))?;
        let outcome = if matches!(
            contract,
            conduit_std_offers::WHISPER_SPEECH_OPERATION
                | conduit_std_offers::WHISPER_CLIP_SPEECH_OPERATION
        ) {
            let recognition = if contract == conduit_std_offers::WHISPER_CLIP_SPEECH_OPERATION {
                super::whisper_speech_operation::execute_clip(speech_recognition, input, cancelled)
            } else {
                super::whisper_speech_operation::execute(speech_recognition, input, cancelled)
            };
            match recognition {
                Ok(encoded) => {
                    let value = self
                        .scheduler
                        .store_host_value(&encoded)
                        .map_err(|error| format!("store remote Whisper recognition: {error:?}"))?;
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output: Some(BoundedValueRef::new(value, maximum_output_bytes).map_err(
                            |error| format!("bound remote Whisper recognition: {error:?}"),
                        )?),
                        failure: None,
                    }
                }
                Err(failure) => super::whisper_speech_operation::failure_outcome(failure),
            }
        } else if matches!(
            contract,
            conduit_ai::GENERATE_TEXT_HOST_OPERATION | conduit_ai::LOCAL_MODEL_OPERATION
        ) {
            let placement = self
                .placements
                .get(usize::from(request.node.0))
                .ok_or_else(|| "remote model request has no exact placement".to_string())?;
            let completion = if cancelled() {
                if let Some(adapter) = local_model.as_mut() {
                    (*adapter).cancel_stream();
                }
                super::model_host::ModelHostCompletion::Cancelled
            } else {
                let completion = super::model_host::execute(
                    contract,
                    placement,
                    input,
                    local_model.as_mut().map(|adapter| {
                        &mut **adapter
                            as &mut (dyn crate::hosted_local_model::HostedLocalModelAdapter
                                      + 'static)
                    }),
                    &mut self.model_output_buffer,
                )?;
                if cancelled() {
                    if let Some(adapter) = local_model.as_mut() {
                        (*adapter).cancel_stream();
                    }
                    super::model_host::ModelHostCompletion::Cancelled
                } else {
                    completion
                }
            };
            let output = if completion.has_output() {
                let value = self
                    .scheduler
                    .store_host_value(&self.model_output_buffer)
                    .map_err(|error| format!("store remote model output: {error:?}"))?;
                Some(
                    BoundedValueRef::new(value, maximum_output_bytes)
                        .map_err(|error| format!("bound remote model output: {error:?}"))?,
                )
            } else {
                None
            };
            completion.outcome(output)
        } else if contract == conduit_std_offers::PIPER_SPEECH_OPERATION {
            let streaming = self
                .placements
                .get(usize::from(request.node.0))
                .is_some_and(|placement| {
                    placement.implementation_id.as_str()
                        == conduit_std_offers::PIPER_STREAMING_SPEECH_IMPLEMENTATION
                });
            match super::speech_synthesis_operation::execute_piper_cancellable(
                speech_synthesis,
                input,
                streaming,
                cancelled,
            ) {
                Ok(block) => {
                    let output = block
                        .map(|block| self.scheduler.store_host_value(block))
                        .transpose()
                        .map_err(|error| format!("store remote Piper block: {error:?}"))?
                        .map(|value| BoundedValueRef::new(value, maximum_output_bytes))
                        .transpose()
                        .map_err(|error| format!("bound remote Piper block: {error:?}"))?;
                    HostOperationOutcome {
                        disposition: HostOperationDisposition::Completed,
                        output,
                        failure: None,
                    }
                }
                Err(error) => {
                    let (disposition, failure) =
                        super::speech_synthesis_operation::piper_failure_outcome(error);
                    HostOperationOutcome {
                        disposition,
                        output: None,
                        failure: Some(failure),
                    }
                }
            }
        } else {
            return Ok(false);
        };
        self.complete_host_operation(request, outcome)?;
        Ok(true)
    }
    pub fn store_host_value(&mut self, bytes: &[u8]) -> Result<BoundedValueRef, String> {
        let value = self
            .scheduler
            .store_host_value(bytes)
            .map_err(|error| format!("store remote std host value: {error:?}"))?;
        let byte_len = u32::try_from(bytes.len())
            .map_err(|_| "remote std host value length overflow".to_string())?;
        BoundedValueRef::new(value, byte_len)
            .map_err(|error| format!("bound remote std host value: {error:?}"))
    }
    pub fn next_egress(
        &mut self,
        endpoint: RemoteEndpointId,
    ) -> Result<Option<RemoteValueTransfer>, String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Egress)?;
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(endpoint, cord)
            .map_err(|error| format!("offer remote std value: {error:?}"))?
        else {
            return Ok(None);
        };
        let bytes = self
            .scheduler
            .host_value(offer.value)
            .map_err(|error| format!("read remote std value: {error:?}"))?
            .to_vec();
        Ok(Some(RemoteValueTransfer {
            endpoint,
            cord,
            sequence: offer.sequence,
            bytes,
        }))
    }
    pub fn accept_egress(&mut self, transfer: &RemoteValueTransfer) -> Result<(), String> {
        self.scheduler
            .remote_egress_accept(transfer.endpoint, transfer.cord, transfer.sequence)
            .map_err(|error| format!("accept remote std value: {error:?}"))
    }
    pub fn deliver_egress(&mut self, transfer: &RemoteValueTransfer) -> Result<(), String> {
        self.scheduler
            .remote_egress_delivered(transfer.endpoint, transfer.cord, transfer.sequence)
            .map_err(|error| format!("deliver remote std value: {error:?}"))
    }
    pub fn admit_ingress(
        &mut self,
        endpoint: RemoteEndpointId,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Ingress)?;
        self.scheduler
            .admit_remote_input(endpoint, cord, sequence, bytes)
            .map_err(|error| format!("admit remote std value: {error:?}"))
    }
    pub fn close_ingress(&mut self, endpoint: RemoteEndpointId) -> Result<(), String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Ingress)?;
        self.scheduler
            .close_remote_input(endpoint, cord)
            .map_err(|error| format!("close remote std input: {error:?}"))
    }
    pub fn egress_terminal(&mut self, endpoint: RemoteEndpointId) -> Result<bool, String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Egress)?;
        self.scheduler
            .remote_egress_terminal(endpoint, cord)
            .map_err(|error| format!("complete remote std output: {error:?}"))
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.pending_body_context = None;
        for host in self.generated_speech_commit_hosts.iter_mut().flatten() {
            host.cancel();
        }
        self.scheduler
            .cancel()
            .map_err(|error| format!("cancel remote std fragment: {error:?}"))
    }

    /// Records an exact admitted Line failure before cancelling kernel work.
    /// A malformed zero code leaves both the session and kernel unchanged.
    pub fn fail_remote_line(
        &mut self,
        endpoint: RemoteEndpointId,
        code: u16,
    ) -> Result<(), String> {
        let session = self
            .sessions
            .get_mut(endpoint)
            .ok_or_else(|| "remote Line failure names no admitted endpoint".to_string())?;
        let binding = session.binding().clone();
        session
            .machine_mut()
            .admit_outbound(binding.frame(SessionMessage::Failed { code }))
            .map_err(|error| format!("record remote Line failure: {error:?}"))?;
        self.cancel()
    }
    fn endpoint_cord(
        &self,
        endpoint: RemoteEndpointId,
        direction: RemoteCordDirection,
    ) -> Result<CordId, String> {
        self.lowered
            .remote_endpoints
            .iter()
            .find(|candidate| candidate.endpoint == endpoint && candidate.direction == direction)
            .map(|candidate| candidate.cord)
            .ok_or_else(|| "remote endpoint direction or identity mismatch".into())
    }
}

fn exact_cord(
    lowered: &LoweredPlanFragment,
    cord: CordId,
) -> Result<conduit_kernel::scheduler::CordSpec, String> {
    lowered
        .cords
        .iter()
        .find(|candidate| candidate.spec.cord == cord)
        .map(|candidate| candidate.spec)
        .ok_or_else(|| "remote endpoint has no exact lowered Cord".into())
}

fn validate_profile(lowered: &LoweredPlanFragment) -> Result<(), String> {
    let route_targets = lowered
        .routes
        .iter()
        .map(|route| route.targets.len())
        .sum::<usize>();
    if lowered.nodes.is_empty()
        || lowered.nodes.len() > MAX_NODES
        || lowered.cords.is_empty()
        || lowered.cords.len() > MAX_CORDS
        || lowered.cord_value_slots as usize > MAX_QUEUE_SLOTS
        || lowered.routes.len() > ROUTE_SLOTS
        || route_targets > ROUTE_TARGETS
    {
        return Err("remote fragment exceeds the installed std kernel profile".into());
    }
    Ok(())
}

fn remote_sign_capacity(lowered: &LoweredPlanFragment) -> Result<u16, String> {
    lowered
        .remote_endpoints
        .iter()
        .try_fold(0_u16, |total, endpoint| {
            let items = exact_cord(lowered, endpoint.cord)?.item_capacity;
            let events = match endpoint.direction {
                RemoteCordDirection::Ingress => items.checked_add(1),
                RemoteCordDirection::Egress => {
                    items.checked_mul(3).and_then(|count| count.checked_add(1))
                }
            }
            .ok_or_else(|| "remote lifecycle Sign capacity overflow".to_string())?;
            total
                .checked_add(events)
                .ok_or_else(|| "remote lifecycle Sign capacity overflow".to_string())
        })
}
