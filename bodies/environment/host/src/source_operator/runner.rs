//! One checked sample invocation in the ordinary bounded kernel.
use super::*;
use conduit_environment_runtime::environmental_publication::{
    EnvironmentalPublicationBack, PublicationScope,
};
use conduit_kernel::{self as k, scheduler::*};
use conduit_plan_lowering::lowering::{lower_plan_fragment, FIXED_KERNEL_STORAGE_PORTS_PER_NODE};
const PORTS: usize = FIXED_KERNEL_STORAGE_PORTS_PER_NODE;
/// Collector-owned evidence, not authenticated remote membership or MCU time.
pub struct CommittedSample {
    pub(super) scope: PublicationScope,
    pub(super) sequence: u64,
    pub(super) canonical: Vec<u8>,
    session: BaseInstanceId,
}
impl CommittedSample {
    pub fn scope(&self) -> &PublicationScope {
        &self.scope
    }
    pub fn sequence(&self) -> u64 {
        self.sequence
    }
    pub fn session(&self) -> &BaseInstanceId {
        &self.session
    }
    pub fn canonical(&self) -> &[u8] {
        &self.canonical
    }
    /// Local collector execution establishes no remote Body membership.
    pub fn body(&self) -> Option<&conduit_body::BodyId> {
        None
    }
}
enum Back {
    Source(HostCallBack),
    Publication(EnvironmentalPublicationBack),
}
impl StepBack<PORTS> for Back {
    fn step(&mut self, io: &mut StepIo<PORTS>, input: &StepInputBytes<'_, PORTS>) -> StepOutcome {
        match self {
            Self::Source(b) => b.step(io, input),
            Self::Publication(b) => b.step(io, input),
        }
    }
    fn step_committed(&mut self) {
        match self {
            Self::Source(b) => <HostCallBack as StepBack<PORTS>>::step_committed(b),
            Self::Publication(b) => {
                <EnvironmentalPublicationBack as StepBack<PORTS>>::step_committed(b)
            }
        }
    }
    fn cancel(&mut self) {
        match self {
            Self::Source(b) => <HostCallBack as StepBack<PORTS>>::cancel(b),
            Self::Publication(b) => <EnvironmentalPublicationBack as StepBack<PORTS>>::cancel(b),
        }
    }
}
type Kernel = FixedScheduler<
    Back,
    k::HostedValueStore,
    k::HostedSignLog,
    2,
    2,
    PORTS,
    2,
    { 2 * PORTS },
    1,
    2,
    1,
