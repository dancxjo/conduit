//! Native production-kernel half of the exact split Text Lab Plan.

use conduit_human::{ConduitIntlKeymap, KeyEvent, KeyModifiers, KeyTransition, KeymapDisposition};
use conduit_kernel::scheduler::{
    FixedScheduler, SchedulerStatus, StepInputBytes, StepIo, StepOperation, StepOutcome,
};
use conduit_kernel::{
    BoundedValueRef, CordId, Failure, FailureCode, FixedHostCallBindings, FixedRoutes,
    HostCallDisposition, HostCallId, HostCallOutcome, HostedSignLog, HostedValueStore, PortId,
    RemoteEndpointId, RequestId, ValueRef, ValueStorage,
};
use conduit_plan_lowering::lowering::{
    lower_plan_fragment, LoweredPlanFragment, RemoteCordDirection,
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE,
};
use conduit_semantic_catalog::{
    exact_text_lab_split_plan, KEYBOARD_KIND, KEYMAP_KIND, TEXT_LAB_MAXIMUM_VALUES,
    TEXT_LAB_NATIVE_HOST, TEXT_PRESENTATION_KIND,
};
use conduit_text::MAX_TEXT_BYTES;

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const ROUTE_SLOTS: usize = 3 * PORTS;
const SIGN_ITEMS: u16 = 192;

type NativeTextLabScheduler = FixedScheduler<
    NativeBack,
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

enum NativeBack {
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

impl NativeBack {
    fn fail(detail: u16) -> StepOutcome {
        StepOutcome::Fail(Failure {
            code: FailureCode::InvalidLifecycle,
            detail,
        })
    }

