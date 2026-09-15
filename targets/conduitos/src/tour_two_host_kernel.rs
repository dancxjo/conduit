//! Two fixed fragment kernels joined only through their admitted remote Cord.

use conduit_core::{ConfigurationValue, PlanFragment};
use conduit_kernel::{
    FixedHostOperationBindings, FixedRoutes, FixedSignLog, FixedValueStore,
    HostOperationDisposition, HostOperationOutcome, KernelEvent, RemoteEndpointId, SignSink,
    ValueStorage,
    scheduler::{
        FixedScheduler, HostOperationRequest, RemoteIngressOutcome, SchedulerError, SchedulerStatus,
    },
};
use conduit_plan_lowering::lowering::{
    FIXED_KERNEL_STORAGE_PORTS_PER_NODE, LoweredPlanFragment, RemoteCordDirection,
};

use crate::text_kernel_operations::{LiteralOperation, LiteralState, PresentationOperation};

const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
const SIGN_CAPACITY: usize = 32;
const REMOTE_SIGN_CAPACITY: u16 = 16;
const VALUE_BYTES: usize = conduit_text::MAX_TEXT_BYTES as usize;

type SourceScheduler = FixedScheduler<
    conduit_kernel::scheduler::OperationDriver<LiteralOperation, PORTS>,
    FixedValueStore<1, VALUE_BYTES>,
    FixedSignLog<SIGN_CAPACITY>,
    1,
    1,
    PORTS,
    1,
    1,
    1,
>;
type SinkScheduler = FixedScheduler<
    conduit_kernel::scheduler::OperationDriver<PresentationOperation, PORTS>,
    FixedValueStore<1, VALUE_BYTES>,
    FixedSignLog<SIGN_CAPACITY>,
    1,
    1,
    PORTS,
    1,
    1,
    1,
    1,
    1,
>;

pub struct SourceKernel {
    scheduler: SourceScheduler,
    endpoint: RemoteEndpointId,
    cord: conduit_kernel::CordId,
}

pub struct SinkKernel {
    scheduler: SinkScheduler,
    endpoint: RemoteEndpointId,
    cord: conduit_kernel::CordId,
}

impl SourceKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        expected: &str,
    ) -> Result<Self, SchedulerError> {
        let endpoint = exact_endpoint(lowered, RemoteCordDirection::Egress)?;
        if fragment.placements.len() != 1
            || fragment.placements[0].kind_id.as_str() != conduit_text::TEXT_LITERAL_KIND
            || lowered.node_specs.len() != 1
            || lowered.cords.len() != 1
            || lowered.routes.len() != 1
            || !lowered.host_operations.is_empty()
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let configured = fragment.placements[0]
            .configuration
            .iter()
            .find_map(|entry| match (&*entry.key, &entry.value) {
                ("value", ConfigurationValue::Text(value)) => Some(value.as_str()),
                _ => None,
            })
            .filter(|value| *value == expected)
            .ok_or(SchedulerError::InvalidPlan)?;
        let mut values = FixedValueStore::<1, VALUE_BYTES>::new(VALUE_BYTES as u32)?;
        let text = values.store(configured.as_bytes())?;
        let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
        let route = &lowered.routes[0];
        routes.install(
            route.source_node,
            route.source_port,
            route.range,
            &route.targets,
        )?;
        routes.seal()?;
        let signs = signs(lowered.sign_bytes)?;
        let scheduler = FixedScheduler::new(
            [lowered.node_specs[0]],
            [lowered.cords[0].spec],
            routes,
            [conduit_kernel::scheduler::OperationDriver::new(
                LiteralOperation {
                    text,
                    state: LiteralState::Emitting,
                },
            )?],
            values,
            signs,
        )?;
        Ok(Self {
            scheduler,
            endpoint: endpoint.endpoint,
            cord: endpoint.cord,
        })
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.scheduler.step()
    }

    pub fn offer(&mut self) -> Result<Option<(u64, &[u8])>, SchedulerError> {
        let Some(offer) = self
            .scheduler
            .remote_egress_offer(self.endpoint, self.cord)?
        else {
            return Ok(None);
        };
        Ok(Some((
            offer.sequence,
            self.scheduler.host_value(offer.value)?,
        )))
    }

    pub fn accept(&mut self, sequence: u64) -> Result<(), SchedulerError> {
        self.scheduler
            .remote_egress_accept(self.endpoint, self.cord, sequence)
    }

    pub fn delivered(&mut self, sequence: u64) -> Result<(), SchedulerError> {
        self.scheduler
            .remote_egress_delivered(self.endpoint, self.cord, sequence)
    }

    pub fn decisions(&self) -> u32 {
        self.scheduler.decisions()
    }
    pub fn signs(&self) -> u16 {
        self.scheduler.signs().len()
    }
}

