//! Lifetime-owned initialized region service over one idle Std Host.
use super::{worker, KernelResourceLedger};
use crate::StdHost;
use conduit_core::{
    BaseLifecycle, BaseProviderEntry, BaseRegistry, HostAdvertisement, OfferGeneration,
};
#[path = "service/activation.rs"]
mod activation;
#[path = "service/controller.rs"]
mod controller;
#[path = "service/execution.rs"]
mod execution;
pub use controller::{RegionAdvance, RegionRoundState};
pub use execution::RegionExecution;
#[path = "service/offers.rs"]
mod offers;
#[path = "service/preparation.rs"]
mod preparation;
#[path = "service/planning.rs"]
mod planning;
pub use activation::RegionWorkerCommand;
pub use offers::RegionServiceConfig;
pub use worker::WorkerError as RegionWorkerError;
pub use worker::WorkerReply as RegionWorkerReply;

pub struct RegionWorkerEvidence {
    pub lane: usize,
    pub plan_id: Option<conduit_core::PlanId>,
    pub region_id: Option<conduit_core::ExecutionRegionId>,
    pub original_nodes: Vec<conduit_kernel::NodeId>,
    pub original_cords: Vec<conduit_kernel::CordId>,
    pub local_signs: Vec<conduit_kernel::KernelEvent>,
    pub remote_signs: Vec<(u32, conduit_kernel::RemoteLifecycleIdentity)>,
    pub failure: Option<RegionWorkerError>,
    pub cancellation: Option<Result<(), RegionWorkerError>>,
}
#[derive(Default)]
pub struct RegionServiceRetirement {
    pub active_play: Option<conduit_core::ActivePlayIdentity>,
    pub controller_error: Option<RegionWorkerError>,
    pub last_round: Option<RegionRoundState>,
    pub plan: Option<conduit_core::Plan>,
    pub admission_observations: Vec<conduit_core::ResourceObservation>,
    pub workers: Vec<RegionWorkerEvidence>,
    pub host_error: Option<String>,
}

impl RegionServiceRetirement {
    fn record_host_error(&mut self, message: String) {
        if let Some(existing) = &mut self.host_error {
            existing.push_str("; ");
            existing.push_str(&message);
        } else {
            self.host_error = Some(message);
        }
    }
}

