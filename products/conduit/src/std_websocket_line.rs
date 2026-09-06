//! Bounded machine-only execution of an exact two-std-Host WebSocket Line.

use conduit_core::{
    BaseImplementationId, BaseInstanceId, LineAvailability, LineAvailabilitySign, LineContinuation,
    LineContract, LineDuplex, LineId, LineOffer, LineOrdering, LineReliability, LineScope,
    LineSecurity, LineTrafficShape, LinkAuthorityReference, LinkBinding, LinkBindingId,
    LinkCredentialReference, LinkEndpoint, LinkEndpointId, LinkLimits, Plan, SignId,
};
#[cfg(test)]
use conduit_core::{BootId, CapabilityId, GearId, HostId};
use conduit_kernel::scheduler::{
    FixedScheduler, StepInputBytes, StepIo, StepOperation, StepOutcome,
};
use conduit_kernel::{
    FixedRoutes, FixedSignLog, FixedValueStore, PortId, SignQuery, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{lower_plan_fragment, RemoteCordDirection};
use conduit_signal::{encode_signal, Signal, SIGNAL_ENCODED_LEN, SIGNAL_ENCODED_LEN_USIZE};
use conduit_std_host::websocket::NativeWebSocketListener;
use conduit_wire::{
    SessionBinding, SessionMachine, SessionMessage, SessionRole, SessionTerminalDisposition,
};
#[cfg(test)]
use std::collections::BTreeMap;
use std::thread;
use std::{io::Write, vec::Vec};

mod peer;
use peer::{activate_source, receive, send};

#[cfg(test)]
pub(crate) const SOURCE_HOST: &str = "product/std-source";
#[cfg(test)]
pub(crate) const SINK_HOST: &str = "product/std-sink";
#[cfg(test)]
const SOURCE_BOOT: &str = "product/std-source/boot-1";
#[cfg(test)]
const SINK_BOOT: &str = "product/std-sink/boot-1";
const MAXIMUM_VALUES: usize = 16;
pub(crate) const PRODUCT_WEBSOCKET_MAXIMUM_FRAME_BYTES: u32 = 2_048;
const MAXIMUM_FRAME_BYTES: u32 = PRODUCT_WEBSOCKET_MAXIMUM_FRAME_BYTES;
const PORTS: usize = conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub(crate) struct ExecutionEvidence {
    pub(crate) received: usize,
    pub(crate) pressure_retries: usize,
}

pub(crate) struct ProductWebSocketRuntime;

impl crate::product_execution::ProductLineRuntime for ProductWebSocketRuntime {
    fn supports(&self, plan: &Plan) -> bool {
        let lines = plan
            .fragments
            .iter()
            .flat_map(|fragment| &fragment.connections)
            .filter_map(|connection| connection.selected_line.as_ref())
            .collect::<Vec<_>>();
        !lines.is_empty()
            && lines.iter().all(|line| {
                line.binding.base == BaseImplementationId::from("conduit.base/websocket-rfc6455@1")
            })
    }

    fn execute(
        &mut self,
        plan: &Plan,
        output: &mut dyn Write,
    ) -> Result<Vec<conduit_core::Observation>, String> {
        let evidence = execute(plan)?;
        writeln!(
            output,
            "Body Line complete values={} pressure_retries={}",
            evidence.received, evidence.pressure_retries
        )
        .map_err(|error| error.to_string())?;
        Ok(Vec::new())
    }
}

pub(super) type Kernel = FixedScheduler<
    Driver,
    FixedValueStore<MAXIMUM_VALUES, { MAXIMUM_VALUES * SIGNAL_ENCODED_LEN_USIZE }>,
    FixedSignLog<128>,
    1,
    1,
    PORTS,
    1,
    PORTS,
    1,
>;

#[derive(Clone, Copy)]
pub(super) enum Driver {
    Source {
        values: [Option<ValueRef>; MAXIMUM_VALUES],
        next: usize,
    },
    Sink {
        received: usize,
    },
}

impl StepOperation<PORTS> for Driver {
    fn step(&mut self, io: &mut StepIo<PORTS>, _input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source { values, next } => {
                let Some(value) = values.get(*next).copied().flatten() else {
                    return StepOutcome::Complete;
                };
                if !io.output_ready(PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.send(PortId(0), value).is_err() {
                    return StepOutcome::Fail(conduit_kernel::Failure {
                        code: conduit_kernel::FailureCode::InvalidLifecycle,
                        detail: 1,
                    });
                }
                *next += 1;
                StepOutcome::Progress
            }
            Self::Sink { received } => {
                if io.input(PortId(0)).is_some() {
                    if io.consume(PortId(0)).is_err() {
                        return StepOutcome::Fail(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::InvalidLifecycle,
                            detail: 2,
                        });
                    }
                    *received += 1;
                    StepOutcome::Progress
                } else if io.input_closed(PortId(0)) {
                    if io.consume_closed(PortId(0)).is_err() {
                        return StepOutcome::Fail(conduit_kernel::Failure {
                            code: conduit_kernel::FailureCode::InvalidLifecycle,
                            detail: 3,
                        });
                    }
                    StepOutcome::Complete
                } else {
                    StepOutcome::Await
                }
            }
        }
    }
}

