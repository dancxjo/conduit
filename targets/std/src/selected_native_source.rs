//! One independently initialized native Source in an installed Host.
//!
//! This entrance does not turn an offer, callback or observation into authority.
//! The retained Base, native definition and grant precede planning; possession is
//! issued only for the current registry entry and the actual installed Play.
mod installation;
mod worker;
use conduit_core::*;
use conduit_kernel::NodeId;
use conduit_plan_lowering::{lowering::LoweredPlanFragment, selected_operation::*};
pub use worker::{
    NativeReadContext, NativeSourceWorkerProvider, NativeWorkerBudget, WorkerBackingObservation,
    WorkerFailure, NATIVE_SOURCE_WORKER_PROFILE,
};

/// Independently retained definition of a single-call native Source.
/// All fields describe the initialized provider, never received Plan bytes.
pub struct NativeSourceDefinition {
    pub kind: Kind,
    pub base: BaseProviderEntry,
    pub grant: AuthorityGrant,
    pub resource_generation: ResourceGenerationId,
    pub configuration: Vec<ConfigurationEntry>,
    pub input_bytes: u32,
    pub output_bytes: u32,
    pub resource_bytes: u64,
}

/// Native correspondence boundary for an immediate bounded CPU Source.
///
/// # Safety
/// Implementations must represent the actual independently initialized owner
/// described by `NativeSourceDefinition`, retain continuity across child Plays,
/// and return within its admitted finite native execution budget without IO,
/// sleeping, allocation, unwinding, background work or retained borrows. Physical IO must
/// use a separately admitted, cancellable worker; this immediate entrance is
/// deliberately insufficient for a blocking USB sensor capture.
pub unsafe trait ImmediateNativeSource: Send {
    fn read(&mut self, input: &[u8], output: &mut [u8]) -> Result<usize, String>;
}

pub(crate) struct InitializedNativeSource {
    definition: NativeSourceDefinition,
    provider: NativeProvider,
    pub(super) backing: Option<crate::kernel_preparation::BaseBackingReservation>,
}
enum NativeProvider {
    Immediate(Box<dyn ImmediateNativeSource>),
    Worker(worker::NativeWorker),
}

impl crate::StdHost {
    /// Install only while the actual finite resource ledger is idle. Refusal
    /// returns the affine native owner unchanged; replacing an existing session
    /// requires its separately implemented quiescent retirement path.
    ///
    /// # Safety
    /// The caller must establish correspondence between this exact definition
    /// and the actual initialized native provider, including its owned finite
    /// memory/execution budget and retained source continuity. The grant must
    /// come from independent current Host authorization, not an incoming Plan,
    /// observer receipt, or callback. These native facts cannot be inferred
    /// from the descriptive IDs in `definition`.
    #[allow(clippy::result_large_err)]
    pub unsafe fn install_immediate_native_source(
        &mut self,
        definition: NativeSourceDefinition,
        provider: Box<dyn ImmediateNativeSource>,
    ) -> Result<
        (),
        (
            NativeSourceDefinition,
            Box<dyn ImmediateNativeSource>,
            String,
        ),
    > {
        let prepared = self.stage_native_source(&definition, None);
        match prepared {
            Err(reason) => Err((definition, provider, reason)),
            Ok((registry, advertisement, ledger)) => {
                self.base_registry = registry;
                self.advertisement = advertisement;
                self.kernel_resources = ledger;
                self.native_source = Some(InitializedNativeSource {
                    definition,
                    provider: NativeProvider::Immediate(provider),
                    backing: None,
                });
                Ok(())
            }
        }
    }
}

