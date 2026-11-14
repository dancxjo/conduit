//! Pure bounded chart construction over exact opaque tuple chains; outside Step.
use conduit_core::*;
use conduit_data::*;
pub use conduit_std_offers::{
    measurement_chart_constructor_std_offer, measurement_chart_constructor_workspace,
    MEASUREMENT_CHART_CONSTRUCTOR_CALL, MEASUREMENT_CHART_CONSTRUCTOR_IMPLEMENTATION,
    MEASUREMENT_CHART_CONSTRUCTOR_PROFILE,
};
pub struct PreparedMeasurementChartConstructor {
    schemas: Vec<Vec<u8>>,
    profiles: Vec<KindId>,
    bounds: Vec<u32>,
    labels: Vec<String>,
    output_type: Vec<u8>,
    output: Vec<u8>,
    length: usize,
    workspace: u32,
}
impl PreparedMeasurementChartConstructor {
    pub fn prepare(placement: &PlannedGear) -> Result<Self, String> {
        let count = measurement_chart_constructor_arity(placement.kind_id.as_str())
            .ok_or("unknown exact chart constructor")?;
        let offer = measurement_chart_constructor_std_offer(count)?;
        if placement.kind_contract_revision != offer.kind_contract_revision
            || placement.semantic_contract != offer.semantic_contract
            || placement.inputs != offer.inputs
            || placement.outputs != offer.outputs
            || placement.limits != offer.limits
            || placement.host_calls != offer.host_calls
            || placement.capability_id != offer.capability_id
            || placement.execution_profile_id != offer.implementation.execution_profile_id
            || placement.implementation_id != offer.implementation.implementation_id
            || placement.artifact_id != offer.implementation.artifact_id
        {
            return Err("chart constructor differs from installed realization".into());
        }
        let labels = measurement_chart_constructor_labels(&placement.configuration, count)?;
        let mut schemas = Vec::with_capacity(count);
        let mut profiles = Vec::with_capacity(count);
        let mut bounds = Vec::with_capacity(count);
        for stage in 1..=count {
            let (ty, bound) = measurement_chart_constructor_input(stage)?;
            profiles.push(
                ty.profile()
                    .map_err(|_| "chart input profile")?
                    .value_kind()
                    .clone(),
            );
            schemas.push(ty.canonical_bytes().map_err(|_| "chart input Type")?);
            bounds.push(bound);
        }
        Ok(Self {
            schemas,
            profiles,
            bounds,
            labels,
            output_type: measurement_chart_type()
                .canonical_bytes()
                .map_err(|_| "chart output Type")?,
            output: vec![0; measurement_chart_canonical_bytes()],
            length: 0,
            workspace: measurement_chart_constructor_workspace(count)?,
        })
    }
    pub fn workspace_bytes(&self) -> u32 {
        self.workspace
    }
    pub fn output_bound(&self) -> u32 {
        self.output.len() as u32
    }
    pub fn input_bound(&self) -> u32 {
        *self.bounds.last().expect("nonempty checked shape")
    }
    pub fn retained_backing_bytes(&self) -> usize {
        self.schemas.capacity() * core::mem::size_of::<Vec<u8>>()
            + self.schemas.iter().map(Vec::capacity).sum::<usize>()
            + self.profiles.capacity() * core::mem::size_of::<KindId>()
            + self
                .profiles
                .iter()
                .map(|id| id.0.capacity())
                .sum::<usize>()
            + self.bounds.capacity() * core::mem::size_of::<u32>()
            + self.labels.capacity() * core::mem::size_of::<String>()
            + self.labels.iter().map(String::capacity).sum::<usize>()
            + self.output_type.capacity()
            + self.output.capacity()
    }
    pub fn execute(&mut self, input: &[u8]) -> Result<&[u8], conduit_kernel::Failure> {
        let failure = || conduit_kernel::Failure {
            code: conduit_kernel::FailureCode::InvalidInput,
            detail: 236,
        };
        let mut series = Vec::with_capacity(self.labels.len());
        self.decode_stage(input, self.labels.len(), &mut series)
            .map_err(|_| failure())?;
        let chart = MeasurementChart::new(series).map_err(|_| failure())?;
        let raw = encode_measurement_chart(&chart).map_err(|_| failure())?;
        let writer = PreparedLinkedStructuredComposer::new(&self.output_type, self.output.len())
            .map_err(|_| failure())?;
        self.length = writer
            .encode_into(BorrowedStructuredValue::Leaf(&raw), &mut self.output)
            .map_err(|_| failure())?
            .len();
        Ok(&self.output[..self.length])
    }
    fn decode_stage(
        &self,
        bytes: &[u8],
        stage: usize,
        series: &mut Vec<LabelledMeasurementSeries>,
    ) -> Result<(), ()> {
        let validator = BorrowedStructuredValueValidator::new(
            &self.schemas[stage - 1],
            self.bounds[stage - 1] as usize,
        )
        .map_err(|_| ())?;
        let value = validator.validate(bytes).map_err(|_| ())?;
        if stage == 1 {
            let raw = value
                .primitive_bytes(MEASUREMENT_PLOT_SERIES_INFO_ID)
                .map_err(|_| ())?;
            let decoded = decode_measurement_plot_series(raw).map_err(|_| ())?;
            series.push(
                LabelledMeasurementSeries::new(self.labels[series.len()].clone(), decoded)
                    .map_err(|_| ())?,
            );
        } else {
            let left = value
                .record_field("item-00000")
                .map_err(|_| ())?
                .ok_or(())?
                .primitive_bytes(self.profiles[stage - 2].as_str())
                .map_err(|_| ())?;
            let right = value
                .record_field("item-00001")
                .map_err(|_| ())?
                .ok_or(())?
                .primitive_bytes(self.profiles[0].as_str())
                .map_err(|_| ())?;
            self.decode_stage(left, stage - 1, series)?;
            self.decode_stage(right, 1, series)?;
        }
        Ok(())
    }
}
#[cfg(test)]
mod tests;
