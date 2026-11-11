//! Std realization of the existing portable measurement projection contract.
//!
//! The shared kernel's HostCallBack owns request/completion/pressure lifecycle.
//! Decoding and projection run in the explicitly bounded Host Call provider,
//! not inside an allocation-sealed kernel Step. Its existing semantic codecs
//! allocate bounded intermediates: this module does not claim zero allocation,
//! actual Root memory custody, history persistence, a Face or physical output.
use conduit_core::{
    ArtifactId, Back, BackOfferBuilder, CapabilityId, CapabilityOffer, ConfigurationValue,
    ExecutionProfileId, HostCallRequirement, ImplementationId, PlannedGear,
};
use conduit_data::{MeasurementPlotOverflowPolicy, MeasurementPlotProfile, MeasurementPlotSeries};
use conduit_kernel::{scheduler::HostCallBack, Failure, FailureCode};

pub const MEASUREMENT_PLOT_IMPLEMENTATION: &str = "std/measurement-plot@1";
pub const MEASUREMENT_PLOT_PROFILE: &str = "std/measurement-plot-hosted@1";
pub const MEASUREMENT_PLOT_HOST_CALL: &str = "conduit.host/measurement-plot@1";

/// Canonical structured values include the exact intrinsic leaf contract/header.
/// These bounds are derived from that contract, not a renderer's packet estimate.
pub fn measurement_plot_value_bounds() -> (u32, u32) {
    let bound = |value_type: conduit_core::StructuredInfoType, payload: usize| {
        u32::try_from(
            value_type
                .canonical_bytes()
                .expect("reviewed measurement type")
                .len()
                + 5
                + payload,
        )
        .expect("finite measurement value bound")
    };
    (
        bound(
            conduit_data::measurement_window_type(),
            conduit_data::MAXIMUM_MEASUREMENT_WINDOW_BYTES,
        ),
        bound(
            conduit_data::measurement_plot_series_type(),
            conduit_data::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES,
        ),
    )
}

/// Conservative requested Rust backing, excluding allocator overhead and stacks.
/// The installation owner must separately admit this requirement and bind the
/// provider to its actual Host Call; this number is not an ownership witness.
pub fn measurement_plot_workspace_bytes() -> u32 {
    let (input, output) = measurement_plot_value_bounds();
    let decoded = core::mem::size_of::<conduit_data::BoundedMeasurementWindow>()
        // The decoder and validated restored window can coexist. All decoded
        // clock text together is bounded by the incoming wire payload.
        + 2 * conduit_data::MAXIMUM_MEASUREMENT_WINDOW_SAMPLES
            * core::mem::size_of::<conduit_data::MeasurementSample>()
        + conduit_data::MAXIMUM_MEASUREMENT_WINDOW_BYTES;
    let projected = 2
        * conduit_data::MAXIMUM_MEASUREMENT_PLOT_POINTS
        * core::mem::size_of::<conduit_data::MeasurementPlotPoint>()
        + core::mem::size_of::<MeasurementPlotSeries>();
    input + output * 2 + u32::try_from(decoded + projected).expect("finite projection workspace")
}

pub fn measurement_plot_std_offer() -> CapabilityOffer {
    let contract = conduit_data::measurement_plot_semantic_contract();
    let kind = contract.kind_id.clone();
    let digest = conduit_core::semantic_digest(
        "std/measurement-plot-provider@1",
        concat!(
            include_str!("hosted_measurement.rs"),
            include_str!("../../../semantics/data/src/measurement_plot.rs"),
            include_str!("../../../semantics/data/src/measurement_wire.rs"),
            include_str!("../../../semantics/data/src/measurement_window.rs"),
            include_str!("../../../semantics/data/types.conduit"),
        )
        .as_bytes(),
    );
    let artifact = format!(
        "sha256:{}",
        digest
            .iter()
            .map(|byte| format!("{byte:02x}"))
            .collect::<String>()
    );
    let (input, output) = measurement_plot_value_bounds();
    BackOfferBuilder::new(
        contract,
        Back {
            capability_id: CapabilityId::from(MEASUREMENT_PLOT_IMPLEMENTATION),
            execution_profile_id: ExecutionProfileId::from(MEASUREMENT_PLOT_PROFILE),
            implementation_id: ImplementationId::from(MEASUREMENT_PLOT_IMPLEMENTATION),
            artifact_id: ArtifactId::from(artifact),
            host_calls: vec![HostCallRequirement {
                contract_id: MEASUREMENT_PLOT_HOST_CALL.into(),
                target_kind: Some(kind),
                maximum_in_flight: 1,
                maximum_input_bytes: input,
                maximum_output_bytes: output,
            }],
            resource_requirements: vec![conduit_core::resource_requirement(
                conduit_core::RUNTIME_MEMORY_RESOURCE_CLASS,
                measurement_plot_workspace_bytes(),
            )],
            authority_requirements: vec![],
        },
    )
    .build()
}

pub struct PreparedMeasurementPlot {
    profile: MeasurementPlotProfile,
    input_type: Vec<u8>,
    output_type: Vec<u8>,
    output: Vec<u8>,
    input_bound: usize,
    output_bound: usize,
}