fn validate_definition(
    d: &NativeSourceDefinition,
    host: &HostAdvertisement,
    worker_bytes: Option<u32>,
) -> Result<(), String> {
    d.kind
        .validate()
        .map_err(|error| format!("native Source Kind: {error:?}"))?;
    if d.base.lifecycle != BaseLifecycle::Ready
        || d.base.capabilities.len() != 1
        || d.base.resources.len() != if worker_bytes.is_some() { 2 } else { 1 }
        || d.base.provider_generation == 0
        || d.resource_generation.0.is_empty()
        || d.input_bytes == 0
        || d.output_bytes == 0
        || d.input_bytes.checked_add(d.output_bytes).is_none()
        || d.resource_bytes == 0
        || d.kind.inputs.len() != 1
        || d.kind.outputs.len() != 1
    {
        return Err("native Source requires one finite ready provider, call and resource".into());
    }
    if let Some(required) = worker_bytes {
        let memory = &d.base.resources[1];
        if memory.class_id.as_str() != RUNTIME_MEMORY_RESOURCE_CLASS
            || memory.capacity_units < required
            || memory.pool_id == d.base.resources[0].pool_id
            || memory.compute.is_some()
            || memory.content.is_some()
        {
            return Err("native worker's independently configured backing pool is insufficient or mismatched".into());
        }
    }
    let offer = &d.base.capabilities[0];
    let grant = &d.grant;
    if offer.capability_id.as_str().is_empty()
        || offer
            .implementation
            .execution_profile_id
            .as_str()
            .is_empty()
        || offer.implementation.implementation_id.as_str().is_empty()
        || offer.implementation.artifact_id.as_str().is_empty()
        || grant.grant_id.as_str().is_empty()
        || offer.startup_parameters != d.kind.startup_parameters
        || offer.shorthand != d.kind.shorthand
        || offer.limits != d.kind.limits
        || offer.limits.max_active_instances != 1
        || offer.kind_id != d.kind.kind_id
        || offer.kind_contract_revision != d.kind.kind_contract_revision
        || offer.inputs != d.kind.inputs
        || offer.outputs != d.kind.outputs
        || offer.semantic_contract != d.kind.semantic_contract()
        || offer.implementation.implementation_id.as_str() != d.base.implementation_id.as_str()
        || offer.host_calls.len() != 1
        || offer.resource_requirements.len() != 1
        || offer.authority_requirements.len() != 1
        || offer.host_calls[0].maximum_input_bytes != d.input_bytes
        || offer.host_calls[0].maximum_output_bytes != d.output_bytes
        || offer.host_calls[0].maximum_in_flight != 1
        || offer.host_calls[0].target_kind.as_ref() != Some(&d.kind.kind_id)
        || grant.host_id != host.host_id
        || grant.boot_id != host.boot_id
        || grant.capability_id != offer.capability_id
        || grant.subject_kind != d.kind.kind_id
        || grant.host_call_contract_id != offer.host_calls[0].contract_id
        || grant.contract_id != offer.authority_requirements[0].contract_id
        || grant.host_call_contract_id != offer.authority_requirements[0].host_call_contract_id
        || grant.subject_kind != offer.authority_requirements[0].subject_kind
        || offer.resource_requirements[0].units != 1
        || d.base.resources[0].capacity_units != 1
        || d.base.resources[0].class_id != offer.resource_requirements[0].class_id
    {
        return Err("native Source definition, grant and initialized Base disagree".into());
    }
    Ok(())
}

