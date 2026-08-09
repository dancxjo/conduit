use super::*;

type SinkScheduler = FixedScheduler<
    OperationDriver<RenderOperation, PORTS>,
    HostedValueStore,
    HostedClueLog,
    1,
    1,
    PORTS,
    1,
    PORTS,
    1,
    1,
    1,
>;

pub(super) struct Sink {
    scheduler: SinkScheduler,
    binding: SessionBinding,
    session: SessionMachine,
    endpoint: RemoteEndpointId,
    cord: CordId,
    execution: Option<RendererExecution>,
    renderer_plan: conduit_core::Plan,
}

impl Sink {
    pub(super) fn prepare(
        plan: conduit_core::Plan,
        fragment: &PlanFragment,
        _source: &PlanFragment,
    ) -> Result<Self, CrossHostRendererError> {
        let (lowered, binding, endpoint, cord) =
            lowered_remote(fragment, RemoteCordDirection::Ingress)?;
        if lowered.host_operations.len() != 1 {
            return Err(CrossHostRendererError::Plan(
                "renderer sink omitted its exact host operation".into(),
            ));
        }
        let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
        routes
            .seal()
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let mut host_bindings = FixedHostOperationBindings::<1>::new(1);
        host_bindings
            .install(
                lowered.host_operations[0].node,
                lowered.host_operations[0].binding,
            )
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        host_bindings
            .seal()
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let scheduler = SinkScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| CrossHostRendererError::Kernel("renderer node table width".into()))?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| CrossHostRendererError::Kernel("renderer Cord table width".into()))?,
            routes,
            host_bindings,
            [OperationDriver::new(RenderOperation { pending: None })
                .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?],
            HostedValueStore::new(1, MAX_RENDERER_VALUE_BYTES, MAX_RENDERER_VALUE_BYTES)
                .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?,
            clue_log()?,
        )
        .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let session = SessionMachine::new(binding.clone(), SessionRole::Sink)
            .map_err(|error| CrossHostRendererError::Session(format!("{error:?}")))?;
        let active_play =
            bind_active_play(&fragment.plan_id, &fragment.host_id, &fragment.boot_id, 0);
        if active_play.active_play_id != binding.sink_active_play_id {
            return Err(CrossHostRendererError::Session(
                "renderer Play identity disagrees with the planned session".into(),
            ));
        }
        Ok(Self {
            scheduler,
            binding,
            session,
            endpoint,
            cord,
            execution: None,
            renderer_plan: plan,
        })
    }

    pub(super) fn run(
        mut self,
        url: &str,
        identity: RendererAdapterIdentity,
    ) -> Result<RendererSnapshot, CrossHostRendererError> {
        let maximum = FRAME_BYTES;
        let config = WebSocketConfig::default()
            .read_buffer_size(maximum)
            .write_buffer_size(0)
            .max_write_buffer_size(maximum + 32)
            .max_message_size(Some(maximum))
            .max_frame_size(Some(maximum));
        let (mut line, _) = connect_with_config(url, Some(config), 0)
            .map_err(|error| CrossHostRendererError::Line(error.to_string()))?;
        let mut input = vec![0; FRAME_BYTES];
        let mut output = vec![0; FRAME_BYTES];
        let hello_binding = self.binding.clone();
        let hello = hello_binding.hello_frame().message;
        send_client(
            &mut line,
            &mut self.session,
            &self.binding,
            hello,
            &mut output,
        )?;
        expect_message(
            receive_client(&mut line, &mut self.session, &mut input)?,
            |message| matches!(message, SessionMessage::Hello(_)),
            "Hello",
        )?;
        send_client(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::Ready,
            &mut output,
        )?;
        expect_message(
            receive_client(&mut line, &mut self.session, &mut input)?,
            |message| matches!(message, SessionMessage::Ready),
            "Ready",
        )?;
        let offered = receive_client(&mut line, &mut self.session, &mut input)?;
        let (sequence, payload) = match offered {
            SessionMessage::Offered { sequence, payload } if sequence == 0 => {
                (sequence, payload.to_vec())
            }
            _ => {
                return Err(CrossHostRendererError::Session(
                    "renderer expected one offered Presentation".into(),
                ))
            }
        };
        match self
            .scheduler
            .admit_remote_input(self.endpoint, self.cord, sequence, &payload)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?
        {
            RemoteIngressOutcome::Accepted { .. } => {}
            RemoteIngressOutcome::Full { .. } => {
                return Err(CrossHostRendererError::Kernel(
                    "fresh renderer input was unexpectedly full".into(),
                ))
            }
        }
        send_client(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::Accepted { sequence },
            &mut output,
        )?;
        self.scheduler
            .step()
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let request = self.scheduler.next_host_request().ok_or_else(|| {
            CrossHostRendererError::Kernel("renderer host operation missing".into())
        })?;
        self.prepare_renderer(request, &payload, &identity)?;
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output: None,
                    failure: None,
                },
            )
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        self.scheduler
            .step()
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        send_client(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::Delivered { sequence },
            &mut output,
        )?;
        expect_message(
            receive_client(&mut line, &mut self.session, &mut input)?,
            |message| matches!(message, SessionMessage::InputClosed { final_sequence: 1 }),
            "input closure",
        )?;
        self.scheduler
            .close_remote_input(self.endpoint, self.cord)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        let mut completed = false;
        for _ in 0..3 {
            if self
                .scheduler
                .step()
                .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?
                == SchedulerStatus::Complete
            {
                completed = true;
                break;
            }
        }
        if !completed
            || self.scheduler.values().used_items() != 0
            || self
                .scheduler
                .cord_usage(self.cord)
                .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?
                != (0, 0)
        {
            return Err(CrossHostRendererError::Kernel(
                "renderer kernel did not finish with empty admitted storage".into(),
            ));
        }
        send_client(
            &mut line,
            &mut self.session,
            &self.binding,
            SessionMessage::Terminal {
                disposition: SessionTerminalDisposition::Completed,
                final_sequence: 1,
            },
            &mut output,
        )?;
        expect_message(
            receive_client(&mut line, &mut self.session, &mut input)?,
            |message| {
                matches!(
                    message,
                    SessionMessage::Terminal {
                        disposition: SessionTerminalDisposition::Completed,
                        final_sequence: 1
                    }
                )
            },
            "source completed terminal",
        )?;
        line.close(None)
            .map_err(|error| CrossHostRendererError::Line(error.to_string()))?;
        let execution = self.execution.ok_or_else(|| {
            CrossHostRendererError::Presentation(
                "renderer adapter did not create a Manifestation".into(),
            )
        })?;
        RendererSnapshot::from_execution(execution).map_err(Into::into)
    }

    fn prepare_renderer(
        &mut self,
        request: HostOperationRequest,
        payload: &[u8],
        identity: &RendererAdapterIdentity,
    ) -> Result<(), CrossHostRendererError> {
        let planned = self
            .scheduler
            .host_value(request.input.value)
            .map_err(|error| CrossHostRendererError::Kernel(format!("{error:?}")))?;
        if planned != payload {
            return Err(CrossHostRendererError::Kernel(
                "renderer host operation input drifted from admitted Info".into(),
            ));
        }
        let presentation = decode_presentation(planned)?;
        let renderer_fragment = fragment_for(&self.renderer_plan, &identity.host_id)?;
        if renderer_fragment.boot_id != identity.boot_id {
            return Err(CrossHostRendererError::Plan(
                "renderer adapter identity disagrees with its planned Host/Boot".into(),
            ));
        }
        self.execution = Some(
            RendererExecution::prepare_planned(
                presentation,
                self.renderer_plan.clone(),
                identity.target_subject.clone(),
                ClueId::from("patchbay-html/cross-host-prepared"),
            )
            .map_err(|error| CrossHostRendererError::Presentation(error.to_string()))?,
        );
        Ok(())
    }
}