#[cfg(test)]
pub(crate) fn host(name: &str) -> conduit_std_host::StdHost {
    let boot = match name {
        SOURCE_HOST => SOURCE_BOOT,
        SINK_HOST => SINK_BOOT,
        _ => "product/unknown/boot",
    };
    conduit_std_host::StdHost::new_with_config(conduit_std_host::StdHostConfig {
        host_id: HostId::from(name),
        boot_id: BootId::from(boot),
        offer_generation: conduit_core::OfferGeneration(1),
    })
}

#[cfg(test)]
pub(crate) fn context() -> Result<crate::product_execution::ProductExecutionContext, String> {
    let source = host(SOURCE_HOST);
    let sink = host(SINK_HOST);
    let offer = line_offer(&source, &sink);
    crate::product_execution::ProductExecutionContext::new(
        vec![source.advertisement().clone(), sink.advertisement().clone()],
        vec![
            crate::product_execution::ProductRuntime::std(source),
            crate::product_execution::ProductRuntime::std(sink),
        ],
        vec![BaseImplementationId::from(
            "conduit.base/websocket-rfc6455@1",
        )],
        vec![offer],
        Vec::new(),
    )
}

#[cfg(test)]
pub(crate) fn placements(
    form: &conduit_form::ExpandedCanonicalForm,
) -> Result<conduit_planner::PlacementChoices, String> {
    let mut by_gear = BTreeMap::new();
    for gear in &form.gears {
        let (host_id, capability_id) = match gear.kind_id.as_str() {
            conduit_signal::PULSE_KIND => (SOURCE_HOST, "pulse-1"),
            conduit_signal::SHOW_KIND => (SINK_HOST, "stdout-show-1"),
            other => return Err(format!("test Host pair does not implement kind '{other}'")),
        };
        by_gear.insert(
            GearId::from(gear.gear_id.as_str()),
            conduit_planner::PlacementChoice {
                host_id: HostId::from(host_id),
                capability_id: CapabilityId::from(capability_id),
            },
        );
    }
    Ok(conduit_planner::PlacementChoices { by_gear })
}

pub(crate) fn line_offer(
    source: &conduit_std_host::StdHost,
    sink: &conduit_std_host::StdHost,
) -> LineOffer {
    let line_id = LineId::from(format!(
        "body-line/{}/{}",
        source.advertisement().host_id.as_str(),
        sink.advertisement().host_id.as_str()
    ));
    let binding_id = LinkBindingId::from(format!("{}/binding", line_id.as_str()));
    LineOffer {
        line_id: line_id.clone(),
        binding: LinkBinding {
            binding_id: binding_id.clone(),
            source: LinkEndpoint {
                host_id: source.advertisement().host_id.clone(),
                boot_id: source.advertisement().boot_id.clone(),
                endpoint_id: LinkEndpointId::from("product/std-source/egress"),
            },
            sink: LinkEndpoint {
                host_id: sink.advertisement().host_id.clone(),
                boot_id: sink.advertisement().boot_id.clone(),
                endpoint_id: LinkEndpointId::from("product/std-sink/ingress"),
            },
            base: BaseImplementationId::from("conduit.base/websocket-rfc6455@1"),
            base_instance_id: BaseInstanceId::from("product/std-websocket/loopback-instance"),
            credential: LinkCredentialReference::None,
            authority: LinkAuthorityReference::ProcessOwned,
            limits: LinkLimits {
                maximum_in_flight_items: 1,
                maximum_payload_bytes: SIGNAL_ENCODED_LEN,
                maximum_buffered_bytes: SIGNAL_ENCODED_LEN,
                maximum_frame_bytes: MAXIMUM_FRAME_BYTES,
            },
        },
        contract: LineContract {
            scope: LineScope::LocalNetwork,
            traffic_shape: LineTrafficShape::Message,
            duplex: LineDuplex::FullDuplex,
            ordering: LineOrdering::Ordered,
            reliability: LineReliability::Reliable,
            continuation: LineContinuation::None,
            security: LineSecurity::PlaintextNetwork,
        },
        availability: LineAvailabilitySign {
            line_id,
            binding_id,
            availability: LineAvailability::Ready,
            sign_id: SignId::from("product/std-websocket/line-ready"),
        },
    }
}