/// The mutable Host borrow prevents ordinary runs or offer refresh from racing
/// this initialized service. Thread/control costs are bounded Base backing;
/// per-Play worker memory is a separate advertised managed quota.
pub struct InitializedRegionService<'host> {
    host: &'host mut StdHost,
    previous_advertisement: Option<HostAdvertisement>,
    previous_registry: Option<BaseRegistry>,
    provider: BaseProviderEntry,
    shutdown_generation: OfferGeneration,
    config: RegionServiceConfig,
    workers: Vec<worker::InitializedRegionWorker>,
    control_backing_bytes: usize,
    plan: Option<conduit_core::Plan>,
    active_play: Option<conduit_core::ActivePlayIdentity>,
    reservation: Option<super::region_resources::RegionResourceReservation>,
    admission_observations: Vec<conduit_core::ResourceObservation>,
    prepared_count: usize,
    retired: Option<RegionServiceRetirement>,
    next_observation_sequence: u64,
}
impl<'host> InitializedRegionService<'host> {
    pub fn initialize(
        host: &'host mut StdHost,
        config: RegionServiceConfig,
    ) -> Result<Self, String> {
        config.validate()?;
        if !host.kernel_resources.is_idle() {
            return Err("worker service cannot replace active Host reservations".into());
        }
        let mut workers = Vec::with_capacity(usize::from(config.lanes));
        for _ in 0..config.lanes {
            workers.push(worker::InitializedRegionWorker::spawn(
                config.stack_bytes,
                config.transfer_bytes,
            )?);
        }
        let ready = offers::initialize(&host.advertisement, &host.base_registry, config, &workers)?;
        let ledger = KernelResourceLedger::new(&ready.advertisement)?;
        let previous_advertisement =
            std::mem::replace(&mut host.advertisement, ready.advertisement);
        let previous_registry = std::mem::replace(&mut host.base_registry, ready.registry);
        host.kernel_resources = ledger;
        Ok(Self {
            host,
            previous_advertisement: Some(previous_advertisement),
            previous_registry: Some(previous_registry),
            provider: ready.provider,
            shutdown_generation: ready.shutdown_generation,
            config,
            workers,
            control_backing_bytes: ready.control_backing_bytes,
            plan: None,
            active_play: None,
            reservation: None,
            admission_observations: Vec::new(),
            prepared_count: 0,
            retired: None,
            next_observation_sequence: 0,
        })
    }
    pub fn advertisement(&self) -> Result<&HostAdvertisement, String> {
        if self.previous_advertisement.is_none() || self.workers.iter().any(|w| !w.is_alive()) {
            return Err("worker service is not available".into());
        }
        Ok(&self.host.advertisement)
    }
    pub fn config(&self) -> RegionServiceConfig {
        self.config
    }
    pub fn control_backing_bytes(&self) -> usize {
        self.control_backing_bytes
    }
    pub fn base_id(&self) -> &conduit_core::HostBaseId {
        &self.provider.base_id
    }
    pub fn shutdown(mut self) -> RegionServiceRetirement {
        self.retire()
    }
    fn retire(&mut self) -> RegionServiceRetirement {
        let mut result = RegionServiceRetirement::default();
        let Some(mut previous) = self.previous_advertisement.take() else {
            return self.retired.take().unwrap_or_default();
        };
        self.prepared_count = 0;
        result.plan = self.plan.take();
        result.active_play = self.active_play.take();
        result.admission_observations = std::mem::take(&mut self.admission_observations);
        for (lane, worker) in self.workers.drain(..).enumerate() {
            let retired = worker.shutdown();
            let mut evidence = RegionWorkerEvidence {
                lane,
                plan_id: None,
                region_id: None,
                original_nodes: Vec::new(),
                original_cords: Vec::new(),
                local_signs: Vec::new(),
                remote_signs: Vec::new(),
                failure: None,
                cancellation: None,
            };
            match retired {
                Ok(retired) => {
                    evidence.failure = retired.failure;
                    evidence.cancellation = retired.cancellation;
                    if let Some(kernel) = retired.kernel {
                        evidence.plan_id = Some(kernel.plan_id().clone());
                        evidence.region_id = Some(kernel.region_id().clone());
                        evidence.original_nodes = kernel.original_nodes().to_vec();
                        evidence.original_cords = kernel.original_cords().to_vec();
                        evidence.local_signs = kernel.signs();
                        evidence.remote_signs = kernel.remote_signs();
                        // Back/value/Sign backing is dropped before quota release.
                    }
                }
                Err(error) => evidence.failure = Some(error),
            }
            result.workers.push(evidence);
        }
        if let Some(reservation) = self.reservation.take() {
            if let Err(error) = self
                .host
                .kernel_resources
                .release_region_resources(reservation)
            {
                result.record_host_error(format!("worker resource retirement: {error}"));
            }
        }
        if let Err(error) = self.host.base_registry.set_lifecycle(
            &self.provider.base_id,
            &self.provider.provider_instance_id,
            self.provider.provider_generation,
            BaseLifecycle::Stopped,
        ) {
            result.record_host_error(format!("worker Base retirement: {error:?}"));
            if let Some(registry) = self.previous_registry.take() {
                self.host.base_registry = registry;
            }
        }
        previous.offer_generation = self.shutdown_generation;
        self.host.kernel_resources = KernelResourceLedger::new(&previous)
            .expect("the previously initialized Host ledger remains valid");
        self.host.advertisement = previous;
        result
    }
}
impl Drop for InitializedRegionService<'_> {
    fn drop(&mut self) {
        let _ = self.retire();
    }
}
