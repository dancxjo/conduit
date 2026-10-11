//! Finite owner sampling outside Play. No observation grants resource capacity.
use super::{LocalModelRuntimeState, ModelResidencyState};
use crate::{StdHost, TimerAdapter};
use conduit_core::*;
use conduit_planner::resource_cost::*;

#[derive(Debug, Clone, Copy)]
pub struct ModelCostSamplingPolicy {
    pub validity_microseconds: u64,
    pub maximum_sampling_microseconds: u64,
    pub minimum_interval_microseconds: u64,
    pub maximum_attempts: u16,
}
pub struct ModelCostSampler {
    host: HostId,
    boot: BootId,
    generation: OfferGeneration,
    profile: HostProfileId,
    capability: CapabilityId,
    policy: ModelCostSamplingPolicy,
    attempts: u16,
    last: Option<MonotonicInstant>,
}
impl ModelCostSampler {
    pub fn admit(
        host: &HostAdvertisement,
        capability: &CapabilityId,
        policy: ModelCostSamplingPolicy,
    ) -> Result<Self, String> {
        if !(1..=1_000_000).contains(&policy.validity_microseconds)
            || !(1..=500_000).contains(&policy.maximum_sampling_microseconds)
            || !(1_000..=1_000_000).contains(&policy.minimum_interval_microseconds)
            || !(1..=64).contains(&policy.maximum_attempts)
        {
            return Err("invalid finite model sampling policy".into());
        }
        let offer = host
            .capabilities
            .iter()
            .find(|o| &o.capability_id == capability)
            .ok_or("sampling capability is absent")?;
        if offer.implementation.implementation_id.as_str() != conduit_ai::LOCAL_MODEL_IMPLEMENTATION
        {
            return Err("sampling capability is not a local-model realization".into());
        }
        Ok(Self {
            host: host.host_id.clone(),
            boot: host.boot_id.clone(),
            generation: host.offer_generation,
            profile: host.profile.clone(),
            capability: capability.clone(),
            policy,
            attempts: 0,
            last: None,
        })
    }
    pub fn attempts(&self) -> u16 {
        self.attempts
    }
}
/// Source borrows the exact current owner advertisement and admitted Sign/
/// calibration. The returned observation never substitutes for hard admission.
pub struct LocalModelExecutionSample<'a> {
    source: CostEvidenceSource<'a>,
    state: LocalModelRuntimeState,
    sampling: MonotonicDuration,
    resident_size: Option<RationalQuantity>,
    vram_size: Option<RationalQuantity>,
}
impl<'a> LocalModelExecutionSample<'a> {
    pub fn source(&self) -> &CostEvidenceSource<'a> {
        &self.source
    }
    pub fn state(&self) -> &LocalModelRuntimeState {
        &self.state
    }
    pub fn sampling_duration(&self) -> &MonotonicDuration {
        &self.sampling
    }
    pub fn resident_size(&self) -> Option<RationalQuantity> {
        self.resident_size
    }
    pub fn vram_size(&self) -> Option<RationalQuantity> {
        self.vram_size
    }
    pub fn execution_observation(
        &self,
    ) -> Result<CostExecutionObservation<'_>, CostEvidenceRefusal> {
        let condition = match self.state.residency {
            ModelResidencyState::NotResident => CostExecutionCondition::Cold,
            ModelResidencyState::Unknown | ModelResidencyState::ResidentWithoutEpoch => {
                CostExecutionCondition::Unknown
            }
        };
        CostExecutionObservation::admit(&self.source, condition)
    }
}
impl StdHost {
    /// Uses the existing owner-provided producing Sign and identified Timer
    /// basis. Neither initialization nor the provider's wall expiry is used as
    /// current residency. Each attempted query consumes the finite sampler.
    pub fn observe_local_model_execution<'a, T: TimerAdapter>(
        &'a self,
        sampler: &mut ModelCostSampler,
        sign: &'a SignId,
        calibration: CostCalibrationRevision<'a>,
        timer: &mut T,
    ) -> Result<LocalModelExecutionSample<'a>, String> {
        let host = self.advertisement();
        if host.host_id != sampler.host
            || host.boot_id != sampler.boot
            || host.offer_generation != sampler.generation
            || host.profile != sampler.profile
            || sign.as_str().is_empty()
            || sign.as_str().len() > MAXIMUM_COST_ID_BYTES
        {
            return Err("model sampling scope or producing Sign changed".into());
        }
        let offer = host
            .capabilities
            .iter()
            .find(|o| o.capability_id == sampler.capability)
            .ok_or("model sampling offer vanished")?;
        let adapter = self
            .local_model
            .as_deref()
            .ok_or("model sampling provider is not initialized")?;
        if !adapter
            .offer()
            .capability_offers()
            .map_err(|e| format!("current model profile: {e:?}"))?
            .iter()
            .any(|o| o == offer)
        {
            return Err("model sampling offer differs from initialized provider profile".into());
        }
        let started = timer
            .monotonic_observation(&host.host_id, &host.boot_id)
            .ok_or("sampling clock is unavailable")?;
        if started.clock().host_id() != &host.host_id
            || started.clock().boot_id() != &host.boot_id
            || started.clock().scale() != TemporalScale::Microseconds
        {
            return Err("sampling needs this Host/Boot's identified microsecond clock".into());
        }
        if let Some(last) = &sampler.last {
            let interval = started
                .elapsed_since(last)
                .map_err(|e| format!("sampling clock changed: {e:?}"))?;
            if interval.ticks() < sampler.policy.minimum_interval_microseconds {
                return Err("sampling update is too soon".into());
            }
        }
        if sampler.attempts >= sampler.policy.maximum_attempts {
            return Err("model sampling attempt bound exhausted".into());
        }
        sampler.attempts += 1;
        sampler.last = Some(started.clone());
        let state = adapter.current_runtime_state()?;
        let finished = timer
            .monotonic_observation(&host.host_id, &host.boot_id)
            .ok_or("sampling completion clock is unavailable")?;
        if started.clock() != finished.clock() {
            return Err("sampling clock quality changed".into());
        }
        let sampling = finished
            .elapsed_since(&started)
            .map_err(|e| format!("sampling interval: {e:?}"))?;
        if sampling.ticks() > sampler.policy.maximum_sampling_microseconds {
            return Err("model sample acquisition exceeded its age bound".into());
        }
        let expires = MonotonicInstant::new(
            started
                .ticks()
                .checked_add(sampler.policy.validity_microseconds)
                .ok_or("sampling expiry overflow")?,
            finished.clock().clone(),
        )
        .map_err(|e| format!("sampling expiry: {e:?}"))?;
        if finished.ticks() >= expires.ticks() {
            return Err("model sample expired during acquisition".into());
        }
        let window = CostObservationWindow::admit(started, expires)
            .map_err(|e| format!("sampling window: {e:?}"))?;
        let resource = host
            .resources
            .iter()
            .find(|resource| resource.class_id.as_str() == conduit_ai::LOCAL_MODEL_MEMORY_RESOURCE);
        let source = CostEvidenceSource::bind(host, offer, resource, sign, calibration, window)
            .map_err(|e| format!("sampling source: {e:?}"))?;
        let size = |bytes: Option<u64>| {
            bytes
                .map(|bytes| {
                    RationalQuantity::new(
                        ExactRational::from_i128(i128::from(bytes)),
                        Unit::resolve("B").map_err(|e| format!("byte Unit: {e:?}"))?,
                        QuantityRole::Linear,
                    )
                    .map_err(|e| format!("byte Quantity: {e:?}"))
                })
                .transpose()
        };
        let resident_size = size(state.resident_bytes)?;
        let vram_size = size(state.vram_bytes)?;
        Ok(LocalModelExecutionSample {
            source,
            state,
            sampling,
            resident_size,
            vram_size,
        })
    }
}
