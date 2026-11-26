//! One admitted contribution preserves two independently reported channel contexts.
use crate::*;
use alloc::vec::Vec;
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
        let count = snapshot.batches.len();
        let mut aggregate = FaceFragment {
            basis: basis.clone(),
            subjects: Vec::with_capacity(samples + 2 * count),
            relationships: Vec::with_capacity(2 * samples),
            properties: Vec::with_capacity(12 * count + 7 * samples),
            composition: Vec::new(),
            text: Vec::new(),
            actions: Vec::new(),
            disclosures: Vec::new(),
            temporal_references: Vec::new(),
            temporal_facts: Vec::new(),
        };
        for batch in snapshot.batches.iter() {
            let mut fragment =
                super::quantity_observation_face::project_batch(basis.clone(), batch, label)?;
            // Preserve distinct reported channel captions in the ordinary graph;
            // retain the authored inspection label separately as inert metadata.
            fragment
                .subjects
                .iter_mut()
                .find(|subject| subject.identity == batch.channel_identity)
                .ok_or(R::Fragment)?
                .name = batch.channel_name.clone();
            aggregate.properties.push(FaceProperty {
                subject: batch.source_identity.clone(),
                name: "inspection-label".into(),
                value: FacePropertyValue::Text(label.into()),
            });
            aggregate.subjects.extend(fragment.subjects);
            aggregate.relationships.extend(fragment.relationships);
            aggregate.properties.extend(fragment.properties);
        }
        aggregate.validate_bounds().map_err(|_| R::Fragment)?;
        if inspection_fragment_backing_bytes(&aggregate).ok_or(R::Backing)?
            > quantity_observation_snapshot_fragment_backing_bound()
        {
            return Err(R::Backing);
        }
        Ok(aggregate)
    }
}
pub fn quantity_observation_snapshot_fragment_backing_bound() -> usize {
    // Aggregate vectors hold both builders plus one inspection label per batch.
    2 * quantity_observation_fragment_backing_bound() - size_of::<FaceFragment>()
        + 2 * (size_of::<FaceProperty>() + 128 + "inspection-label".len() + 128)
}
