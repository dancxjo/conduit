//! Exact finite demand custody before a forecast can affect preference.
use super::{
    CostEvidenceRefusal as Refusal, CostEvidenceSource, CostWorkClass, FulfillmentCostForecast,
};
use conduit_core::{FrontValueLocation, PortId, PortTemporal, ValuePayload};
use conduit_plot::CheckedGear;

pub const MAXIMUM_COST_INPUTS: usize = 32;
pub const MAXIMUM_COST_INPUT_BYTES: usize = 65_536;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct CostInput<'a> {
    pub port: &'a PortId,
    pub value: &'a ValuePayload,
}
/// Exact finite input sequence, not a user-selected workload-class label or raw
/// rate. Forecast reuse requires identical checked meaning and input bytes.
/// Flow inputs describe this finite submitted prefix, not the whole stream.
#[derive(Clone, Copy)]
pub struct CostDemand<'a> {
    class: CostWorkClass<'a>,
    inputs: &'a [CostInput<'a>],
    bytes: usize,
}
impl<'a> CostDemand<'a> {
    pub fn admit(gear: &'a CheckedGear, inputs: &'a [CostInput<'a>]) -> Result<Self, Refusal> {
        if inputs.len() > MAXIMUM_COST_INPUTS {
            return Err(Refusal::InputLimit);
        }
        let bytes = inputs
            .iter()
            .try_fold(0usize, |sum, input| {
                sum.checked_add(input.value.encoded.len())
            })
            .ok_or(Refusal::InputLimit)?;
        if bytes > MAXIMUM_COST_INPUT_BYTES {
            return Err(Refusal::InputLimit);
        }
        for input in inputs {
            let port = gear
                .inputs
                .iter()
                .find(|port| &port.port_id == input.port)
                .ok_or(Refusal::InvalidWorkInput)?;
            if port.value_kind != input.value.value_kind {
                return Err(Refusal::InvalidWorkInput);
            }
            let contract = gear
                .semantic_contract
                .value_contracts()
                .iter()
                .find(|contract| {
                    contract.location == FrontValueLocation::Input(input.port.clone())
                });
            let valid = if let Some(contract) = contract {
                contract.contract.value_kind == input.value.value_kind
                    && contract.contract.validate(&input.value.encoded).is_ok()
            } else {
                conduit_core::validate_primitive_info(
                    input.value.value_kind.as_str(),
                    &input.value.encoded,
                )
                .is_ok()
            };
            if !valid {
                return Err(Refusal::InvalidWorkInput);
            }
        }
        // A Value or Current input is one complete input, never a missing value
        // silently treated as a zero-size job. Flow cardinality remains explicit.
        for port in &gear.inputs {
            if !matches!(port.temporal, PortTemporal::Flow { .. })
                && inputs
                    .iter()
                    .filter(|input| input.port == &port.port_id)
                    .count()
                    != 1
            {
                return Err(Refusal::InvalidWorkInput);
            }
        }
        Ok(Self {
            class: CostWorkClass::from_checked_gear(gear),
            inputs,
            bytes,
        })
    }
    pub const fn class(self) -> CostWorkClass<'a> {
        self.class
    }
    pub const fn inputs(self) -> &'a [CostInput<'a>] {
        self.inputs
    }
    pub const fn input_bytes(self) -> usize {
        self.bytes
    }
    pub fn validate_match(self, demand: Self) -> Result<(), Refusal> {
        self.class.validate_demand(demand.class.gear())?;
        if self.inputs != demand.inputs {
            return Err(Refusal::WorkExtentChanged);
        }
        Ok(())
    }
}
/// Forecast for precisely the admitted demand. The source must have been
/// admitted by the producer-trust boundary; this wrapper creates no authority.
pub struct CostedForecast<'a> {
    source: &'a CostEvidenceSource<'a>,
    demand: CostDemand<'a>,
    forecast: &'a FulfillmentCostForecast,
    calibration: Option<&'a super::CostCalibrationEstimate<'a>>,
}
// Evidence diagnostics expose size and scope, not prompt/input contents or a
// repeated copy of the complete advertised inventory.
impl core::fmt::Debug for CostDemand<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CostDemand")
            .field("kind", &self.class.gear().kind_id)
            .field("contract", &self.class.gear().kind_contract_revision)
            .field("input_count", &self.inputs.len())
            .field("input_bytes", &self.bytes)
            .finish()
    }
}
impl core::fmt::Debug for CostedForecast<'_> {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        f.debug_struct("CostedForecast")
            .field("host", &self.source.host().host_id)
            .field("boot", &self.source.host().boot_id)
            .field("capability", &self.source.offer().capability_id)
            .field("sign", &self.source.sign())
            .field("calibration", &self.source.calibration())
            .field("demand", &self.demand)
            .field("lower_seconds", &self.lower_seconds())
            .field("upper_seconds", &self.upper_seconds())
            .field("calibrated_estimate", &self.calibration)
            .finish()
    }
}
impl<'a> CostedForecast<'a> {
    pub fn bind(
        source: &'a CostEvidenceSource<'a>,
        demand: CostDemand<'a>,
        forecast: &'a FulfillmentCostForecast,
    ) -> Result<Self, Refusal> {
        demand.class.validate_source(source)?;
        Ok(Self {
            source,
            demand,
            forecast,
            calibration: None,
        })
    }
    pub fn source(&self) -> &'a CostEvidenceSource<'a> {
        self.source
    }
    pub const fn demand(&self) -> CostDemand<'a> {
        self.demand
    }
    pub const fn forecast(&self) -> &'a FulfillmentCostForecast {
        self.forecast
    }
    /// Preserve the original stage account separately from the calibrated mean
    /// estimate. The extra source is revalidated again at actual selection.
    pub fn with_calibration(
        mut self,
        estimate: &'a super::CostCalibrationEstimate<'a>,
        condition: super::CostExecutionCondition,
        now: &conduit_core::MonotonicInstant,
    ) -> Result<Self, Refusal> {
        estimate.validate_use(self.source, self.demand, condition, now)?;
        self.calibration = Some(estimate);
        Ok(self)
    }
    pub fn calibration(&self) -> Option<&'a super::CostCalibrationEstimate<'a>> {
        self.calibration
    }
    pub fn lower_seconds(&self) -> conduit_core::ExactRational {
        self.calibration.map_or_else(
            || self.forecast.lower_seconds(),
            |estimate| estimate.duration().lower().seconds(),
        )
    }
    pub fn upper_seconds(&self) -> conduit_core::ExactRational {
        self.calibration.map_or_else(
            || self.forecast.upper_seconds(),
            |estimate| estimate.duration().upper().seconds(),
        )
    }
    pub fn validate_at(
        &self,
        host: &conduit_core::HostAdvertisement,
        revision: super::CostCalibrationRevision<'_>,
        now: &conduit_core::MonotonicInstant,
    ) -> Result<(), Refusal> {
        self.source.validate_at(host, revision, now)?;
        if let Some(estimate) = self.calibration {
            estimate.source().validate_at(host, revision, now)?;
        }
        Ok(())
    }
}