>;
impl<P: EnvironmentalSourceProvider> UsbSourceRoot<P> {
    /// Preparation allocates finite kernel storage; the selected capture runs once.
    /// A failure drops possession instead of silently re-admitting or replaying it.
    pub async fn sample(&mut self) -> Result<CommittedSample, Refusal> {
        if self.failed {
            return Err(Refusal::Acquisition);
        }
        let advertisement = self.advertisement()?;
        self.provider
            .as_mut()
            .ok_or(Refusal::Acquisition)?
            .prepare_for_host(&advertisement)
            .map_err(|_| Refusal::Acquisition)?;
        let plan = self.plan()?;
        let fragment = plan.fragments.first().ok_or(Refusal::Plan)?;
        if plan.fragments.len() != 1 {
            return Err(Refusal::Plan);
        }
        let lowered = lower_plan_fragment(fragment).map_err(|_| Refusal::Plan)?;
        if lowered.nodes.len() != 2
            || lowered.cords.len() != 2
            || lowered.host_calls.len() != 1
            || lowered.fore_ports.len() != 1
            || !lowered.remote_endpoints.is_empty()
        {
            return Err(Refusal::Plan);
        }
        let source = fragment
            .placements
            .iter()
            .find(|g| g.kind_id.as_str() == HOST_CALL)
            .ok_or(Refusal::Plan)?;
        let sink=fragment.placements.iter().find(|g|g.kind_id.as_str()==conduit_environment_runtime::environmental_reading::source_observation::PUBLICATION_KIND).ok_or(Refusal::Plan)?;
        let i = &self.identity;
        let base = source.base.as_ref().ok_or(Refusal::Authority)?;
        if source.implementation_id != i.implementation_id
            || source.artifact_id != i.artifact_id
            || base.provider_instance_id != i.provider_instance_id
            || base.provider_generation != i.provider_generation
            || base.base_id != i.base_id
        {
            return Err(Refusal::Authority);
        }
        let issued = {
            let installed = &self.installed;
            installed
                .borrow_mut()
                .as_mut()
                .ok_or(Refusal::Acquisition)?
                .issue_kernel_play(fragment)
                .map_err(|_| Refusal::Authority)?
        };
        let active = issued.identity();
        let next = self.next_play.checked_add(1).ok_or(Refusal::Capacity)?;
        let _reservation = NativeReservation::acquire(self.installed.clone(), fragment)?;
        let (table, handle, claim) = self.issue(
            fragment,
            &issued,
            _reservation
                .lease
                .as_ref()
                .expect("active native reservation"),
        )?;
        let provider = self.provider.take().ok_or(Refusal::Acquisition)?;
        let contract = ObservationContract::prepare().map_err(|_| Refusal::Plan)?;
        // Actual same-process Root acquired and retained the exclusive USB provider,
        // independently checked its configuration and constructed this checked Plan.
        let binding = unsafe {
            EnvironmentalSourceHostCall::bind_selected(
                table,
                handle,
                claim,
                provider,
                &contract,
                SelectedOperationPlan {
                    fragment,
                    lowered: &lowered,
                    active: &active,
                    placement_id: &source.placement_id,
                },
            )
        };
        let mut owner = match binding {
            Ok(owner) => owner,
            Err((_, _, _, provider, _)) => {
                self.provider = Some(provider);
                self.failed = true;
                return Err(Refusal::Authority);
            }
        };
        self.next_play = next;
        let result = async {
            let publication = owner
                .prepare_publication(
                    SelectedOperationPlan {
                        fragment,
                        lowered: &lowered,
                        active: &active,
                        placement_id: &sink.placement_id,
                    },
                    1,
                )
                .map_err(|_| Refusal::Plan)?;
            let source_node = lowered
                .identity
                .node_for_placement(&source.placement_id)
                .ok_or(Refusal::Plan)?;
            let mut publication = Some(publication);
            let drivers = core::array::from_fn(|n| {
                if n == usize::from(source_node.0) {
                    Back::Source(HostCallBack::new(1))
                } else {
                    Back::Publication(publication.take().expect("one publication node"))
                }
            });
            let mut routes = k::FixedRoutes::<{ 2 * PORTS }, 1>::new(PORTS as u16);
            for r in &lowered.routes {
                routes
                    .install(r.source_node, r.source_port, r.range, &r.targets)
                    .map_err(|_| Refusal::Capacity)?;
            }
            routes.seal().map_err(|_| Refusal::Capacity)?;
            let mut calls = k::FixedHostCallBindings::<2>::new(1);
            calls
                .install(lowered.host_calls[0].node, lowered.host_calls[0].binding)
                .map_err(|_| Refusal::Capacity)?;
            calls.seal().map_err(|_| Refusal::Capacity)?;
            let values =
                k::HostedValueStore::new(4, MAXIMUM_BYTES as u32, 4 * MAXIMUM_BYTES as u32)
                    .map_err(|_| Refusal::Capacity)?;
            let signs = k::HostedSignLog::new_with_remote_storage(
                64,
                64 * core::mem::size_of::<k::KernelEvent>() as u32,
                8,
                k::remote_sign_storage_bytes(8).ok_or(Refusal::Capacity)?,
            )
            .map_err(|_| Refusal::Capacity)?;
            let mut kernel = Kernel::new_with_host_calls(
                lowered
                    .node_specs
                    .clone()
                    .try_into()
                    .map_err(|_| Refusal::Plan)?,
                lowered
                    .cords
                    .iter()
                    .map(|c| c.spec)
                    .collect::<Vec<_>>()
                    .try_into()
                    .map_err(|_| Refusal::Plan)?,
                routes,
                calls,
                drivers,
                values,
                signs,
            )
            .map_err(|_| Refusal::Capacity)?;
            let fore = &lowered.fore_ports[0];
            fore.validate_value(&[]).map_err(|_| Refusal::Plan)?;
            kernel
                .admit_remote_input(fore.endpoint, fore.cord, 0, &[])
                .map_err(|_| Refusal::Plan)?;
            kernel
                .close_remote_input(fore.endpoint, fore.cord)
                .map_err(|_| Refusal::Plan)?;
            let mut captured = false;
            for _ in 0..64 {
                let status = kernel.step().map_err(|_| Refusal::Plan)?;
                if let Some(request) = kernel.next_host_request() {
                    if captured || request.node != source_node {
                        return Err(Refusal::Plan);
                    }
                    captured = true;
                    let mut output = [0u8; MAXIMUM_BYTES];
                    let input = kernel
                        .host_value(request.input.value)
                        .map_err(|_| Refusal::Plan)?;
                    let len = owner
                        .invoke(
                            request.node,
                            request.call,
                            request.request,
                            input,
                            &mut output,
                        )
                        .await
                        .map_err(|_| Refusal::Acquisition)?;
                    let value = kernel
                        .store_host_value(&output[..len])
                        .map_err(|_| Refusal::Capacity)?;
                    kernel
                        .complete_host_call(
                            request.node,
                            request.request,
                            k::HostCallOutcome {
                                disposition: k::HostCallDisposition::Completed,
                                output: Some(
                                    k::BoundedValueRef::new(value, MAXIMUM_BYTES as u32)
                                        .map_err(|_| Refusal::Capacity)?,
                                ),
                                failure: None,
                            },
                        )
                        .map_err(|_| Refusal::Plan)?;
                }
                if status == SchedulerStatus::Drained {
                    let committed = kernel
                        .drivers()
                        .iter()
                        .find_map(|b| match b {
                            Back::Publication(p) => p.snapshot(),
                            _ => None,
                        })
                        .ok_or(Refusal::Plan)?;
                    let sample = CommittedSample {
                        scope: committed.scope().clone(),
                        sequence: next,
                        canonical: committed.canonical().to_vec(),
                        session: self.identity.provider_instance_id.clone(),
                    };
                    return Ok(sample);
                }
            }
            Err(Refusal::Capacity)
        }
        .await;
        owner.revoke().map_err(|_| Refusal::Authority)?;
        self.provider = Some(
            EnvironmentalSourceHostCall::retire_retained(&mut Some(owner))
                .map_err(|_| Refusal::Authority)?,
        );
        if result.is_err() {
            self.failed = true;
        }
        result
    }
}

struct NativeReservation {
    host: SharedHost,
    lease: Option<conduit_std_host::NativeFragmentReservation>,
}
impl NativeReservation {
    fn acquire(host: SharedHost, fragment: &PlanFragment) -> Result<Self, Refusal> {
        let lease = host
            .borrow_mut()
            .as_mut()
            .ok_or(Refusal::Acquisition)?
            .reserve_native_fragment(fragment)
            .map_err(|_| Refusal::Capacity)?;
        Ok(Self {
            host,
            lease: Some(lease),
        })
    }
}
impl Drop for NativeReservation {
    fn drop(&mut self) {
        if let (Some(host), Some(lease)) = (self.host.borrow_mut().as_mut(), self.lease.take()) {
            let _ = host.release_native_fragment(lease);
        }
    }
}
