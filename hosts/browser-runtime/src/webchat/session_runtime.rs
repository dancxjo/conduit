use super::session::{BrowserChatEffect, BrowserChatSession, InteractionFrame};
use conduit_kernel::scheduler::{HostOperationRequest, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, HostOperationDisposition, HostOperationOutcome,
};
use conduit_presentation::{
    Manifestation, ManifestationLifecycle, PresentationInteraction,
    PresentationInteractionDisposition,
};

impl BrowserChatSession {
    pub(crate) fn effect(&self) -> BrowserChatEffect {
        self.current
            .and_then(|request| self.contract(request).ok())
            .map_or(BrowserChatEffect::None, |contract| match contract {
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_OPEN_HOST_OPERATION => {
                    BrowserChatEffect::SocketOpen
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION => {
                    BrowserChatEffect::SocketReceive
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_SEND_HOST_OPERATION => {
                    BrowserChatEffect::SocketSend
                }
                conduit_net::EXTERNAL_WEBSOCKET_CLIENT_CLOSE_HOST_OPERATION => {
                    BrowserChatEffect::SocketClose
                }
                conduit_chat::BROWSER_RENDER_HOST_OPERATION => BrowserChatEffect::Present,
                _ => BrowserChatEffect::None,
            })
    }

    pub(crate) fn effect_bytes(&self) -> &[u8] {
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
        let request = self.current.take().ok_or(-220)?;
        let output = if effect == BrowserChatEffect::SocketSend {
            Some(request.input)
        } else if effect == BrowserChatEffect::Present {
            let prepared = Manifestation::prepared(
                &self.presentation,
                &self.plan,
                self.active_play.clone(),
                self.renderer_placement.clone(),
                "chat/document".into(),
                "browser/document".into(),
                conduit_core::SignId::from("browser/presentation-prepared"),
            )
            .map_err(|_| -235)?;
            let available = prepared
                .transition(
                    ManifestationLifecycle::Available,
                    conduit_core::SignId::from("browser/presentation-available"),
                )
                .map_err(|_| -235)?;
            let bytes = serde_json::to_vec(&available).map_err(|_| -235)?;
            let value = self.scheduler.store_host_value(&bytes).map_err(|_| -235)?;
            self.manifestation = Some(available);
            self.interaction_text.clear();
            self.interaction_text.extend_from_slice(&bytes);
            Some(BoundedValueRef::new(value, 16 * 1024).map_err(|_| -235)?)
        } else {
            None
        };
        self.complete_request(request, HostOperationDisposition::Completed, output, None)?;
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
        self.complete_request(
            request,
            HostOperationDisposition::Completed,
            Some(output),
            None,
        )?;
        self.drive()
    }

    pub(crate) fn submit(&mut self, bytes: &[u8]) -> Result<(), i32> {
        if bytes.is_empty()
            || bytes.len() > 4_096
            || self.effect() != BrowserChatEffect::SocketReceive
        {
            return Err(-223);
        }
        if self.parked_input.is_none() {
            return Err(-224);
        }
        let frame: InteractionFrame = serde_json::from_slice(bytes).map_err(|_| -236)?;
        let manifestation = self.manifestation.as_ref().ok_or(-237)?;
        if frame.presentation_id != self.presentation.identity.as_str()
            || frame.presentation_revision != self.presentation.revision
        {
            return Err(-251);
        }
        if frame.manifestation_id != manifestation.manifestation_id.as_str() {
            return Err(-252);
        }
        let interaction = PresentationInteraction::new(
            &self.presentation,
            manifestation,
            &frame.input_id,
            &frame.action_id,
            &frame.target,
            &frame.value_kind,
            frame.value.as_bytes(),
            frame.sequence,
        )
        .map_err(interaction_refusal_code)?;
        self.interaction_ledger
            .admit(interaction.clone())
            .map_err(interaction_refusal_code)?;
        let encoded = interaction.encode();
        let value = self
            .scheduler
            .store_host_value(&encoded)
            .map_err(|_| -225)?;
        let output = BoundedValueRef::new(
            value,
            conduit_presentation::MAX_PRESENTATION_INTERACTION_BYTES as u32,
        )
        .map_err(|_| -225)?;
        let input_request = self.parked_input.take().ok_or(-224)?;
        self.complete_request(
            input_request,
            HostOperationDisposition::Completed,
            Some(output),
            None,
        )?;
        let receive = self.current.take().ok_or(-223)?;
        self.complete_request(
            receive,
            HostOperationDisposition::Cancelled,
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
            HostOperationDisposition::Cancelled,
            Some(output),
            Some(Failure {
                code: FailureCode::Cancelled,
                detail: 2,
            }),
        )?;
        if let Some(input) = self.parked_input.take() {
            self.complete_request(
                input,
                HostOperationDisposition::Cancelled,
                None,
                Some(Failure {
                    code: FailureCode::Cancelled,
                    detail: 2,
                }),
            )?;
        }
        self.disconnected = true;
        self.drive()
    }

    pub(super) fn drive(&mut self) -> Result<(), i32> {
        loop {
            while let Some(request) = self.scheduler.next_host_request() {
                self.identity
                    .bind_request(
                        &self.lowered_identity,
                        request.node,
                        request.request,
                        request.operation,
                    )
                    .map_err(|_| -227)?;
                let contract = self.contract(request)?.to_owned();
                if contract == conduit_chat::BROWSER_INTERACTION_HOST_OPERATION {
                    if self.parked_input.replace(request).is_some() {
                        return Err(-228);
                    }
                    continue;
                }
                if contract == conduit_net::EXTERNAL_WEBSOCKET_CLIENT_RECEIVE_HOST_OPERATION {
                    if self.parked_receive.replace(request).is_some() {
                        return Err(-233);
                    }
                    continue;
                }
                if contract == conduit_chat::CHAT_STATE_MESSAGE_HOST_OPERATION
                    || contract == conduit_chat::CHAT_STATE_CONNECTION_HOST_OPERATION
                {
                    let input = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|_| -232)?
                        .to_vec();
                    if contract == conduit_chat::CHAT_STATE_MESSAGE_HOST_OPERATION {
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
                    self.presentation = self.chat_state.presentation().map_err(|_| -239)?;
                    let bytes = serde_json::to_vec(&self.presentation).map_err(|_| -239)?;
                    let value = self.scheduler.store_host_value(&bytes).map_err(|_| -232)?;
                    let output = BoundedValueRef::new(
                        value,
                        conduit_presentation::MAX_PRESENTATION_TOTAL_BYTES as u32,
                    )
                    .map_err(|_| -232)?;
                    self.complete_request(
                        request,
                        HostOperationDisposition::Completed,
                        Some(output),
                        None,
                    )?;
                    continue;
                }
                if contract == conduit_chat::CHAT_SUBMIT_HOST_OPERATION {
                    let input = self
                        .scheduler
                        .host_value(request.input.value)
                        .map_err(|_| -232)?
                        .to_vec();
                    let interaction = PresentationInteraction::decode(&input).map_err(|_| -239)?;
                    let value = self
                        .scheduler
                        .store_host_value(&interaction.value)
                        .map_err(|_| -232)?;
                    let output =
                        BoundedValueRef::new(value, conduit_chat::MAXIMUM_CHAT_MESSAGE_BYTES)
                            .map_err(|_| -232)?;
                    let evidence = self
                        .interaction_ledger
                        .finish_front(PresentationInteractionDisposition::Accepted {
                            operation_request_id: format!("browser/request/{}", request.request.0),
                        })
                        .map_err(|_| -239)?;
                    let encoded_evidence = serde_json::to_vec(evidence).map_err(|_| -239)?;
                    self.evidence_text.clear();
                    self.evidence_text.extend_from_slice(&encoded_evidence);
                    self.complete_request(
                        request,
                        HostOperationDisposition::Completed,
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
                SchedulerStatus::Complete => {
                    self.complete = true;
                    return Ok(());
                }
                SchedulerStatus::Cancelled => return Err(-230),
            }
        }
    }

    fn contract(&self, request: HostOperationRequest) -> Result<&str, i32> {
        self.lowered_identity
            .host_operation_contract(request.node, request.operation)
            .map(|contract| contract.as_str())
            .ok_or(-231)
    }

    fn complete_request(
        &mut self,
        request: HostOperationRequest,
        disposition: HostOperationDisposition,
        output: Option<BoundedValueRef>,
        failure: Option<Failure>,
    ) -> Result<(), i32> {
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition,
                    output,
                    failure,
                },
            )
            .map_err(|_| -232)
    }
}

fn interaction_refusal_code(refusal: conduit_presentation::PresentationInteractionRefusal) -> i32 {
    use conduit_presentation::PresentationInteractionRefusal as R;
    match refusal {
        R::InvalidPresentation => -250,
        R::StalePresentation => -251,
        R::StaleManifestation => -252,
        R::FailedManifestation => -253,
        R::UnknownInput => -254,
        R::UnknownAction => -255,
        R::WrongTarget => -256,
        R::UnavailableAction => -257,
        R::RefusedAction => -258,
        R::WrongValueKind => -259,
        R::EmptyValue => -260,
        R::OversizeValue => -261,
        R::MalformedEncoding => -262,
        R::DuplicateDelivery => -263,
        R::QueuePressure => -264,
        R::EvidenceExhausted => -265,
    }
}