    fn keyboard_request(
        empty: ValueRef,
        next: &mut u32,
        pending: &mut Option<RequestId>,
        io: &mut StepIo<PORTS>,
    ) -> StepOutcome {
        if *next >= TEXT_LAB_MAXIMUM_VALUES as u32 {
            return StepOutcome::Complete;
        }
        let request = RequestId(*next);
        let input = BoundedValueRef::new(empty, 0).expect("keyboard request input is empty");
        if io.request_host_call(request, HostCallId(0), input).is_err() {
            return Self::fail(5);
        }
        *next += 1;
        *pending = Some(request);
        StepOutcome::Progress
    }
}

impl StepOperation<PORTS> for NativeBack {
    fn step(
        &mut self,
        io: &mut StepIo<PORTS>,
        _input_bytes: &StepInputBytes<'_, PORTS>,
    ) -> StepOutcome {
        match self {
            Self::Keyboard {
                empty,
                pending,
                next,
                emitted,
            } => {
                if let Some(expected) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    let Some(output) = outcome.output else {
                        return Self::fail(1);
                    };
                    if request != expected
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                        || !io.output_ready(PortId(0))
                        || io.consume_host_completion().is_err()
                        || io.send(PortId(0), output.value).is_err()
                    {
                        return Self::fail(5);
                    }
                    *pending = None;
                    *emitted = true;
                    return StepOutcome::Progress;
                }
                if *emitted {
                    *emitted = false;
                }
                Self::keyboard_request(*empty, next, pending, io)
            }
            Self::Keymap { pending, next } => {
                if let Some(expected) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    let Some(output) = outcome.output else {
                        return Self::fail(3);
                    };
                    if request != expected
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.failure.is_some()
                        || !io.output_ready(PortId(0))
                        || io.consume_host_completion().is_err()
                        || io.send(PortId(0), output.value).is_err()
                    {
                        return Self::fail(5);
                    }
                    *pending = None;
                    *next += 1;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    if *next >= TEXT_LAB_MAXIMUM_VALUES as u32 {
                        return Self::fail(5);
                    }
                    let Ok(input) =
                        BoundedValueRef::new(value, conduit_human::KEY_EVENT_ENCODED_LEN as u32)
                    else {
                        return Self::fail(2);
                    };
                    let request = RequestId(*next);
                    if io.consume(PortId(0)).is_err()
                        || io.request_host_call(request, HostCallId(0), input).is_err()
                    {
                        return Self::fail(5);
                    }
                    *pending = Some(request);
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    if io.consume_closed(PortId(0)).is_err() {
                        return Self::fail(5);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
            Self::Presentation { pending, next } => {
                if let Some(expected) = *pending {
                    let Some((request, outcome)) = io.host_completion() else {
                        return StepOutcome::Await;
                    };
                    if request != expected
                        || outcome.disposition != HostCallDisposition::Completed
                        || outcome.output.is_some()
                        || outcome.failure.is_some()
                        || io.consume_host_completion().is_err()
                    {
                        return Self::fail(5);
                    }
                    *pending = None;
                    *next += 1;
                    return StepOutcome::Progress;
                }
                if let Some(value) = io.input(PortId(0)) {
                    if *next >= TEXT_LAB_MAXIMUM_VALUES as u32 {
                        return Self::fail(5);
                    }
                    let Ok(input) = BoundedValueRef::new(value, MAX_TEXT_BYTES) else {
                        return Self::fail(4);
                    };
                    let request = RequestId(*next);
                    if io.consume(PortId(0)).is_err()
                        || io.request_host_call(request, HostCallId(0), input).is_err()
                    {
                        return Self::fail(5);
                    }
                    *pending = Some(request);
                    return StepOutcome::Progress;
                }
                if io.input_closed(PortId(0)) {
                    if io.consume_closed(PortId(0)).is_err() {
                        return Self::fail(5);
                    }
                    return StepOutcome::Complete;
                }
                StepOutcome::Await
            }
        }
    }

    fn cancel(&mut self) {
        match self {
            Self::Keyboard { pending, .. }
            | Self::Keymap { pending, .. }
            | Self::Presentation { pending, .. } => *pending = None,
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
        let exact = exact_text_lab_split_plan(
            base_instance,
            &conduit_browser_runtime::presentation_nucleus::browser_text_upper_offer(),
        )?;
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
            || lowered.host_calls.len() != 3
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
        let mut bindings = FixedHostCallBindings::<3>::new(1);
        for operation in &lowered.host_calls {
            bindings
                .install(operation.node, operation.binding)
                .map_err(|error| format!("{error:?}"))?;
        }
        bindings.seal().map_err(|error| format!("{error:?}"))?;
        let mut values = HostedValueStore::new(6, MAX_TEXT_BYTES, MAX_TEXT_BYTES * 6)
            .map_err(|error| format!("{error:?}"))?;
        let empty = values.store(&[]).map_err(|error| format!("{error:?}"))?;
        let mut backs = Vec::with_capacity(3);
        let mut kinds = Vec::with_capacity(3);
        for placement in &fragment.placements {
            let kind = placement.kind_id.as_str();
            let back = match kind {
                KEYBOARD_KIND => NativeBack::Keyboard {
                    empty,
                    pending: None,
                    next: 0,
                    emitted: false,
                },
                KEYMAP_KIND => NativeBack::Keymap {
                    pending: None,
                    next: 0,
                },
                TEXT_PRESENTATION_KIND => NativeBack::Presentation {
                    pending: None,
                    next: 0,
                },
                _ => return Err(format!("unsupported native Text Lab Kind {kind}")),
            };
            kinds.push(kind.to_string());
            backs.push(back);
        }
        let sign_bytes = u32::from(SIGN_ITEMS)
            .checked_mul(core::mem::size_of::<conduit_kernel::KernelEvent>() as u32)
            .ok_or_else(|| "split Text Lab native Sign budget overflow".to_string())?;
        let scheduler = NativeTextLabScheduler::new_with_host_calls(
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
            backs.try_into().map_err(|_| "native Back width")?,
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
                    BoundedValueRef::new(value, conduit_human::KEY_EVENT_ENCODED_LEN as u32)
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
                    BoundedValueRef::new(value, conduit_human::CHORD_ENCODED_LEN as u32)
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
            .complete_host_call(
                request.node,
                request.request,
                HostCallOutcome {
                    disposition: HostCallDisposition::Completed,
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
                SchedulerStatus::Drained => {
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
