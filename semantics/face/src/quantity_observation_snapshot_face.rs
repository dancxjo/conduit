//! One admitted contribution preserves two independently reported channel contexts.
use crate::*;
use conduit_data::PreparedQuantityObservationSnapshotInput;
use core::mem::size_of;

pub struct PreparedQuantityObservationSnapshotFace {
    input: PreparedQuantityObservationSnapshotInput,
}
impl PreparedQuantityObservationSnapshotFace {
    pub fn prepare() -> Result<Self, QuantityObservationFaceRefusal> {
        Ok(Self {
            input: PreparedQuantityObservationSnapshotInput::prepare()
                .map_err(|_| QuantityObservationFaceRefusal::Preparation)?,
        })
    }
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.input.retained_backing_bytes()
    }
    pub fn projection_workspace_bytes(&self) -> Option<usize> {
        // Original batch projection, aggregate vectors, then a retained clone
        // made by the existing contribution gate can coexist during preparation.
        self.input
            .projection_workspace_bytes()?
            .checked_add(3 * quantity_observation_snapshot_fragment_backing_bound())
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
        let snapshot = self.input.decode(bytes).map_err(R::Input)?;
        let samples: usize = snapshot.batches.iter().map(|b| b.observations.len()).sum();
        super::quantity_observation_face::aggregate_fragments(
            basis.clone(),
            snapshot.batches.iter().map(|batch| {
                Ok((
                    super::quantity_observation_face::project_batch(basis.clone(), batch, label)?,
                    batch.source_identity.as_str(),
                    batch.channel_identity.as_str(),
                    batch.channel_name.as_str(),
                ))
            }),
            samples,
            0,
            label,
            quantity_observation_snapshot_fragment_backing_bound(),
        )
    }
}
pub fn quantity_observation_snapshot_fragment_backing_bound() -> usize {
    // Aggregate vectors hold both builders plus one inspection label per batch.
    2 * quantity_observation_fragment_backing_bound() - size_of::<FaceFragment>()
        + 2 * (size_of::<FaceProperty>() + 128 + "inspection-label".len() + 128)
}
