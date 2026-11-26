//! Reported finite quantity observations become an inspection contribution.
//! Input provenance is data, never membership, a recording commit or authority.
use crate::*;
use conduit_data::{
    PreparedQuantityObservationInput, QuantityObservationOutcome,
    MAX_RESOLVED_QUANTITY_OBSERVATIONS,
};
mod builder;
mod model;
pub(super) use builder::aggregate_fragments;
pub use builder::{project_reported_quantity_channel, project_reported_quantity_channels};
pub use model::{
    reported_quantity_fragment_backing_bound, reported_quantity_projection_workspace_bytes,
    ReportedQuantityChannel, ReportedQuantityObservation, ReportedQuantityOutcome,
    ReportedQuantityRecovery, MAX_REPORTED_QUANTITY_OBSERVATIONS,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum QuantityObservationFaceRefusal {
    Preparation,
    Input(conduit_data::QuantityObservationInputRefusal),
    Basis,
    Label,
    Fragment,
    Backing,
}

pub struct PreparedQuantityObservationFace {
    input: PreparedQuantityObservationInput,
}
impl PreparedQuantityObservationFace {
    pub fn prepare() -> Result<Self, QuantityObservationFaceRefusal> {
        Ok(Self {
            input: PreparedQuantityObservationInput::prepare()
                .map_err(|_| QuantityObservationFaceRefusal::Preparation)?,
        })
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.input.retained_backing_bytes()
    }
    pub fn projection_workspace_bytes(&self) -> Option<usize> {
        self.input
            .projection_workspace_bytes()?
            .checked_add(2 * quantity_observation_fragment_backing_bound())
    }
    pub fn canonical_observation(
        &self,
        basis: FaceContributionBasis,
        bytes: &[u8],
        label: &str,
    ) -> Result<FaceFragment, QuantityObservationFaceRefusal> {
        use QuantityObservationFaceRefusal as R;
        if basis.required_interaction_context.is_some()
            || [
                basis.checked_plot_id.as_str(),
                basis.plan_id.as_str(),
                basis.active_play_id.as_str(),
            ]
            .iter()
            .any(|id| id.is_empty() || id.len() > MAX_FACE_ID_BYTES)
        {
            return Err(R::Basis);
        }
        if !conduit_core::valid_data_label(label) {
            return Err(R::Label);
        }
        let batch = self.input.decode(bytes).map_err(R::Input)?;
        project_batch(basis, &batch, label)
    }
}

pub(super) fn project_batch(
    basis: FaceContributionBasis,
    batch: &conduit_data::ResolvedQuantityObservationBatch,
    label: &str,
) -> Result<FaceFragment, QuantityObservationFaceRefusal> {
    let channel = ReportedQuantityChannel {
        source_identity: &batch.source_identity,
        source_name: &batch.source_name,
        channel_identity: &batch.channel_identity,
        channel_name: &batch.channel_name,
        producer: [
            &batch.producer_host,
            &batch.producer_boot,
            &batch.producer_plan,
            &batch.producer_play,
        ],
        samples_outside_view: batch.samples_outside_view,
        recovery: None,
    };
    builder::project_iter(
        basis,
        channel,
        batch
            .observations
            .iter()
            .map(|sample| ReportedQuantityObservation {
                sequence: sample.sequence,
                sampled_at_boot_ms: sample.sampled_at_boot_ms,
                outcome: match &sample.outcome {
                    QuantityObservationOutcome::Available(value) => {
                        ReportedQuantityOutcome::Available(value.as_ref())
                    }
                    QuantityObservationOutcome::Unavailable(reason) => {
                        ReportedQuantityOutcome::Unavailable(reason)
                    }
                },
                gap_before: sample
                    .gap_before
                    .as_ref()
                    .map(|gap| (gap.first_sequence, gap.last_sequence)),
            }),
        label,
        MAX_RESOLVED_QUANTITY_OBSERVATIONS,
    )
}

/// Existing Data16 projection retains its independent admission/model ceiling.
pub fn quantity_observation_fragment_backing_bound() -> usize {
    reported_quantity_fragment_backing_bound(MAX_RESOLVED_QUANTITY_OBSERVATIONS, false)
        .expect("fixed Data16 profile")
}

#[cfg(test)]
mod tests;