impl InitializedNativeSource {
    pub(crate) fn matches(&self, gear: &PlannedGear) -> bool {
        let d = &self.definition;
        let offer = &d.base.capabilities[0];
        gear.capability_id == offer.capability_id
            && gear.kind_id == d.kind.kind_id
            && gear.implementation_id == offer.implementation.implementation_id
            && gear.artifact_id == offer.implementation.artifact_id
    }
    pub(crate) fn budget(&self) -> crate::installed_std::factory::BackBudget {
        let d = &self.definition;
        crate::installed_std::factory::BackBudget {
            value_items: 2,
            value_bytes: d.input_bytes.saturating_add(d.output_bytes),
            host_requests: 1,
            sign_items: 16,
            maximum_value_bytes: d.input_bytes.max(d.output_bytes),
        }
    }
    pub(crate) fn prepare<'a>(
        &'a mut self,
        registry: &BaseRegistry,
        host: &HostAdvertisement,
        fragment: &PlanFragment,
        lowered: &LoweredPlanFragment,
        active: &ActivePlayIdentity,
    ) -> Result<Option<PreparedNativeSource<'a>>, String> {
        let d = &self.definition;
        let mut gears = fragment.placements.iter().filter(|gear| self.matches(gear));
        let Some(gear) = gears.next() else {
            return Ok(None);
        };
        if let NativeProvider::Worker(worker) = &self.provider {
            if !worker.ready_for_play() {
                return Err("native worker is not ready and quiescent for a child Play".into());
            }
        }
        if gears.next().is_some() {
            return Err("native Source slot supports exactly one placement".into());
        }
        if !registry.entries().iter().any(|entry| entry == &d.base)
            || host.host_id != d.grant.host_id
            || host.boot_id != d.grant.boot_id
            || fragment.offer_generation != host.offer_generation
            || gear.base.as_ref()
                != host
                    .bases
                    .iter()
                    .find(|base| base.provider_instance_id == d.base.provider_instance_id)
                    .map(|base| base.binding())
                    .as_ref()
            || gear.resources.len() != 1
            || gear.resources[0].pool_id != d.base.resources[0].pool_id
            || gear.resources[0].class_id != d.base.resources[0].class_id
            || gear.resources[0].units != 1
            || gear.authority.len() != 1
        {
            return Err("native Source selected a stale registry, offer, grant or resource".into());
        }
        let a = &gear.authority[0];
        let g = &d.grant;
        if a.grant_id != g.grant_id
            || a.contract_id != g.contract_id
            || a.host_call_contract_id != g.host_call_contract_id
            || a.subject_kind != g.subject_kind
            || a.host_id != g.host_id
            || a.boot_id != g.boot_id
            || a.capability_id != g.capability_id
        {
            return Err(
                "native Source Plan does not possess the independently installed grant".into(),
            );
        }
        let envelope: CapabilityEnvelopeId = "std/native-source/exact@1".into();
        let scope = BaseCapabilityScope {
            host_id: host.host_id.clone(),
            boot_id: host.boot_id.clone(),
            base_instance_id: d.base.provider_instance_id.clone(),
            base_provider_generation: d.base.provider_generation,
            plan_id: fragment.plan_id.clone(),
            active_play_id: active.active_play_id.clone(),
            authority_grant_id: g.grant_id.clone(),
            authority_contract_id: g.contract_id.clone(),
            capability_id: g.capability_id.clone(),
            implementation_id: gear.implementation_id.clone(),
            operation_contract_id: g.host_call_contract_id.clone(),
            subject_kind: g.subject_kind.clone(),
            resource_pool_id: d.base.resources[0].pool_id.clone(),
            resource_generation_id: d.resource_generation.clone(),
            envelope_id: envelope.clone(),
            maximum_parameter_bytes: d.input_bytes,
            maximum_result_bytes: d.output_bytes,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let authority = BaseCapabilityAuthority {
            grant: g.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            envelope_id: envelope,
            maximum_parameter_bytes: d.input_bytes,
            maximum_result_bytes: d.output_bytes,
            maximum_work_units: 1,
            maximum_in_flight: 1,
            maximum_operations: 1,
        };
        let claim = BaseOperationClaim {
            host_id: scope.host_id.clone(),
            boot_id: scope.boot_id.clone(),
            base_instance_id: scope.base_instance_id.clone(),
            base_provider_generation: scope.base_provider_generation,
            plan_id: scope.plan_id.clone(),
            active_play_id: scope.active_play_id.clone(),
            implementation_id: scope.implementation_id.clone(),
            operation_contract_id: scope.operation_contract_id.clone(),
            subject_kind: scope.subject_kind.clone(),
            resource_pool_id: scope.resource_pool_id.clone(),
            resource_generation_id: scope.resource_generation_id.clone(),
            envelope_id: scope.envelope_id.clone(),
            parameter_bytes: d.input_bytes,
            work_units: 1,
        };
        let mut issuer = [0; 32];
        getrandom::fill(&mut issuer)
            .map_err(|error| format!("native Source issuer entropy: {error}"))?;
        let mut table = BaseCapabilityTable::new(
            host.host_id.clone(),
            host.boot_id.clone(),
            d.base.provider_instance_id.clone(),
            d.base.provider_generation,
            issuer,
            1,
        )
        .map_err(|error| format!("native Source table: {error:?}"))?;
        let handle = table
            .issue(CapabilityIssueRequest { scope, authority })
            .map_err(|error| format!("native Source possession: {error:?}"))?;
        let node = bind_selected_configured_operation(
            &table,
            &claim,
            SelectedOperationContract {
                kind: &d.kind,
                call: g.host_call_contract_id.as_str(),
                input_bytes: d.input_bytes,
                output_bytes: d.output_bytes,
                resource_bytes: d.resource_bytes,
            },
            &d.configuration,
            SelectedOperationPlan {
                fragment,
                lowered,
                active,
                placement_id: &gear.placement_id,
            },
        )
        .map_err(|error| format!("native Source selection: {error:?}"))?;
        let operation = BoundBaseOperation::bind(table, handle, claim)
            .map_err(|(_, _, _, error)| format!("native Source binding: {error:?}"))?;
        Ok(Some(PreparedNativeSource {
            node,
            operation,
            provider: &mut self.provider,
            output: vec![0; d.output_bytes as usize].into_boxed_slice(),
            next_request: 0,
            pending: None,
            cancelled: false,
            cancellation_started: None,
        }))
    }
}