pub(crate) fn execute(plan: &Plan) -> Result<ExecutionEvidence, String> {
    let lowered = plan
        .fragments
        .iter()
        .map(|fragment| {
            lower_plan_fragment(fragment)
                .map(|lowered| (fragment, lowered))
                .map_err(|error| format!("{error:?}"))
        })
        .collect::<Result<Vec<_>, _>>()?;
    let (source, source_lowered) = lowered
        .iter()
        .find(|(_, lowered)| {
            lowered
                .remote_endpoints
                .iter()
                .any(|endpoint| endpoint.direction == RemoteCordDirection::Egress)
        })
        .ok_or("source fragment missing")?;
    let (sink, sink_lowered) = lowered
        .iter()
        .find(|(_, lowered)| {
            lowered
                .remote_endpoints
                .iter()
                .any(|endpoint| endpoint.direction == RemoteCordDirection::Ingress)
        })
        .ok_or("sink fragment missing")?;
    let source_remote = source_lowered
        .remote_endpoints
        .first()
        .ok_or("source remote endpoint missing")?;
    let sink_remote = sink_lowered
        .remote_endpoints
        .first()
        .ok_or("sink remote endpoint missing")?;
    if source_remote.direction != RemoteCordDirection::Egress
        || sink_remote.direction != RemoteCordDirection::Ingress
    {
        return Err("planned Line directions are not exact".into());
    }
    let connection = source
        .connections
        .iter()
        .find(|candidate| candidate.connection_id == source_remote.connection_id)
        .ok_or("planned Line connection missing")?;
    let binding = SessionBinding::from_planned_connection(
        plan.plan_id.clone(),
        source.fragment_id.clone(),
        sink.fragment_id.clone(),
        connection,
    )
    .map_err(|error| format!("{error:?}"))?;
    let listener = NativeWebSocketListener::bind_loopback(MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("{error:?}"))?;
    let address = listener
        .local_addr()
        .map_err(|error| format!("{error:?}"))?;
    let url = listener.url().map_err(|error| format!("{error:?}"))?;
    let sink_binding = binding.clone();
    let sink_lowered = sink_lowered.clone();
    let source_lowered = source_lowered.clone();
    let sink_handle =
        thread::spawn(move || peer::run_sink(sink_lowered, sink_binding, address, &url));
    let source_result = run_source(source_lowered, binding, listener);
    let sink_result = sink_handle
        .join()
        .map_err(|_| "sink Host thread panicked".to_string())?;
    let pressure_retries = source_result?;
    Ok(ExecutionEvidence {
        received: sink_result?,
        pressure_retries,
    })
}

pub(super) fn kernel(
    lowered: &conduit_plan_lowering::lowering::LoweredPlanFragment,
    driver: Driver,
    mut values: FixedValueStore<MAXIMUM_VALUES, { MAXIMUM_VALUES * SIGNAL_ENCODED_LEN_USIZE }>,
) -> Result<Kernel, String> {
    let mut routes = FixedRoutes::<PORTS, 1>::new(PORTS as u16);
    for route in &lowered.routes {
        routes
            .install(
                route.source_node,
                route.source_port,
                route.range,
                &route.targets,
            )
            .map_err(|error| format!("{error:?}"))?;
    }
    routes.seal().map_err(|error| format!("{error:?}"))?;
    let signs = FixedSignLog::<128>::new_with_remote_storage(
        (128 * core::mem::size_of::<conduit_kernel::KernelEvent>()) as u32,
        128,
        conduit_kernel::remote_sign_storage_bytes(128).ok_or("remote Sign budget overflow")?,
    )
    .map_err(|error| format!("{error:?}"))?;
    let _ = &mut values;
    Kernel::new(
        lowered
            .node_specs
            .clone()
            .try_into()
            .map_err(|_| "node width")?,
        lowered
            .cords
            .iter()
            .map(|cord| cord.spec)
            .collect::<Vec<_>>()
            .try_into()
            .map_err(|_| "cord width")?,
        routes,
        [driver],
        values,
        signs,
    )
    .map_err(|error| format!("{error:?}"))
}

