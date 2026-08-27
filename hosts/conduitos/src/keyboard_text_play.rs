//! Fixed-storage production-kernel execution for the ordinary keyboard text Form.

use alloc::boxed::Box;
use conduit_core::{ConduitIntlKeymap, KeyEvent, KeymapDisposition, PlanFragment};
use conduit_kernel::{
    BoundedValueRef, Failure, FailureCode, FixedHostOperationBindings, FixedRoutes, FixedSignLog,
    FixedValueStore, HostOperationDisposition, HostOperationOutcome, KernelEvent, NodeId, SignSink,
    ValueRef, ValueStorage,
    scheduler::{
        FixedScheduler, HostOperationRequest, OperationDriver, SchedulerError, SchedulerStatus,
    },
};
use conduit_plan_lowering::lowering::{FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment};

use crate::{
    keyboard_text_operations::{
        KeyboardOperation, PlannedOperation, PresentationOperation, StreamTransformOperation,
    },
    keyboard_text_plan::PreparedKeyboardTextPlay,
    ordinary_plan::PreparationError,
};

pub const MAXIMUM_INPUT_EVENTS: usize = 48;
pub const MAXIMUM_PRESENTATIONS: usize = 16;
const MAX_NODES: usize = 4;
const MAX_CORDS: usize = 3;
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const QUEUE_SLOTS: usize = 3;
const ROUTE_SLOTS: usize = MAX_NODES * PORTS;
const ROUTE_TARGETS: usize = 3;
const HOST_BINDING_SLOTS: usize = MAX_NODES * MAX_NODES;
const PENDING_REQUESTS: usize = 4;
const VALUE_SLOTS: usize = 64;
const MAX_VALUE_BYTES: usize = conduit_text::MAX_TEXT_BYTES as usize;
const VALUE_BYTE_CAPACITY: usize = 128;
const SIGN_CAPACITY: usize = 768;

type Driver = OperationDriver<PlannedOperation, PORTS>;
type Scheduler = FixedScheduler<
    Driver,
    FixedValueStore<VALUE_SLOTS, MAX_VALUE_BYTES>,
    FixedSignLog<SIGN_CAPACITY>,
    MAX_NODES,
    MAX_CORDS,
    PORTS,
    QUEUE_SLOTS,
    ROUTE_SLOTS,
    ROUTE_TARGETS,
    HOST_BINDING_SLOTS,
    PENDING_REQUESTS,
>;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct PresentationFragment {
    bytes: [u8; 4],
    len: u8,
}

impl PresentationFragment {
    pub fn as_bytes(&self) -> &[u8] {
        &self.bytes[..usize::from(self.len)]
    }

