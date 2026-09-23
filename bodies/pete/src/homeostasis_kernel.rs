//! Executable Pete homeostasis Gear and its admitted host-side reducer.

use crate::{
    decode_capability_observation, decode_power_observation, decode_pressure_observation,
    decode_reduction_instant, decode_safety_observation, decode_thermal_observation,
    reduce_homeostasis, HomeostasisPolicy, HomeostaticInputs,
};
use conduit_composite::{KernelOperationBudget, KernelOperationFactory};
use conduit_core::{ConfigurationValue, ImplementationId, PlannedGear};
use conduit_kernel::scheduler::{StepBack, StepInputBytes, StepIo, StepOutcome};
use conduit_kernel::{BoundedValueRef, HostCallDisposition, HostCallId, RequestId};
use conduit_plan_lowering::lowering::FIXED_KERNEL_STORAGE_PORTS_PER_NODE;

pub const HOMEOSTASIS_IMPLEMENTATION: &str = "conduit.pete/homeostasis-reducer@1";
pub const HOMEOSTASIS_HOST_CALL: HostCallId = HostCallId(0);
const INPUTS: u8 = 7;

pub struct HomeostasisKernelFactory {
    implementation_id: ImplementationId,
}

impl Default for HomeostasisKernelFactory {
    fn default() -> Self {
        Self {
            implementation_id: HOMEOSTASIS_IMPLEMENTATION.into(),
        }
    }
}

impl KernelOperationFactory for HomeostasisKernelFactory {
    fn implementation_id(&self) -> &ImplementationId {
        &self.implementation_id
    }

    fn budget(&self, _placement: &PlannedGear) -> Result<KernelOperationBudget, String> {
        Ok(KernelOperationBudget {
            value_items: 1,
            value_bytes: 8_192,
            maximum_value_bytes: 8_192,
            host_requests: 1,
            sign_items: 32,
        })
    }

    fn prepare(
        &self,
        placement: &PlannedGear,
        _values: &mut conduit_kernel::HostedValueStore,
    ) -> Result<Box<dyn StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> + Send>, String> {
        policy_from_placement(placement)?;
        Ok(Box::new(HomeostasisBack {
            phase: 0,
            pending: false,
        }))
    }
}

struct HomeostasisBack {
    phase: u8,
    pending: bool,
}

impl StepBack<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }> for HomeostasisBack {
    fn step(
        &mut self,
        io: &mut StepIo<{ FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
        _bytes: &StepInputBytes<'_, { FIXED_KERNEL_STORAGE_PORTS_PER_NODE }>,
    ) -> StepOutcome {
        if let Some((request, outcome)) = io.host_completion() {
            if !self.pending || request != RequestId(u32::from(self.phase) + 1) {
                return failure(1);
            }
            if outcome.disposition != HostCallDisposition::Completed || outcome.failure.is_some() {
                return failure(2);
            }
            if self.phase + 1 == INPUTS {
                let Some(output) = outcome.output else {
                    return failure(3);
                };
                if !io.output_ready(conduit_kernel::PortId(0)) {
                    return StepOutcome::Await;
                }
                if io.consume_host_completion().is_err()
                    || io.send(conduit_kernel::PortId(0), output.value).is_err()
                {
                    return failure(4);
                }
                self.pending = false;
                self.phase = INPUTS;
                return StepOutcome::Complete;
            }
            if outcome.output.is_some() || io.consume_host_completion().is_err() {
                return failure(5);
            }
            self.pending = false;
            self.phase += 1;
            return StepOutcome::Progress;
        }
        if self.phase == INPUTS {
            return StepOutcome::Complete;
        }
        if self.pending {
            return StepOutcome::Await;
        }
        let port = conduit_kernel::PortId(u16::from(self.phase));
        let Some(value) = io.input(port) else {
            return StepOutcome::Await;
        };
        let bounded = match BoundedValueRef::new(value, value.byte_len) {
            Ok(value) => value,
            Err(_) => return failure(6),
        };
        if io.consume(port).is_err() {
            return failure(6);
        }
        if io
            .request_host_call(
                RequestId(u32::from(self.phase) + 1),
                HOMEOSTASIS_HOST_CALL,
                bounded,
            )
            .is_err()
        {
            return failure(7);
        }
        self.pending = true;
        StepOutcome::Progress
    }
}

fn failure(detail: u16) -> StepOutcome {
    StepOutcome::Fail(conduit_kernel::Failure {
        code: conduit_kernel::FailureCode::InvalidLifecycle,
        detail,
    })
}

/// State owned by the host adapter for one admitted reducer instance.
pub struct HomeostasisHostAdapter {
    policy: HomeostasisPolicy,
    power: Option<conduit_human::SourceObservation<crate::PowerCondition>>,
    thermal: Option<conduit_human::SourceObservation<crate::ThermalCondition>>,
    compute: Option<conduit_human::SourceObservation<crate::PressureCondition>>,
    storage: Option<conduit_human::SourceObservation<crate::PressureCondition>>,
    safety: Option<conduit_human::SourceObservation<crate::SafetyCondition>>,
    capability: Option<conduit_human::SourceObservation<crate::CapabilityCondition>>,
}

impl HomeostasisHostAdapter {
    pub fn new(policy: HomeostasisPolicy) -> Self {
        Self {
            policy,
            power: None,
            thermal: None,
            compute: None,
            storage: None,
            safety: None,
            capability: None,
        }
    }