fn run_source(
    lowered: conduit_plan_lowering::lowering::LoweredPlanFragment,
    binding: SessionBinding,
    listener: NativeWebSocketListener,
) -> Result<usize, String> {
    let mut values =
        FixedValueStore::<MAXIMUM_VALUES, { MAXIMUM_VALUES * SIGNAL_ENCODED_LEN_USIZE }>::new(
            (MAXIMUM_VALUES * SIGNAL_ENCODED_LEN_USIZE) as u32,
        )
        .map_err(|error| format!("{error:?}"))?;
    let mut refs = [None; MAXIMUM_VALUES];
    for (sequence, slot) in refs.iter_mut().enumerate() {
        *slot = Some(
            values
                .store(
                    &encode_signal(&Signal {
                        sequence: sequence as u64,
                        level: sequence % 2 == 1,
                    })
                    .encoded,
                )
                .map_err(|error| format!("{error:?}"))?,
        );
    }
    let mut kernel = kernel(
        &lowered,
        Driver::Source {
            values: refs,
            next: 0,
        },
        values,
    )?;
    let remote = &lowered.remote_endpoints[0];
    let mut line = listener
        .accept()
        .map_err(|error| format!("peer absence: {error:?}"))?;
    let mut session = SessionMachine::new(binding.clone(), SessionRole::Source)
        .map_err(|error| format!("{error:?}"))?;
    activate_source(&mut session, &binding, &mut line)?;
    let mut frame = [0; MAXIMUM_FRAME_BYTES as usize];
    let mut pressure_seen = false;
    loop {
        kernel.step().map_err(|error| format!("{error:?}"))?;
        let Some(offer) = kernel
            .remote_egress_offer(remote.endpoint, remote.cord)
            .map_err(|error| format!("{error:?}"))?
        else {
            break;
        };
        let payload = kernel
            .host_value(offer.value)
            .map_err(|error| format!("{error:?}"))?;
        send(
            &mut session,
            &binding,
            &mut line,
            SessionMessage::Offered {
                sequence: offer.sequence,
                payload,
            },
            &mut frame,
        )?;
        let response = receive(&mut session, &mut line, &mut frame)?;
        if !pressure_seen {
            match response {
                SessionMessage::Pressure { sequence } if sequence == offer.sequence => {
                    pressure_seen = true;
                }
                other => return Err(format!("expected initial pressure, got {other:?}")),
            }
            send(
                &mut session,
                &binding,
                &mut line,
                SessionMessage::Offered {
                    sequence: offer.sequence,
                    payload,
                },
                &mut frame,
            )?;
            match receive(&mut session, &mut line, &mut frame)? {
                SessionMessage::Accepted { sequence } if sequence == offer.sequence => kernel
                    .remote_egress_accept(remote.endpoint, remote.cord, sequence)
                    .map_err(|error| format!("{error:?}"))?,
                other => return Err(format!("expected acceptance, got {other:?}")),
            }
        } else {
            match response {
                SessionMessage::Accepted { sequence } if sequence == offer.sequence => kernel
                    .remote_egress_accept(remote.endpoint, remote.cord, sequence)
                    .map_err(|error| format!("{error:?}"))?,
                other => return Err(format!("expected acceptance, got {other:?}")),
            }
        }
        match receive(&mut session, &mut line, &mut frame)? {
            SessionMessage::Delivered { sequence } if sequence == offer.sequence => kernel
                .remote_egress_delivered(remote.endpoint, remote.cord, sequence)
                .map_err(|error| format!("{error:?}"))?,
            other => return Err(format!("expected delivery, got {other:?}")),
        }
    }
    send(
        &mut session,
        &binding,
        &mut line,
        SessionMessage::InputClosed {
            final_sequence: MAXIMUM_VALUES as u64,
        },
        &mut frame,
    )?;
    send(
        &mut session,
        &binding,
        &mut line,
        SessionMessage::Terminal {
            disposition: SessionTerminalDisposition::Completed,
            final_sequence: MAXIMUM_VALUES as u64,
        },
        &mut frame,
    )?;
    let _ = receive(&mut session, &mut line, &mut frame)?;
    if !session.is_terminal()
        || !kernel
            .remote_egress_terminal(remote.endpoint, remote.cord)
            .map_err(|error| format!("{error:?}"))?
        || !kernel
            .signs()
            .contains_kind(conduit_kernel::KernelEventKind::RemoteValueDelivered)
    {
        return Err("source terminal invariants failed".into());
    }
    line.close().map_err(|error| format!("{error:?}"))?;
    Ok(usize::from(pressure_seen))
}
