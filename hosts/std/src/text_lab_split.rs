//! Native production-kernel half of the exact split Text Lab Plan.

use conduit_core::{ConduitIntlKeymap, KeyEvent, KeyModifiers, KeyTransition, KeymapDisposition};
use conduit_kernel::scheduler::{FixedScheduler, OperationDriver, SchedulerStatus};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes,
    HostOperationDisposition, HostOperationId, HostOperationOutcome, HostedSignLog,
    HostedValueStore, Operation, OperationAction, OperationInput, PortId, RemoteEndpointId,
    RequestId, ValueRef, ValueStorage,
};
use conduit_runtime::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection, MAXIMUM_KERNEL_PORTS_PER_NODE,
};
use conduit_std_catalog::{
    exact_text_lab_split_plan, KEYBOARD_KIND, KEYMAP_KIND, TEXT_LAB_MAXIMUM_VALUES,
    TEXT_LAB_NATIVE_HOST, TEXT_PRESENTATION_KIND,
};
use conduit_text::MAX_TEXT_BYTES;

const PORTS: usize = MAXIMUM_KERNEL_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = 3 * PORTS;
const SIGN_ITEMS: u16 = 192;

type NativeTextLabScheduler = FixedScheduler<
    OperationDriver<NativeOperation, PORTS>,
    HostedValueStore,
    HostedSignLog,
    3,
    3,
    PORTS,
    3,
    ROUTE_SLOTS,
    3,
    3,
    3,
>;

enum NativeOperation {
    Keyboard {
        empty: ValueRef,
        pending: Option<RequestId>,
        next: u32,
        emitted: bool,
    },
    Keymap {
        pending: Option<RequestId>,
        next: u32,
    },
    Presentation {
        pending: Option<RequestId>,
        next: u32,
    },
}

impl NativeOperation {
    fn fail(detail: u16) -> OperationAction {
        OperationAction::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    fn keyboard_request(
        empty: ValueRef,
        next: &mut u32,
        pending: &mut Option<RequestId>,
    ) -> OperationAction {
        if *next >= TEXT_LAB_MAXIMUM_VALUES as u32 {
            return OperationAction::Complete;
        }
        let request = RequestId(*next);
        *next += 1;
        *pending = Some(request);
        OperationAction::RequestHostOperation {
            request,
            operation: HostOperationId(0),
            input: BoundedValueRef::new(empty, 0).expect("keyboard request input is empty"),
        }
    }
}

impl Operation for NativeOperation {
    fn start(&mut self) -> OperationAction {
        match self {
            Self::Keyboard {
                empty,
                pending,
                next,
                ..
            } => Self::keyboard_request(*empty, next, pending),
            Self::Keymap { .. } | Self::Presentation { .. } => OperationAction::Await,
        }
    }