impl SinkKernel {
    pub fn prepare(
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
    ) -> Result<Self, SchedulerError> {
        let endpoint = exact_endpoint(lowered, RemoteCordDirection::Ingress)?;
        if fragment.placements.len() != 1
            || fragment.placements[0].kind_id.as_str()
                != conduit_semantic_catalog::TEXT_PRESENTATION_KIND
            || lowered.node_specs.len() != 1
            || lowered.cords.len() != 1
            || !lowered.routes.is_empty()
            || lowered.host_operations.len() != 1
        {
            return Err(SchedulerError::InvalidPlan);
        }
        let mut routes = FixedRoutes::<1, 1>::new(PORTS as u16);
        routes.seal()?;
        let mut bindings = FixedHostOperationBindings::<1>::new(1);
        let operation = &lowered.host_operations[0];
        bindings.install(operation.node, operation.binding)?;
        bindings.seal()?;
        let scheduler = FixedScheduler::new_with_host_operations(
            [lowered.node_specs[0]],
            [lowered.cords[0].spec],
            routes,
            bindings,
            [conduit_kernel::scheduler::OperationDriver::new(
                PresentationOperation {
                    pending: false,
                    complete: false,
                },
            )?],
            FixedValueStore::<1, VALUE_BYTES>::new(VALUE_BYTES as u32)?,
            signs(lowered.sign_bytes)?,
        )?;
        Ok(Self {
            scheduler,
            endpoint: endpoint.endpoint,
            cord: endpoint.cord,
        })
    }

    pub fn admit(
        &mut self,
        sequence: u64,
        bytes: &[u8],
    ) -> Result<RemoteIngressOutcome, SchedulerError> {
        self.scheduler
            .admit_remote_input(self.endpoint, self.cord, sequence, bytes)
    }

    pub fn close(&mut self) -> Result<(), SchedulerError> {
        self.scheduler.close_remote_input(self.endpoint, self.cord)
    }

    pub fn step(&mut self) -> Result<SchedulerStatus, SchedulerError> {
        self.scheduler.step()
    }
    pub fn request(&mut self) -> Option<HostOperationRequest> {
        self.scheduler.next_host_request()
    }
    pub fn value(&self, request: HostOperationRequest) -> Result<&[u8], SchedulerError> {
        self.scheduler.host_value(request.input.value)
    }
    pub fn complete(&mut self, request: HostOperationRequest) -> Result<(), SchedulerError> {
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
    pub fn decisions(&self) -> u32 {
        self.scheduler.decisions()
    }
    pub fn signs(&self) -> u16 {
        self.scheduler.signs().len()
    }
}

fn exact_endpoint(
    lowered: &LoweredPlanFragment,
    direction: RemoteCordDirection,
) -> Result<&conduit_plan_lowering::lowering::LoweredRemoteEndpoint, SchedulerError> {
    let mut matches = lowered
        .remote_endpoints
        .iter()
        .filter(|endpoint| endpoint.direction == direction);
    let endpoint = matches.next().ok_or(SchedulerError::InvalidPlan)?;
    if matches.next().is_some() {
        return Err(SchedulerError::InvalidPlan);
    }
    Ok(endpoint)
}

fn signs(bytes: u32) -> Result<FixedSignLog<SIGN_CAPACITY>, SchedulerError> {
    Ok(FixedSignLog::new_with_remote_storage(
        bytes.max((SIGN_CAPACITY * core::mem::size_of::<KernelEvent>()) as u32),
        REMOTE_SIGN_CAPACITY,
        conduit_kernel::remote_sign_storage_bytes(REMOTE_SIGN_CAPACITY)
            .ok_or(SchedulerError::InvalidPlan)?,
    )?)
}