    pub fn from_placement(placement: &PlannedGear) -> Result<Self, String> {
        Ok(Self::new(policy_from_placement(placement)?))
    }

    /// Admit one exact phase. Only the final reduction-time phase emits bytes.
    pub fn handle(
        &mut self,
        request: RequestId,
        bytes: &[u8],
    ) -> Result<Option<Vec<u8>>, crate::HomeostasisRefusal> {
        match request.0 {
            1 => self.power = Some(decode_power_observation(bytes)?),
            2 => self.thermal = Some(decode_thermal_observation(bytes)?),
            3 => self.compute = Some(decode_pressure_observation(bytes)?),
            4 => self.storage = Some(decode_pressure_observation(bytes)?),
            5 => self.safety = Some(decode_safety_observation(bytes)?),
            6 => self.capability = Some(decode_capability_observation(bytes)?),
            7 => {
                let state = reduce_homeostasis(
                    &decode_reduction_instant(bytes)?,
                    &self.policy,
                    &HomeostaticInputs {
                        power: self
                            .power
                            .take()
                            .ok_or(crate::HomeostasisRefusal::InvalidObservation)?,
                        thermal: self
                            .thermal
                            .take()
                            .ok_or(crate::HomeostasisRefusal::InvalidObservation)?,
                        compute_pressure: self
                            .compute
                            .take()
                            .ok_or(crate::HomeostasisRefusal::InvalidObservation)?,
                        storage_pressure: self
                            .storage
                            .take()
                            .ok_or(crate::HomeostasisRefusal::InvalidObservation)?,
                        motion_safety: self
                            .safety
                            .take()
                            .ok_or(crate::HomeostasisRefusal::InvalidObservation)?,
                        important_capability: self
                            .capability
                            .take()
                            .ok_or(crate::HomeostasisRefusal::InvalidObservation)?,
                    },
                )?;
                return Ok(Some(
                    state
                        .canonical_value()?
                        .canonical_bytes()
                        .map_err(|_| crate::HomeostasisRefusal::EncodingCapacity)?,
                ));
            }
            _ => return Err(crate::HomeostasisRefusal::InvalidObservation),
        }
        Ok(None)
    }
}

fn policy_from_placement(placement: &PlannedGear) -> Result<HomeostasisPolicy, String> {
    let text = |key: &str| {
        placement
            .configuration
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| match &entry.value {
                ConfigurationValue::Text(value) => Some(value.clone()),
                _ => None,
            })
            .ok_or_else(|| format!("missing {key}"))
    };
    let count = |key: &str| {
        placement
            .configuration
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| match entry.value {
                ConfigurationValue::U64(value) => u16::try_from(value).ok(),
                _ => None,
            })
            .ok_or_else(|| format!("missing {key}"))
    };
    let scalar = |key: &str| {
        placement
            .configuration
            .iter()
            .find(|entry| entry.key == key)
            .and_then(|entry| match entry.value {
                ConfigurationValue::I64(value) => i32::try_from(value).ok(),
                _ => None,
            })
            .ok_or_else(|| format!("missing {key}"))
    };
    Ok(HomeostasisPolicy {
        revision: text("policy-revision")?,
        energy_low_permille: count("energy-low-permille")?,
        energy_critical_permille: count("energy-critical-permille")?,
        thermal_constrained_milli_celsius: scalar("thermal-constrained-milli-celsius")?,
        thermal_critical_milli_celsius: scalar("thermal-critical-milli-celsius")?,
        pressure_high_permille: count("pressure-high-permille")?,
        pressure_critical_permille: count("pressure-critical-permille")?,
    })
}

pub fn decode_homeostatic_state(
    bytes: &[u8],
) -> Result<conduit_core::StructuredInfoValue, crate::HomeostasisRefusal> {
    let value = conduit_core::StructuredInfoValue::from_canonical_bytes(bytes)
        .map_err(|_| crate::HomeostasisRefusal::InvalidObservation)?;
    if value.value_type() != &crate::homeostatic_state_type() {
        return Err(crate::HomeostasisRefusal::InvalidObservation);
    }
    Ok(value)
}