    fn resume(&mut self, input: OperationInput) -> OperationAction {
        match (self, input) {
            (
                Self::Keyboard {
                    pending, emitted, ..
                },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return Self::fail(1);
                };
                *pending = None;
                *emitted = true;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (
                Self::Keymap { pending, next },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none() && *next < TEXT_LAB_MAXIMUM_VALUES as u32 => {
                let request = RequestId(*next);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(
                        value,
                        conduit_core::KEY_EVENT_ENCODED_LEN as u32,
                    ) {
                        Ok(value) => value,
                        Err(_) => return Self::fail(2),
                    },
                }
            }
            (
                Self::Keymap { pending, next },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.failure.is_none() =>
            {
                let Some(output) = outcome.output else {
                    return Self::fail(3);
                };
                *pending = None;
                *next += 1;
                OperationAction::Emit {
                    port: PortId(0),
                    value: output.value,
                }
            }
            (Self::Keymap { pending, .. }, OperationInput::Closed { port: PortId(0) })
                if pending.is_none() =>
            {
                OperationAction::Complete
            }
            (
                Self::Presentation { pending, next },
                OperationInput::Value {
                    port: PortId(0),
                    value,
                },
            ) if pending.is_none() && *next < TEXT_LAB_MAXIMUM_VALUES as u32 => {
                let request = RequestId(*next);
                *pending = Some(request);
                OperationAction::RequestHostOperation {
                    request,
                    operation: HostOperationId(0),
                    input: match BoundedValueRef::new(value, MAX_TEXT_BYTES) {
                        Ok(value) => value,
                        Err(_) => return Self::fail(4),
                    },
                }
            }
            (
                Self::Presentation { pending, next },
                OperationInput::HostOperationCompleted { request, outcome },
            ) if *pending == Some(request)
                && outcome.disposition == HostOperationDisposition::Completed
                && outcome.output.is_none()
                && outcome.failure.is_none() =>
            {
                *pending = None;
                *next += 1;
                OperationAction::Await
            }
            (Self::Presentation { pending, .. }, OperationInput::Closed { port: PortId(0) })
                if pending.is_none() =>
            {
                OperationAction::Complete
            }
            _ => Self::fail(5),
        }
    }

    fn advance(&mut self) -> OperationAction {
        match self {
            Self::Keyboard {
                empty,
                pending,
                next,
                emitted,
            } if *emitted => {
                *emitted = false;
                Self::keyboard_request(*empty, next, pending)
            }
            _ => OperationAction::Await,
        }
    }
}

pub struct NativeTextLabFragment {
    scheduler: NativeTextLabScheduler,
    lowered: LoweredPlanFragment,
    kinds: [String; 3],
    keymap: ConduitIntlKeymap,
    keyboard_index: usize,
    presented: String,
}

pub struct NativeTextOffer {
    pub sequence: u64,
    pub bytes: Vec<u8>,
}

mod lifecycle;

impl NativeTextLabFragment {
    pub fn prepare(base_instance: &str) -> Result<Self, String> {
        let exact = exact_text_lab_split_plan(base_instance)?;
        let fragment = exact
            .plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id.as_str() == TEXT_LAB_NATIVE_HOST)
            .ok_or_else(|| "split Text Lab native fragment is missing".to_string())?;
        let lowered = lower_plan_fragment(fragment).map_err(|error| format!("{error:?}"))?;
        if lowered.nodes.len() != 3
            || lowered.cords.len() != 3
            || lowered.remote_endpoints.len() != 2
            || lowered.host_operations.len() != 3
        {
            return Err("split Text Lab native fragment has the wrong exact shape".into());
        }
        let mut routes = FixedRoutes::<ROUTE_SLOTS, 3>::new(PORTS as u16);
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
        let mut bindings = FixedHostOperationBindings::<3>::new(1);
        for operation in &lowered.host_operations {
            bindings
                .install(operation.node, operation.binding)
                .map_err(|error| format!("{error:?}"))?;
        }
        bindings.seal().map_err(|error| format!("{error:?}"))?;
        let mut values = HostedValueStore::new(6, MAX_TEXT_BYTES, MAX_TEXT_BYTES * 6)
            .map_err(|error| format!("{error:?}"))?;
        let empty = values.store(&[]).map_err(|error| format!("{error:?}"))?;
        let mut drivers = Vec::with_capacity(3);
        let mut kinds = Vec::with_capacity(3);
        for placement in &fragment.placements {
            let kind = placement.kind_id.as_str();
            let operation = match kind {
                KEYBOARD_KIND => NativeOperation::Keyboard {
                    empty,
                    pending: None,
                    next: 0,
                    emitted: false,
                },
                KEYMAP_KIND => NativeOperation::Keymap {
                    pending: None,
                    next: 0,
                },
                TEXT_PRESENTATION_KIND => NativeOperation::Presentation {
                    pending: None,
                    next: 0,
                },
                _ => return Err(format!("unsupported native Text Lab Kind {kind}")),
            };
            kinds.push(kind.to_string());
            drivers.push(OperationDriver::new(operation).map_err(|error| format!("{error:?}"))?);
        }
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "split Text Lab native Sign budget overflow".to_string())?;
        let scheduler = NativeTextLabScheduler::new_with_host_operations(
            lowered
                .node_specs
                .clone()
                .try_into()
                .map_err(|_| "native node width")?,
            lowered
                .cords
                .iter()
                .map(|cord| cord.spec)
                .collect::<Vec<_>>()
                .try_into()
                .map_err(|_| "native Cord width")?,
            routes,
            bindings,
            drivers.try_into().map_err(|_| "native driver width")?,
            values,
            HostedSignLog::new_with_remote_storage(
                SIGN_ITEMS,
                sign_bytes,
                SIGN_ITEMS,
                conduit_kernel::remote_sign_storage_bytes(SIGN_ITEMS)
                    .ok_or("native remote Sign byte overflow")?,
            )
            .map_err(|error| format!("{error:?}"))?,
        )
        .map_err(|error| format!("{error:?}"))?;
        Ok(Self {
            scheduler,
            lowered,
            kinds: kinds.try_into().map_err(|_| "native Kind width")?,
            keymap: ConduitIntlKeymap::new(),
            keyboard_index: 0,
            presented: String::with_capacity(TEXT_LAB_MAXIMUM_VALUES * MAX_TEXT_BYTES as usize),
        })
    }