pub(crate) struct PreparedNativeSource<'a> {
    pub(crate) node: NodeId,
    operation: BoundBaseOperation,
    provider: &'a mut NativeProvider,
    output: Box<[u8]>,
    next_request: u32,
    pending: Option<NativePending>,
    cancelled: bool,
    cancellation_started: Option<std::time::Instant>,
}
struct NativePending {
    request: conduit_kernel::scheduler::HostCallRequest,
    generation: u64,
    lease: BaseOperationLease,
}
impl PreparedNativeSource<'_> {
    pub(crate) fn input_bytes(&self) -> u32 {
        self.operation.inspection().scope.maximum_parameter_bytes
    }
    pub(crate) fn is_pending(&self) -> bool {
        self.pending.is_some()
    }
    pub(crate) fn cancel(&mut self) {
        self.cancelled = true;
        self.cancellation_started
            .get_or_insert_with(std::time::Instant::now);
        if let NativeProvider::Worker(worker) = self.provider {
            worker.cancel();
        }
    }
    pub(crate) fn read(
        &mut self,
        request: conduit_kernel::scheduler::HostCallRequest,
        input: &[u8],
    ) -> Result<Option<&[u8]>, String> {
        if self.cancelled
            || self.pending.is_some()
            || request.node != self.node
            || request.call != conduit_kernel::HostCallId(0)
            || request.request != conduit_kernel::RequestId(self.next_request)
            || input.len() > self.operation.inspection().scope.maximum_parameter_bytes as usize
        {
            return Err("native Source request differs from selected call".into());
        }
        let lease = self
            .operation
            .authorize()
            .map_err(|e| format!("native Source authorization: {e:?}"))?;
        self.next_request = self
            .next_request
            .checked_add(1)
            .ok_or("native Source request sequence exhausted")?;
        match self.provider {
            NativeProvider::Immediate(provider) => {
                let result = provider.read(input, &mut self.output);
                let bytes = result.as_ref().copied().unwrap_or(0);
                let accepted = if bytes <= self.output.len() { bytes } else { 0 };
                self.operation
                    .complete(lease, accepted as u32)
                    .map_err(|e| format!("native Source completion: {e:?}"))?;
                if bytes > self.output.len() {
                    self.operation
                        .revoke()
                        .map_err(|e| format!("native Source revoke: {e:?}"))?;
                    return Err("native Source exceeded its fixed result slot".into());
                }
                result?;
                Ok(Some(&self.output[..bytes]))
            }
            NativeProvider::Worker(worker) => match worker.submit(input) {
                Ok(generation) => {
                    self.pending = Some(NativePending {
                        request,
                        generation,
                        lease,
                    });
                    Ok(None)
                }
                Err(error) => {
                    self.operation
                        .complete(lease, 0)
                        .map_err(|e| format!("native Source unsubmitted completion: {e:?}"))?;
                    self.operation
                        .revoke()
                        .map_err(|e| format!("native Source unsubmitted revoke: {e:?}"))?;
                    Err(format!("native Source submit: {error:?}"))
                }
            },
        }
    }
    pub(crate) fn poll(
        &mut self,
    ) -> Result<Option<(conduit_kernel::scheduler::HostCallRequest, &[u8])>, String> {
        let Some(pending) = self.pending.as_ref() else {
            return Ok(None);
        };
        let NativeProvider::Worker(worker) = self.provider else {
            return Err("immediate Source cannot own pending IO".into());
        };
        if self
            .cancellation_started
            .is_some_and(|started| started.elapsed() > worker.cancellation_timeout())
        {
            worker.mark_cancellation_overdue();
        }
        let result = match worker.poll(pending.generation, &mut self.output) {
            Ok(None) => return Ok(None),
            Ok(Some(result)) => result,
            Err(error) => {
                if !worker.is_quiescent() {
                    worker.cancel();
                    return Ok(None);
                }
                Err(error)
            }
        };
        let pending = self.pending.take().expect("observed pending owner");
        let bytes = if self.cancelled {
            0
        } else {
            result.as_ref().copied().unwrap_or(0)
        };
        self.operation
            .complete(pending.lease, bytes as u32)
            .map_err(|e| format!("native worker completion: {e:?}"))?;
        if self.cancelled || result.is_err() {
            self.operation
                .revoke()
                .map_err(|e| format!("native worker revoke: {e:?}"))?;
            return if self.cancelled {
                Ok(None)
            } else {
                Err(format!("native worker read: {:?}", result.err().unwrap()))
            };
        }
        Ok(Some((pending.request, &self.output[..bytes])))
    }
}
impl Drop for PreparedNativeSource<'_> {
    fn drop(&mut self) {
        if self.pending.is_none() {
            return;
        }
        self.cancel();
        // Error/terminal cleanup cannot detach a live native read or free its
        // lease. The cooperative cancellation bound is observable, not proof
        // of quiescence: a stubborn provider keeps this ownership retained.
        while self.pending.is_some() {
            let _ = self.poll();
            if self.pending.is_some() {
                std::thread::park_timeout(std::time::Duration::from_millis(1));
            }
        }
    }
}

/// Read-only evidence of this Host's actual retained native worker backing.
/// It is not permission to allocate a worker or invoke a Source.
#[derive(Clone, Copy, Debug)]
pub struct NativeWorkerInspection {
    pub backing: WorkerBackingObservation,
    pub reserved_bytes: u32,
    pub idle_backing_is_reserved: bool,
    pub cancellation_overdue: bool,
}
pub struct RetiredNativeSource {
    pub definition: NativeSourceDefinition,
    /// None means the native provider was lost; no replacement is invented.
    pub provider: Option<Box<dyn NativeSourceWorkerProvider>>,
}