impl PreparedMeasurementPlot {
    /// Prepare an inert provider and the original production kernel Back. This
    /// checks the selected implementation/front/configuration, not Host/Boot,
    /// resource grants or execution authority owned by the installation caller.
    pub fn prepare(placement: &PlannedGear) -> Result<(HostCallBack, Self), String> {
        let expected = measurement_plot_std_offer();
        if placement.kind_id != expected.kind_id
            || placement.kind_contract_revision != expected.kind_contract_revision
            || placement.semantic_contract != expected.semantic_contract
            || placement.capability_id != expected.capability_id
            || placement.execution_profile_id != expected.implementation.execution_profile_id
            || placement.implementation_id != expected.implementation.implementation_id
            || placement.artifact_id != expected.implementation.artifact_id
            || placement.inputs != expected.inputs
            || placement.outputs != expected.outputs
            || placement.host_calls != expected.host_calls
            || placement.limits != expected.limits
        {
            return Err("measurement plot placement differs from installed implementation".into());
        }
        let [first, second] = placement.configuration.as_slice() else {
            return Err("measurement plot requires exactly points and when-full".into());
        };
        let get = |name: &str| {
            [first, second]
                .into_iter()
                .find(|entry| entry.key == name)
                .map(|entry| &entry.value)
        };
        let points = match get("points") {
            Some(ConfigurationValue::U64(points))
                if (1..=conduit_data::MAXIMUM_MEASUREMENT_PLOT_POINTS as u64).contains(points) =>
            {
                *points as u16
            }
            _ => return Err("measurement plot points outside finite contract".into()),
        };
        let policy = match get("when-full") {
            Some(ConfigurationValue::Text(policy)) if policy == "reject" => {
                MeasurementPlotOverflowPolicy::Reject
            }
            Some(ConfigurationValue::Text(policy)) if policy == "evenly-spaced" => {
                MeasurementPlotOverflowPolicy::EvenlySpaced
            }
            _ => return Err("measurement plot overflow policy is not declared".into()),
        };
        let (input, output) = measurement_plot_value_bounds();
        Ok((
            HostCallBack::new(input),
            Self {
                profile: MeasurementPlotProfile {
                    point_capacity: points,
                    overflow_policy: policy,
                },
                input_type: conduit_data::measurement_window_type()
                    .canonical_bytes()
                    .map_err(|e| format!("measurement input type: {e:?}"))?,
                output_type: conduit_data::measurement_plot_series_type()
                    .canonical_bytes()
                    .map_err(|e| format!("measurement output type: {e:?}"))?,
                output: Vec::with_capacity(output as usize),
                input_bound: input as usize,
                output_bound: output as usize,
            },
        ))
    }

    pub fn retained_backing_bytes(&self) -> usize {
        self.input_type.capacity() + self.output_type.capacity() + self.output.capacity()
    }

    /// Called only by the installation's admitted Host Call dispatcher. The
    /// semantic decoder checks sample count, quantity ranges, clocks and order;
    /// projection preserves its existing sample-index x-axis meaning. It does
    /// not claim elapsed-time alignment or authenticate an observation source.
    pub fn execute(&mut self, input: &[u8]) -> Result<&[u8], Failure> {
        if input.len() > self.input_bound {
            return Err(failure(1));
        }
        let node = input
            .strip_prefix(self.input_type.as_slice())
            .ok_or(failure(2))?;
        if node.len() < 5 || node[0] != 0 {
            return Err(failure(2));
        }
        let len = u32::from_le_bytes(node[1..5].try_into().map_err(|_| failure(2))?) as usize;
        if len > conduit_data::MAXIMUM_MEASUREMENT_WINDOW_BYTES || node.len() != 5 + len {
            return Err(failure(2));
        }
        let window = conduit_data::decode_measurement_window(&node[5..]).map_err(|_| failure(3))?;
        let series = MeasurementPlotSeries::project(&window, self.profile).map_err(|error| {
            use conduit_data::MeasurementPlotRefusal::*;
            failure(match error {
                InvalidPointCapacity => 4,
                EmptyWindow => 5,
                Full => 6,
                DegenerateValueRange => 7,
                ArithmeticOverflow => 8,
                InvalidProjection => 9,
            })
        })?;
        let payload =
            conduit_data::encode_measurement_plot_series(&series).map_err(|_| failure(10))?;
        let length = self.output_type.len() + 5 + payload.len();
        if payload.len() > conduit_data::MAXIMUM_MEASUREMENT_PLOT_SERIES_BYTES
            || length > self.output_bound
        {
            return Err(failure(11));
        }
        // Do not publish partial output or change the prior output on refusal.
        self.output.clear();
        self.output.extend_from_slice(&self.output_type);
        self.output.push(0);
        self.output
            .extend_from_slice(&(payload.len() as u32).to_le_bytes());
        self.output.extend_from_slice(&payload);
        Ok(&self.output)
    }
}

fn failure(detail: u16) -> Failure {
    Failure {
        code: FailureCode::InvalidInput,
        detail,
    }
}

#[cfg(test)]
#[path = "hosted_measurement/tests.rs"]
mod tests;