    fn endpoint(&self, direction: RemoteCordDirection) -> (RemoteEndpointId, CordId) {
        let endpoint = self
            .lowered
            .remote_endpoints
            .iter()
            .find(|endpoint| endpoint.direction == direction)
            .expect("exact split Text Lab direction was checked");
        (endpoint.endpoint, endpoint.cord)
    }

    fn complete_host_request(&mut self) -> Result<bool, String> {
        let Some(request) = self.scheduler.next_host_request() else {
            return Ok(false);
        };
        let kind = self.kinds[usize::from(request.node.0)].as_str();
        let output = match kind {
            KEYBOARD_KIND => {
                let usage = [0x0b, 0x08, 0x0f, 0x0f, 0x12]
                    .get(self.keyboard_index)
                    .copied()
                    .ok_or_else(|| "scripted keyboard exceeded five admitted events".to_string())?;
                self.keyboard_index += 1;
                let event = KeyEvent::new(usage, KeyTransition::Pressed, KeyModifiers::NONE)
                    .map_err(|error| format!("{error:?}"))?;
                let value = self
                    .scheduler
                    .store_host_value(&event.encode())
                    .map_err(|error| format!("{error:?}"))?;
                Some(
                    BoundedValueRef::new(value, conduit_core::KEY_EVENT_ENCODED_LEN as u32)
                        .map_err(|error| format!("{error:?}"))?,
                )
            }
            KEYMAP_KIND => {
                let bytes = self
                    .scheduler
                    .host_value(request.input.value)
                    .map_err(|error| format!("{error:?}"))?;
                let event = KeyEvent::decode(bytes).map_err(|error| format!("{error:?}"))?;
                let KeymapDisposition::Text(text) = self.keymap.apply(event) else {
                    return Err("Text Lab keymap did not produce text".into());
                };
                let value = self
                    .scheduler
                    .store_host_value(text.as_bytes())
                    .map_err(|error| format!("{error:?}"))?;
                Some(
                    BoundedValueRef::new(value, conduit_core::CHORD_ENCODED_LEN as u32)
                        .map_err(|error| format!("{error:?}"))?,
                )
            }
            TEXT_PRESENTATION_KIND => {
                let bytes = self
                    .scheduler
                    .host_value(request.input.value)
                    .map_err(|error| format!("{error:?}"))?;
                self.presented
                    .push_str(core::str::from_utf8(bytes).map_err(|error| error.to_string())?);
                None
            }
            _ => return Err("unsupported native Text Lab host request".into()),
        };
        self.scheduler
            .complete_host_operation(
                request.node,
                request.request,
                HostOperationOutcome {
                    disposition: HostOperationDisposition::Completed,
                    output,
                    failure: None,
                },
            )
            .map_err(|error| format!("{error:?}"))?;
        Ok(true)
    }

    pub fn next_text_offer(&mut self) -> Result<NativeTextOffer, String> {
        loop {
            if self.complete_host_request()? {
                continue;
            }
            let (endpoint, cord) = self.endpoint(RemoteCordDirection::Egress);
            if let Some(offer) = self
                .scheduler
                .remote_egress_offer(endpoint, cord)
                .map_err(|error| format!("{error:?}"))?
            {
                let bytes = self
                    .scheduler
                    .host_value(offer.value)
                    .map_err(|error| format!("{error:?}"))?
                    .to_vec();
                return Ok(NativeTextOffer {
                    sequence: offer.sequence,
                    bytes,
                });
            }
            match self
                .scheduler
                .step()
                .map_err(|error| format!("{error:?}"))?
            {
                SchedulerStatus::Progress { .. } => {}
                SchedulerStatus::Idle => {
                    return Err("native Text Lab became idle before output".into())
                }
                SchedulerStatus::Complete => {
                    return Err("native Text Lab completed before output".into())
                }
                SchedulerStatus::Cancelled => return Err("native Text Lab cancelled".into()),
            }
        }
    }

    pub fn accept_text(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Egress);
        self.scheduler
            .remote_egress_accept(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn deliver_text(&mut self, sequence: u64) -> Result<(), String> {
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Egress);
        self.scheduler
            .remote_egress_delivered(endpoint, cord, sequence)
            .map_err(|error| format!("{error:?}"))
    }

    pub fn admit_returned(&mut self, sequence: u64, bytes: &[u8]) -> Result<(), String> {
        let (endpoint, cord) = self.endpoint(RemoteCordDirection::Ingress);
        self.scheduler
            .admit_remote_input(endpoint, cord, sequence, bytes)
            .map(|_| ())
            .map_err(|error| format!("{error:?}"))
    }

    pub fn presented(&self) -> &str {
        &self.presented
    }
}