    fn from_bytes(bytes: &[u8]) -> Result<Self, PreparationError> {
        if bytes.len() > 4 || core::str::from_utf8(bytes).is_err() {
            return Err(PreparationError::KernelRejected);
        }
        let mut value = [0; 4];
        value[..bytes.len()].copy_from_slice(bytes);
        Ok(Self {
            bytes: value,
            len: bytes.len() as u8,
        })
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeyboardTextPlayReport {
    pub presentations: [Option<PresentationFragment>; MAXIMUM_PRESENTATIONS],
    pub presentation_count: u8,
    pub decisions: u32,
    pub signs: u16,
    pub completed: bool,
}

pub struct KeyboardTextKernel {
    scheduler: Scheduler,
    keyboard_node: NodeId,
    keymap_node: NodeId,
    upper_node: NodeId,
    presentation_node: NodeId,
    keymap: ConduitIntlKeymap,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum KeyboardTextRequestKind {
    Keyboard,
    Keymap,
    Upper,
    Presentation,
}

impl KeyboardTextKernel {
    pub fn prepare(
        prepared: &PreparedKeyboardTextPlay,
        event_count: usize,
    ) -> Result<Self, PreparationError> {
        if event_count == 0 || event_count > MAXIMUM_INPUT_EVENTS {
            return Err(PreparationError::PlanRejected);
        }
        let fragment = prepared
            .plan
            .fragments
            .first()
            .ok_or(PreparationError::PlanRejected)?;
        let lowered = conduit_plan_lowering::lowering::lower_plan_fragment(fragment)
            .map_err(|_| PreparationError::LoweringRejected)?;
        validate_shape(fragment, &lowered)?;
        let keyboard_node = node_for(fragment, conduit_std_catalog::KEYBOARD_KIND)?;
        let keymap_node = node_for(fragment, conduit_std_catalog::KEYMAP_KIND)?;
        let upper_node = node_for(fragment, conduit_text::TEXT_UPPER_KIND)?;
        let presentation_node = node_for(fragment, conduit_std_catalog::TEXT_PRESENTATION_KIND)?;
        let mut values =
            FixedValueStore::<VALUE_SLOTS, MAX_VALUE_BYTES>::new(VALUE_BYTE_CAPACITY as u32)
                .map_err(|_| PreparationError::KernelRejected)?;
        let empty = values
            .store(&[])
            .map_err(|_| PreparationError::KernelRejected)?;
        for _ in 1..event_count {
            values
                .retain(empty)
                .map_err(|_| PreparationError::KernelRejected)?;
        }
        let nodes = lowered
            .node_specs
            .as_slice()
            .try_into()
            .map_err(|_| PreparationError::KernelRejected)?;
        let cords = [
            lowered.cords[0].spec,
            lowered.cords[1].spec,
            lowered.cords[2].spec,
        ];
        let mut routes = FixedRoutes::<ROUTE_SLOTS, ROUTE_TARGETS>::new(PORTS as u16);
        for route in &lowered.routes {
            routes
                .install(
                    route.source_node,
                    route.source_port,
                    route.range,
                    &route.targets,
                )
                .map_err(|_| PreparationError::KernelRejected)?;
        }
        routes
            .seal()
            .map_err(|_| PreparationError::KernelRejected)?;
        let mut bindings = FixedHostOperationBindings::<HOST_BINDING_SLOTS>::new(MAX_NODES as u16);
        for operation in &lowered.host_operations {
            bindings
                .install(operation.node, operation.binding)
                .map_err(|_| PreparationError::KernelRejected)?;
        }
        bindings
            .seal()
            .map_err(|_| PreparationError::KernelRejected)?;
        let mut drivers = [None, None, None, None];
        drivers[usize::from(keyboard_node.0)] = Some(
            OperationDriver::new(PlannedOperation::Keyboard(KeyboardOperation {
                empty,
                pending: None,
                next: 0,
                maximum: event_count as u32,
            }))
            .map_err(|_| PreparationError::KernelRejected)?,
        );
        drivers[usize::from(keymap_node.0)] = Some(
            OperationDriver::new(PlannedOperation::Keymap(StreamTransformOperation::new(
                true,
            )))
            .map_err(|_| PreparationError::KernelRejected)?,
        );
        drivers[usize::from(upper_node.0)] = Some(
            OperationDriver::new(PlannedOperation::Upper(StreamTransformOperation::new(
                false,
            )))
            .map_err(|_| PreparationError::KernelRejected)?,
        );
        drivers[usize::from(presentation_node.0)] = Some(
            OperationDriver::new(PlannedOperation::Presentation(PresentationOperation {
                pending: None,
                next: 0,
            }))
            .map_err(|_| PreparationError::KernelRejected)?,
        );
        let [Some(first), Some(second), Some(third), Some(fourth)] = drivers else {
            return Err(PreparationError::KernelRejected);
        };
        let minimum_sign_bytes = (SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32;
        let signs = FixedSignLog::<SIGN_CAPACITY>::new(lowered.sign_bytes.max(minimum_sign_bytes))
            .map_err(|_| PreparationError::KernelRejected)?;
        let scheduler = FixedScheduler::new_with_host_operations(
            nodes,
            cords,
            routes,
            bindings,
            [first, second, third, fourth],
            values,
            signs,
        )
        .map_err(|_| PreparationError::KernelRejected)?;
        Ok(Self {
            scheduler,
            keyboard_node,
            keymap_node,
            upper_node,
            presentation_node,
            keymap: ConduitIntlKeymap::new(),
        })
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.scheduler.step()
    }

    pub fn next_host_request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }

    pub fn request_kind(
        &self,
        request: HostOperationRequest,
    ) -> Result<KeyboardTextRequestKind, SchedulerError> {
        if request.node == self.keyboard_node {
            Ok(KeyboardTextRequestKind::Keyboard)
        } else if request.node == self.keymap_node {
            Ok(KeyboardTextRequestKind::Keymap)
        } else if request.node == self.upper_node {
            Ok(KeyboardTextRequestKind::Upper)
        } else if request.node == self.presentation_node {
            Ok(KeyboardTextRequestKind::Presentation)
        } else {
            Err(SchedulerError::InvalidHostOperationAccess)
        }
    }

    pub fn host_value(&self, value: ValueRef) -> Result<&[u8], SchedulerError> {
        self.scheduler.host_value(value)
    }

    pub fn complete_keyboard(
        &mut self,
        request: HostOperationRequest,
        event: KeyEvent,
    ) -> Result<(), SchedulerError> {
        self.complete_with_output(request, self.keyboard_node, &event.encode())
    }

    pub fn fail_keyboard_device_removed(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<(), SchedulerError> {
        if request.node != self.keyboard_node {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        self.complete_failed(request, FailureCode::HostOperationFailed, 81)
    }

    pub fn complete_keymap(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
        if request.node != self.keymap_node {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        let input = self.scheduler.host_value(request.input.value)?;
        let event =
            KeyEvent::decode(input).map_err(|_| SchedulerError::InvalidHostOperationAccess)?;
        let output = match self.keymap.apply(event) {
            KeymapDisposition::Text(fragment) => Some(fragment),
            KeymapDisposition::NoText | KeymapDisposition::Cancelled => None,
            KeymapDisposition::Refused(_) => {
                return self.complete_failed(request, FailureCode::InvalidInput, 72);
            }
        };
        match output {
            Some(fragment) => {
                self.complete_with_output(request, self.keymap_node, fragment.as_bytes())
            }
            None => self.complete_without_output(request, self.keymap_node),
        }
    }

    pub fn complete_upper(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
        if request.node != self.upper_node {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        let input = self.scheduler.host_value(request.input.value)?;
        let output = crate::text_upper::uppercase(input)
            .map_err(|_| SchedulerError::InvalidHostOperationAccess)?;
        self.complete_with_output(request, self.upper_node, output.as_bytes())
    }

    pub fn complete_presentation(
        &mut self,
        request: HostOperationRequest,
    ) -> Result<PresentationFragment, SchedulerError> {
        if request.node != self.presentation_node {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        let value =
            PresentationFragment::from_bytes(self.scheduler.host_value(request.input.value)?)
                .map_err(|_| SchedulerError::InvalidHostOperationAccess)?;
        self.complete_without_output(request, self.presentation_node)?;
        Ok(value)
    }

    pub fn cancel(&mut self) -> Result<(), SchedulerError> {
        self.keymap.reset();
        self.scheduler.cancel()
    }

    fn complete_with_output(
        &mut self,
        request: HostOperationRequest,
        node: NodeId,
        output: &[u8],
    ) -> Result<(), SchedulerError> {
        if request.node != node {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        let value = self.scheduler.store_host_value(output)?;
        let output = BoundedValueRef::new(value, output.len() as u32)
            .map_err(|_| SchedulerError::InvalidHostOperationAccess)?;
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: Some(output),
                failure: None,
            },
        )
    }

    fn complete_without_output(
        &mut self,
        request: HostOperationRequest,
        node: NodeId,
    ) -> Result<(), SchedulerError> {
        if request.node != node {
            return Err(SchedulerError::InvalidHostOperationAccess);
        }
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Completed,
                output: None,
                failure: None,
            },
        )
    }

    fn complete_failed(
        &mut self,
        request: HostOperationRequest,
        code: FailureCode,
        detail: u16,
    ) -> Result<(), SchedulerError> {
        self.scheduler.complete_host_operation(
            request.node,
            request.request,
            HostOperationOutcome {
                disposition: HostOperationDisposition::Failed,
                output: None,
                failure: Some(Failure { code, detail }),
            },
        )
    }
}

pub fn run(
    prepared: &PreparedKeyboardTextPlay,
    events: &[KeyEvent],
) -> Result<KeyboardTextPlayReport, PreparationError> {
    run_with_presentation(prepared, events, |_| {})
}

pub fn run_with_presentation(
    prepared: &PreparedKeyboardTextPlay,
    events: &[KeyEvent],
    mut present: impl FnMut(PresentationFragment),
) -> Result<KeyboardTextPlayReport, PreparationError> {
    // ConduitOS admits this fixed-size kernel from its finite boot arena before
    // the Play starts; scheduler progress itself performs no allocation.
    let mut kernel = Box::new(KeyboardTextKernel::prepare(prepared, events.len())?);
    let mut event_index = 0usize;
    let mut presentations = [None; MAXIMUM_PRESENTATIONS];
    let mut presentation_count = 0usize;
    for _ in 0..2_048 {
        while let Some(request) = kernel.next_host_request() {
            if request.node == kernel.keyboard_node {
                let event = events
                    .get(event_index)
                    .copied()
                    .ok_or(PreparationError::KernelRejected)?;
                event_index += 1;
                kernel
                    .complete_keyboard(request, event)
                    .map_err(|_| PreparationError::KernelRejected)?;
            } else if request.node == kernel.keymap_node {
                kernel
                    .complete_keymap(request)
                    .map_err(|_| PreparationError::KernelRejected)?;
            } else if request.node == kernel.upper_node {
                kernel
                    .complete_upper(request)
                    .map_err(|_| PreparationError::KernelRejected)?;
            } else if request.node == kernel.presentation_node {
                let slot = presentations
                    .get_mut(presentation_count)
                    .ok_or(PreparationError::KernelRejected)?;
                let fragment = kernel
                    .complete_presentation(request)
                    .map_err(|_| PreparationError::KernelRejected)?;
                present(fragment);
                *slot = Some(fragment);
                presentation_count += 1;
            } else {
                return Err(PreparationError::KernelRejected);
            }
        }
        match kernel
            .step()
            .map_err(|_| PreparationError::KernelRejected)?
        {
            SchedulerStatus::Progress { .. } => {}
            SchedulerStatus::Idle => {
                if kernel.scheduler.pending_host_operation_count() == 0 {
                    return Err(PreparationError::KernelRejected);
                }
            }
            SchedulerStatus::Complete => {
                if event_index != events.len() {
                    return Err(PreparationError::KernelRejected);
                }
                return Ok(KeyboardTextPlayReport {
                    presentations,
                    presentation_count: presentation_count as u8,
                    decisions: kernel.scheduler.decisions(),
                    signs: kernel.scheduler.signs().len(),
                    completed: true,
                });
            }
            SchedulerStatus::Cancelled => return Err(PreparationError::KernelRejected),
        }
    }
    Err(PreparationError::KernelRejected)
}

fn validate_shape(
    fragment: &PlanFragment,
    lowered: &LoweredPlanFragment,
) -> Result<(), PreparationError> {
    if fragment.placements.len() != MAX_NODES
        || fragment.connections.len() != MAX_CORDS
        || lowered.nodes.len() != MAX_NODES
        || lowered.cords.len() != MAX_CORDS
        || lowered.routes.len() != MAX_CORDS
        || lowered.host_operations.len() != MAX_NODES
        || !lowered.remote_endpoints.is_empty()
    {
        return Err(PreparationError::LoweringRejected);
    }
    Ok(())
}

fn node_for(fragment: &PlanFragment, kind: &str) -> Result<NodeId, PreparationError> {
    fragment
        .placements
        .iter()
        .position(|placement| placement.kind_id.as_str() == kind)
        .and_then(|index| u16::try_from(index).ok())
        .map(NodeId)
        .ok_or(PreparationError::PlanRejected)
}
