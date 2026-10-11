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
    bind_active_play, kind_id, HostAdvertisement, HostCallContractId, PlanFragment,
};
use conduit_kernel::scheduler::{HostCallRequest, RemoteIngressOutcome};
use conduit_kernel::{
    BoundedValueRef, CordId, HostCallDisposition, HostCallOutcome, HostedSignLog, HostedValueStore,
    RemoteEndpointId,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection,
};
use conduit_wire::SessionMessage;

mod body_time_step;
mod vision;
mod voice;

fn semantic_data_refusal(detail: u16) -> HostCallOutcome {
    HostCallOutcome {
        disposition: HostCallDisposition::Denied,
        output: None,
        failure: Some(conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::HostCallDenied,
            detail,
        }),
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteValueTransfer {
    pub endpoint: RemoteEndpointId,
    pub cord: CordId,
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RemoteHostWork {
    pub request: HostCallRequest,
    pub contract_id: HostCallContractId,
    pub maximum_output_bytes: u32,
    pub input: Vec<u8>,
}

pub struct InstalledRemoteFragment {
    scheduler: InstalledScheduler,
    body_time_required: bool,
    lowered: LoweredPlanFragment,
    placements: Vec<conduit_core::PlannedGear>,
    whisper_languages: super::whisper_language::WhisperLanguages,
    sessions: RemoteCordSessions,
    text_output_buffer: Vec<u8>,
    model_output_buffer: Vec<u8>,
    recognized_turn_commit_hosts:
        Vec<Option<super::recognized_turn_commit_back::RecognizedTurnCommitHost>>,
    speech_window_hosts:
        Vec<Option<super::speech_recognition_adapter_back::SpeechWindowToClipHost>>,
    speech_result_stream_hosts:
        Vec<Option<super::speech_recognition_adapter_back::SpeechResultToEventStreamHost>>,
    generated_speech_commit_hosts:
        Vec<Option<super::generated_speech_commit_back::GeneratedSpeechCommitHost>>,
    body_chat_prompt_hosts: Vec<Option<super::body_chat_prompt_back::BodyChatPromptHost>>,
    vision_tracker: Option<crate::vision_tracker::LocalVisionTracker>,
    vision_request_sequence: u64,
    vision_active_play_id: String,
    vision_run_id: String,
    vision_clock_basis: String,
    pending_body_context: Option<HostCallRequest>,
    delivered_body_context: Option<[u8; 32]>,
    data_text_hosts: super::data_text_host::DataTextGenerationHosts,
}

impl InstalledRemoteFragment {
    pub fn prepare(
        advertisement: &HostAdvertisement,
        fragment: &PlanFragment,
        play_sequence: u64,
    ) -> Result<Self, String> {
        if advertisement.host_id != fragment.host_id || advertisement.boot_id != fragment.boot_id {
            return Err("remote fragment preparation requires its exact host and Boot".into());
        }
        if !supports(fragment) {
            return Err("remote fragment contains an uninstalled std implementation".into());
        }
        let lowered = lower_plan_fragment(fragment)
            .map_err(|error| format!("lower remote std fragment: {error:?}"))?;
        validate_profile(&lowered)?;
        let whisper_languages =
            super::whisper_language::WhisperLanguages::prepare(fragment, &lowered.identity)?;
        let sessions = RemoteCordSessions::prepare(fragment, &lowered)?;

        let mut value_items = 0_u16;
        let mut value_bytes = 0_u32;
        let mut maximum_value_bytes = super::TICK_ENCODED_LEN;
        let mut sign_items = 32_u16;
        for placement in &fragment.placements {
            let budget = preparation::back_budget(placement)?;
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
        let data_text_hosts = super::data_text_host::DataTextGenerationHosts::prepare(
            fragment,
            &lowered.identity,
            &play,
        )?;
        let drivers =
            preparation::prepare_operations(fragment, &lowered, &mut values, &play, None, None)?;
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
            super::recognized_turn_commit_back::prepare_hosts(fragment);
        let speech_window_hosts =
            super::speech_recognition_adapter_back::prepare_window_hosts(fragment);
        let speech_result_stream_hosts =
            super::speech_recognition_adapter_back::prepare_result_hosts(fragment);
        let generated_speech_commit_hosts =
            super::generated_speech_commit_back::prepare_hosts(fragment)?;
        let body_chat_prompt_hosts = super::body_chat_prompt_back::prepare_hosts(fragment);
        let vision_tracker = if fragment.placements.iter().any(|placement| {
            placement.implementation_id.as_str()
                == conduit_std_offers::LOCAL_VISION_TRACK_IMPLEMENTATION
        }) {
            Some(crate::vision_tracker::LocalVisionTracker::prepare(
                format!(
                    "{}/{}",
                    fragment.host_id.as_str(),
                    fragment.boot_id.as_str()
                ),
                format!("{}/vision-track", play.active_play_id.as_str()),
            )?)
        } else {
            None
        };
        let vision_run_id = String::with_capacity(play.active_play_id.as_str().len() + 64);
        let vision_active_play_id = play.active_play_id.as_str().to_string();
        let vision_clock_basis = format!("{}/monotonic", fragment.boot_id.as_str());
        Ok(Self {
            scheduler,
            body_time_required: false,
            lowered,
            placements: fragment.placements.clone(),
            whisper_languages,
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
            vision_tracker,
            vision_request_sequence: 0,
            vision_active_play_id,
            vision_run_id,
            vision_clock_basis,
            pending_body_context: None,
            delivered_body_context: None,
            data_text_hosts,
        })
    }

    pub fn sessions(&self) -> &RemoteCordSessions {
        &self.sessions
    }
    pub fn sessions_mut(&mut self) -> &mut RemoteCordSessions {
        &mut self.sessions
    }
    pub fn next_host_request(&mut self) -> Option<HostCallRequest> {
        self.scheduler.next_host_request()
    }
    pub fn describe_host_request(
        &self,
        request: HostCallRequest,
    ) -> Result<RemoteHostWork, String> {
        let operation = self
            .lowered
            .host_calls
            .iter()
            .find(|operation| operation.node == request.node && operation.call == request.call)
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
    pub fn complete_host_call(
        &mut self,
        request: HostCallRequest,
        outcome: HostCallOutcome,
    ) -> Result<(), String> {
        self.scheduler
            .complete_host_call(request.node, request.request, outcome)
            .map_err(|error| format!("complete remote std host-call: {error:?}"))
    }

    pub fn complete_portable_host_call(
        &mut self,
        request: HostCallRequest,
    ) -> Result<bool, String> {
        let operation = self
            .lowered
            .host_calls
            .iter()
            .find(|operation| operation.node == request.node && operation.call == request.call)
            .ok_or_else(|| "remote host request has no lowered contract identity".to_string())?;
        let contract = operation.contract_id.as_str();
        let maximum_output_bytes = operation.binding.maximum_output_bytes;
        let input = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| format!("read remote std text input: {error:?}"))?;
        if contract == conduit_std_offers::BODY_CONVERSATION_CONTEXT_OPERATION {
            if !input.is_empty() {
                return Err("remote body context request carries unexpected bytes".into());
            }
            if self.pending_body_context.replace(request).is_some() {
                return Err("remote body context has two pending requests".into());
            }
            return Ok(true);
        }
        if matches!(
            contract,
            conduit_std_offers::DATA_SAVE_TEXT_HOST_CALL
                | conduit_std_offers::DATA_LOAD_TEXT_HOST_CALL
        ) {
            let operation = if contract == conduit_std_offers::DATA_SAVE_TEXT_HOST_CALL {
                super::data_text_back::DataTextOperation::Save
            } else {
                super::data_text_back::DataTextOperation::Load
            };
            let completion = self.data_text_hosts.execute(request.node, operation, input);
            let outcome = match completion {
                super::data_text_host::DataTextCompletion::Output(encoded) => {
                    let value = self
                        .scheduler
                        .store_host_value(encoded)
                        .map_err(|error| format!("store remote data Text output: {error:?}"))?;
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(BoundedValueRef::new(value, maximum_output_bytes).map_err(
                            |error| format!("bound remote data Text output: {error:?}"),
                        )?),
                        failure: None,
                    }
                }
                super::data_text_host::DataTextCompletion::SaveTerminal(terminal) => {
                    semantic_data_refusal(u16::from(terminal.encode()[0]))
                }
                super::data_text_host::DataTextCompletion::LoadTerminal(terminal) => {
                    semantic_data_refusal(u16::from(terminal.encode()[0]))
                }
                super::data_text_host::DataTextCompletion::Failed(detail) => HostCallOutcome {
                    disposition: HostCallDisposition::Failed,
                    output: None,
                    failure: Some(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::HostCallFailed,
                        detail,
                    }),
                },
            };
            self.scheduler
                .complete_host_call(request.node, request.request, outcome)
                .map_err(|error| format!("complete remote data Text operation: {error:?}"))?;
            return Ok(true);
        }
        if matches!(
            contract,
            conduit_std_offers::SPEECH_WINDOW_PUSH_OPERATION
                | conduit_std_offers::SPEECH_WINDOW_CLOSE_OPERATION
        ) {
            let host = self
                .speech_window_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote speech-window request has no admitted host".to_string())?;
            let output = if contract == conduit_std_offers::SPEECH_WINDOW_PUSH_OPERATION {
                host.push(input)?;
                None
            } else {
                Some(host.close()?)
            };
            let output = output
                .map(|bytes| self.scheduler.store_host_value(&bytes))
                .transpose()
                .map_err(|error| format!("store remote speech-window output: {error:?}"))?
                .map(|value| BoundedValueRef::new(value, maximum_output_bytes))
                .transpose()
                .map_err(|error| format!("bound remote speech-window output: {error:?}"))?;
            self.scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output,
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete remote speech-window operation: {error:?}"))?;
            return Ok(true);
        }
        if contract == conduit_std_offers::SPEECH_RESULT_TO_EVENT_OPERATION {
            let encoded = self
                .speech_result_stream_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote speech-result adapter has no admitted host".to_string())?
                .execute(input)?;
            let value = self
                .scheduler
                .store_host_value(&encoded)
                .map_err(|error| format!("store remote recognition event: {error:?}"))?;
            let output = BoundedValueRef::new(value, maximum_output_bytes)
                .map_err(|error| format!("bound remote recognition event: {error:?}"))?;
            self.scheduler
                .complete_host_call(
                    request.node,
                    request.request,
                    HostCallOutcome {
                        disposition: HostCallDisposition::Completed,
                        output: Some(output),
                        failure: None,
                    },
                )
                .map_err(|error| format!("complete remote speech-result adapter: {error:?}"))?;
            return Ok(true);
        }
        let (disposition, output) = if contract == conduit_std_offers::TEXT_UPPER_HOST_CALL_CONTRACT
            && operation.target_kind.as_ref()
                == Some(&kind_id(conduit_std_offers::TEXT_UPPER_HOST_CALL_TARGET))
        {
            super::text_backs::uppercase_utf8(input, &mut self.text_output_buffer)?;
            (
                HostCallDisposition::Completed,
                Some(self.text_output_buffer.as_slice()),
            )
        } else if contract == conduit_std_offers::RECOGNIZED_TURN_COMMIT_OPERATION {
            let output = self
                .recognized_turn_commit_hosts
                .get_mut(usize::from(request.node.0))
                .and_then(Option::as_mut)
                .ok_or_else(|| "remote recognized-turn request has no admitted host".to_string())?
                .execute(input)?;
            (HostCallDisposition::Completed, output)
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
            (HostCallDisposition::Completed, output)
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
                .ok_or_else(|| "remote body Chat request has no admitted host".to_string())?
                .execute(contract, input)?;
            (HostCallDisposition::Completed, output)
        } else if contract == conduit_std_offers::COMMITTED_TURN_TO_TEXT_OPERATION {
            match conduit_tongues::project_encoded_committed_turn_text(input) {
                Ok(text) => {
                    self.text_output_buffer.clear();
                    self.text_output_buffer.extend_from_slice(&text);
                    (
                        HostCallDisposition::Completed,
                        Some(self.text_output_buffer.as_slice()),
                    )
                }
                Err(_) => (HostCallDisposition::Denied, None),
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
                        HostCallDisposition::Completed,
                        Some(self.text_output_buffer.as_slice()),
                    )
                }
                Err(_) => (HostCallDisposition::Denied, None),
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
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition,
                    output,
                    failure: None,
                },
            )
            .map_err(|error| format!("complete remote portable host-call: {error:?}"))?;
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
            source.ok_or_else(|| "remote body context lost its admitted source".to_string())?;
        let poll = source.poll_after(self.delivered_body_context, |encoded| {
            self.scheduler.store_host_value(encoded)
        });
        let outcome = match poll {
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Pending => {
                return Ok(false)
            }
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Lost => {
                HostCallOutcome {
                    disposition: HostCallDisposition::Cancelled,
                    output: None,
                    failure: None,
                }
            }
            crate::hosted_body_conversation_context::BodyConversationContextPoll::Current {
                fingerprint,
                value,
            } => {
                self.delivered_body_context = Some(fingerprint);
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
                    output: Some(
                        BoundedValueRef::new(
                            value
                                .map_err(|error| format!("store remote body context: {error:?}"))?,
                            conduit_body::MAXIMUM_BODY_CONVERSATION_CONTEXT_BYTES as u32,
                        )
                        .map_err(|error| format!("bound remote body context: {error:?}"))?,
                    ),
                    failure: None,
                }
            }
        };
        self.pending_body_context = None;
        self.complete_host_call(request, outcome)?;
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
        self.close_ingress_with_disposition(
            endpoint,
            conduit_kernel::RemoteTerminalDisposition::NormalClose,
        )
    }
    pub fn close_ingress_with_disposition(
        &mut self,
        endpoint: RemoteEndpointId,
        disposition: conduit_kernel::RemoteTerminalDisposition,
    ) -> Result<(), String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Ingress)?;
        self.scheduler
            .close_remote_input_with_disposition(endpoint, cord, disposition)
            .map_err(|error| format!("close remote std input: {error:?}"))
    }
    pub fn close_ingress_abnormal(
        &mut self,
        endpoint: RemoteEndpointId,
        terminal: &[u8],
    ) -> Result<(), String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Ingress)?;
        let session = self
            .sessions
            .get(endpoint)
            .ok_or_else(|| "unknown remote std ingress session".to_string())?;
        validate_remote_abnormal(
            session.binding().abnormal_kind.as_ref(),
            session.binding().limits.maximum_payload_bytes,
            terminal,
        )?;
        let terminal = conduit_kernel::CanonicalValue::new(terminal)
            .map_err(|error| format!("bound remote std abnormal terminal: {error:?}"))?;
        self.scheduler
            .close_remote_input_abnormal(endpoint, cord, terminal)
            .map_err(|error| format!("close remote std input abnormally: {error:?}"))
    }
    pub fn egress_terminal(&mut self, endpoint: RemoteEndpointId) -> Result<bool, String> {
        Ok(self.egress_terminal_disposition(endpoint)?.is_some())
    }
    pub fn egress_terminal_disposition(
        &mut self,
        endpoint: RemoteEndpointId,
    ) -> Result<Option<conduit_kernel::RemoteTerminalDisposition>, String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Egress)?;
        self.scheduler
            .remote_egress_terminal_disposition(endpoint, cord)
            .map_err(|error| format!("complete remote std output: {error:?}"))
    }
    pub fn egress_abnormal_terminal(
        &self,
        endpoint: RemoteEndpointId,
    ) -> Result<Option<Vec<u8>>, String> {
        let cord = self.endpoint_cord(endpoint, RemoteCordDirection::Egress)?;
        self.scheduler
            .remote_egress_abnormal_terminal(endpoint, cord)
            .map(|terminal| terminal.map(|value| value.as_slice().to_vec()))
            .map_err(|error| format!("read remote std abnormal terminal: {error:?}"))
    }
    pub fn cancel(&mut self) -> Result<(), String> {
        self.pending_body_context = None;
        for host in self.speech_window_hosts.iter_mut().flatten() {
            host.cancel();
        }
        for host in self.speech_result_stream_hosts.iter_mut().flatten() {
            host.cancel();
        }
        for host in self.generated_speech_commit_hosts.iter_mut().flatten() {
            host.cancel();
        }
        self.model_output_buffer.clear();
        self.text_output_buffer.clear();
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

fn validate_remote_abnormal(
    abnormal_kind: Option<&conduit_core::KindId>,
    maximum_payload_bytes: u32,
    terminal: &[u8],
) -> Result<(), String> {
    let abnormal_kind = abnormal_kind
        .ok_or_else(|| "remote std ingress has no abnormal terminal kind".to_string())?;
    if conduit_core::primitive_info_kind(abnormal_kind.as_str()).is_none() {
        return Err(
            "remote std abnormal terminal kind has no installed exact validator".to_string(),
        );
    }
    if terminal.len()
        > usize::try_from(maximum_payload_bytes)
            .map_err(|_| "remote std abnormal terminal bound is not addressable".to_string())?
    {
        return Err("remote std abnormal terminal exceeds its planned bound".into());
    }
    conduit_core::validate_primitive_info(abnormal_kind.as_str(), terminal)
        .map_err(|_| "remote std abnormal terminal does not inhabit its planned kind".to_string())
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

#[cfg(test)]
mod terminal_tests {
    use super::*;

    #[test]
    fn remote_abnormal_truth_must_inhabit_the_exact_planned_kind_and_bound() {
        let unit = conduit_core::kind_id(conduit_core::EMPTY_INFO_ID);
        assert_eq!(validate_remote_abnormal(Some(&unit), 1, &[]), Ok(()));
        assert!(validate_remote_abnormal(Some(&unit), 1, &[0]).is_err());
        assert!(validate_remote_abnormal(Some(&unit), 0, &[0]).is_err());
        assert!(validate_remote_abnormal(None, 1, &[]).is_err());
        let domain = conduit_core::kind_id("domain/specific/fault");
        assert!(validate_remote_abnormal(Some(&domain), 8, b"bounded but untyped").is_err());
    }
}
