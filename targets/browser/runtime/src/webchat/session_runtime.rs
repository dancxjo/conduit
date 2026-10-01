use super::session::{BrowserChatEffect, BrowserChatSession, InteractionFrame};
use conduit_kernel::scheduler::{HostCallRequest, SchedulerStatus};
use conduit_kernel::{BoundedValueRef, Failure, FailureCode, HostCallDisposition, HostCallOutcome};
use conduit_presentation::{FaceInteraction, FaceInteractionDisposition};

impl BrowserChatSession {
    pub(crate) fn effect(&self) -> BrowserChatEffect {
        if self.pending_face.is_some() {
            return BrowserChatEffect::Present;
        }
        self.current
            .and_then(|request| self.contract(request).ok())
            .map_or(BrowserChatEffect::None, |contract| match contract {
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_OPEN_HOST_CALL => {
                    BrowserChatEffect::SocketOpen
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_CALL => {
                    BrowserChatEffect::SocketReceive
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_SEND_HOST_CALL => {
                    BrowserChatEffect::SocketSend
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_CLOSE_HOST_CALL => {
                    BrowserChatEffect::SocketClose
                }
                _ => BrowserChatEffect::None,
            })
    }

    pub(crate) fn effect_bytes(&self) -> &[u8] {
        if let Some(offer) = self.pending_face {
            return self.scheduler.host_value(offer.value).unwrap_or(&[]);
        }
        self.current
            .and_then(|request| self.scheduler.host_value(request.input.value).ok())
            .unwrap_or(&[])
    }

    pub(crate) fn identity_text(&self) -> &[u8] {
        &self.identity_text
    }

    pub(crate) fn interaction_text(&self) -> &[u8] {
        &self.interaction_text
    }

    pub(crate) fn evidence_text(&self) -> &[u8] {
        &self.evidence_text
    }

    pub(crate) fn status(&self) -> i32 {
        if self.error < 0 {
            self.error
        } else if self.complete {
            1
        } else {
            0
        }
    }

    pub(crate) fn disconnected(&self) -> bool {
        self.disconnected
    }

    pub(crate) fn capacity_stable(&self) -> bool {
        self.scheduler.values().allocation_capacities() == self.value_capacity
            && self.identity.allocation_capacities() == self.identity_capacity
    }

    pub(crate) fn request_count(&self) -> usize {
        self.identity.lengths().0
    }

    pub(crate) fn complete_simple(&mut self, effect: BrowserChatEffect) -> Result<(), i32> {
        if self.effect() != effect {
            return Err(-220);
        }
        if effect == BrowserChatEffect::Present {
            let offer = self.pending_face.take().ok_or(-220)?;
            let face: conduit_presentation::Presentation =
                serde_json::from_slice(self.scheduler.host_value(offer.value).map_err(|_| -235)?)
                    .map_err(|_| -235)?;
            let (mut mask, mask_effect) = crate::workspace_mask::BrowserMaskRuntime::prepare(
                self.body_id.clone(),
                self.active_play.host_id.clone(),
                self.active_play.boot_id.clone(),
                face,
                self.wake.clone(),
                self.body_plan.clone(),
            )
            .map_err(|_| -235)?;
            mask.acknowledge(&crate::workspace_mask::BrowserMaskAcknowledgement {
                show_id: mask_effect.show_id.clone(),
                manifestation_id: mask_effect.manifestation_id.clone(),
                mask_plan_id: mask_effect.mask_plan_id.clone(),
                active_play_id: mask_effect.mask_play.active_play_id.clone(),
                placement_id: mask_effect.placement_id.clone(),
                presentation_id: mask_effect.presentation_id.clone(),
                presentation_revision: mask_effect.presentation_revision,
            })
            .map_err(|_| -235)?;
            let bytes = serde_json::to_vec(&mask_effect).map_err(|_| -235)?;
            self.mask = Some(mask);
            self.interaction_text.clear();
            self.interaction_text.extend_from_slice(&bytes);
            self.scheduler
                .remote_egress_accept(offer.endpoint, offer.cord, offer.sequence)
                .map_err(|_| -235)?;
            self.scheduler
                .remote_egress_delivered(offer.endpoint, offer.cord, offer.sequence)
                .map_err(|_| -235)?;
            return self.drive();
        }
        let request = self.current.take().ok_or(-220)?;
        let output = if effect == BrowserChatEffect::SocketSend {
            Some(request.input)
        } else {
            None
        };
        self.complete_request(request, HostCallDisposition::Completed, output, None)?;
        self.drive()
    }

    pub(crate) fn receive(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if self.effect() != BrowserChatEffect::SocketReceive
            || bytes.len() > conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES as usize
        {
            return Err(-221);
        }
        let value = self.scheduler.store_host_value(bytes).map_err(|_| -222)?;
        let output =
            BoundedValueRef::new(value, conduit_net::MAXIMUM_EXTERNAL_WEBSOCKET_MESSAGE_BYTES)
                .map_err(|_| -222)?;
        let request = self.current.take().ok_or(-221)?;
        self.complete_request(request, HostCallDisposition::Completed, Some(output), None)?;
        self.drive()
    }

    pub(crate) fn submit(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if bytes.is_empty()
            || bytes.len() > 4_096
            || self.effect() != BrowserChatEffect::SocketReceive
        {
            return Err(-223);
        }
        let frame: InteractionFrame = serde_json::from_slice(bytes).map_err(|_| -236)?;
        let mask = self.mask.as_mut().ok_or(-237)?;
        let receipt = mask
            .interact(&crate::workspace_mask::BrowserMaskInteraction {
                show_id: frame.show_id,
                presentation_id: frame.presentation_id,
                presentation_revision: frame.presentation_revision,
                action_id: frame.action_id,
                target: frame.target,
                arguments: frame.arguments,
                sequence: frame.sequence,
            })
            .map_err(|_| -252)?;
        let interaction = receipt.correlation.interaction;
        self.interaction_ledger
            .admit(interaction.clone())
            .map_err(interaction_refusal_code)?;
        let encoded = interaction.encode();
        match self
            .scheduler
            .admit_remote_input(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
                frame.sequence,
                &encoded,
            )
            .map_err(|_| -225)?
        {
            conduit_kernel::scheduler::RemoteIngressOutcome::Accepted { sequence }
                if sequence == frame.sequence => {}
            _ => return Err(-225),
        }
        let receive = self.current.take().ok_or(-223)?;
        self.complete_request(
            receive,
            HostCallDisposition::Cancelled,
            None,
            Some(Failure {
                code: FailureCode::Cancelled,
                detail: 1,
            }),
        )?;
        self.drive()
    }

    pub(crate) fn disconnect(&mut self) -> Result<(), i32> {
        if self.effect() != BrowserChatEffect::SocketReceive {
            return Err(-226);
        }
        let receive = self.current.take().ok_or(-226)?;
        let value = self.scheduler.store_host_value(&[0]).map_err(|_| -226)?;
        let output = BoundedValueRef::new(value, 1).map_err(|_| -226)?;
        self.complete_request(
            receive,
            HostCallDisposition::Cancelled,
            Some(output),
            Some(Failure {
                code: FailureCode::Cancelled,
                detail: 2,
            }),
        )?;
        self.scheduler
            .close_remote_input(
                self.interaction_boundary.endpoint,
                self.interaction_boundary.cord,
            )
            .map_err(|_| -226)?;
        self.disconnected = true;
        self.drive()
    }

    pub(super) fn drive(&mut self) -> Result<(), i32> {
        loop {
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(self.face_boundary.endpoint, self.face_boundary.cord)
                .map_err(|_| -267)?
            {
                self.pending_face = Some(offer);
                return Ok(());
            }
            while let Some(request) = self.scheduler.next_host_request() {
                self.identity
                    .bind_request(
                        &self.lowered_identity,
                        request.node,
                        request.request,
                        request.call,
                    )
                    .map_err(|_| -227)?;
                let contract = self.contract(request)?.to_owned();
                if contract == conduit_net::EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_CALL {
                    if self.parked_receive.replace(request).is_some() {
                        return Err(-233);
                    }
                    continue;
                }
                if contract == conduit_chat::CHAT_STATE_MESSAGE_HOST_CALL
                    || contract == conduit_chat::CHAT_STATE_CONNECTION_HOST_CALL
                {
                    let input = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|_| -232)?
                        .to_vec();
                    if contract == conduit_chat::CHAT_STATE_MESSAGE_HOST_CALL {
                        self.chat_state.receive(&input).map_err(|_| -239)?;
                    } else {
                        self.chat_state
                            .set_connection(if input.first() == Some(&1) {
                                conduit_chat::ChatConnectionState::Connected
                            } else {
                                conduit_chat::ChatConnectionState::Disconnected
                            })
                            .map_err(|_| -239)?;
                    }
                    self.presentation = self
                        .chat_state
                        .presentation()
                        .map_err(|_| -239)?
                        .with_basis(self.presentation.basis.clone())
                        .map_err(|_| -239)?;
                    let bytes = serde_json::to_vec(&self.presentation).map_err(|_| -239)?;
                    let value = self.scheduler.store_host_value(&bytes).map_err(|_| -232)?;
                    let output = BoundedValueRef::new(
                        value,
                        conduit_presentation::MAX_PRESENTATION_TOTAL_BYTES as u32,
                    )
                    .map_err(|_| -232)?;
                    self.complete_request(
                        request,
                        HostCallDisposition::Completed,
                        Some(output),
                        None,
                    )?;
                    continue;
                }
                if contract == conduit_chat::CHAT_FROM_WEBSOCKET_HOST_CALL
                    || contract == conduit_chat::CHAT_TO_WEBSOCKET_HOST_CALL
                    || contract == conduit_chat::CHAT_CONNECTION_FROM_WEBSOCKET_HOST_CALL
                {
                    let bytes = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|_| -232)?
                        .to_vec();
                    core::str::from_utf8(&bytes).map_err(|_| -239)?;
                    let value = self.scheduler.store_host_value(&bytes).map_err(|_| -232)?;
                    let output =
                        BoundedValueRef::new(value, conduit_chat::MAXIMUM_CHAT_MESSAGE_BYTES)
                            .map_err(|_| -232)?;
                    self.complete_request(
                        request,
                        HostCallDisposition::Completed,
                        Some(output),
                        None,
                    )?;
                    continue;
                }
                if contract == conduit_chat::CHAT_SUBMIT_HOST_CALL {
                    let input = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|_| -232)?
                        .to_vec();
                    let interaction = FaceInteraction::decode(&input).map_err(|_| -239)?;
                    let argument = interaction
                        .arguments
                        .iter()
                        .find(|argument| argument.name == conduit_chat::CHAT_MESSAGE_INPUT)
                        .ok_or(-239)?;
                    let value = self
                        .scheduler
                        .store_host_value(&argument.value)
                        .map_err(|_| -232)?;
                    let output =
                        BoundedValueRef::new(value, conduit_chat::MAXIMUM_CHAT_MESSAGE_BYTES)
                            .map_err(|_| -232)?;
                    let evidence = self
                        .interaction_ledger
                        .finish_front(FaceInteractionDisposition::Accepted {
                            operation_request_id: format!("browser/request/{}", request.request.0),
                        })
                        .map_err(|_| -239)?;
                    let encoded_evidence = serde_json::to_vec(evidence).map_err(|_| -239)?;
                    self.evidence_text.clear();
                    self.evidence_text.extend_from_slice(&encoded_evidence);
                    self.complete_request(
                        request,
                        HostCallDisposition::Completed,
                        Some(output),
                        None,
                    )?;
                    continue;
                }
                self.current = Some(request);
                return Ok(());
            }
            match self.scheduler.step().map_err(|_| -229)? {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    if self.disconnected {
                        self.complete = true;
                    }
                    if let Some(receive) = self.parked_receive.take() {
                        self.current = Some(receive);
                    }
                    return Ok(());
                }
                SchedulerStatus::Drained => {
                    self.complete = true;
                    return Ok(());
                }
                SchedulerStatus::Cancelled => return Err(-230),
            }
        }
    }

    fn contract(&self, request: HostCallRequest) -> Result<&str, i32> {
        self.lowered_identity
            .host_call_contract(request.node, request.call)
            .map(|contract| contract.as_str())
            .ok_or(-231)
    }

    fn complete_request(
        &mut self,
        request: HostCallRequest,
        disposition: HostCallDisposition,
        output: Option<BoundedValueRef>,
        failure: Option<Failure>,
    ) -> Result<(), i32> {
        self.scheduler
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition,
                    output,
                    failure,
                },
            )
            .map_err(|_| -232)
    }
}

fn interaction_refusal_code(refusal: conduit_presentation::FaceInteractionRefusal) -> i32 {
    use conduit_presentation::FaceInteractionRefusal as R;
    match refusal {
        R::InvalidFace => -250,
        R::StaleFace => -251,
        R::StaleShow => -252,
        R::FailedShow => -253,
        R::NoQueuedInteraction => -254,
        R::UnknownAction => -255,
        R::WrongTarget => -256,
        R::UnavailableAction => -257,
        R::RefusedAction => -258,
        R::DuplicateArgument | R::MissingArgument | R::UnknownArgument => -254,
        R::WrongValueKind => -259,
        R::ViolatedConstraint => -260,
        R::OversizeValue => -261,
        R::MalformedEncoding => -262,
        R::DuplicateDelivery => -263,
        R::QueuePressure => -264,
        R::EvidenceExhausted => -265,
        R::ValidatorIncapacity => -266,
    }
}
