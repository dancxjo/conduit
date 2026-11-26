//! Reported finite quantity observations become an inspection contribution.
//! Input provenance is data, never membership, a recording commit or authority.
use crate::*;
use alloc::{string::String, vec::Vec};
use conduit_data::{
    PreparedQuantityObservationInput, QuantityObservationOutcome,
    MAX_RESOLVED_QUANTITY_OBSERVATIONS,
};
use core::{fmt::Write, mem::size_of};

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
    use QuantityObservationFaceRefusal as R;
    let source = batch.source_identity.as_str();
    let channel = batch.channel_identity.as_str();
    let n = batch.observations.len();
    let mut fragment = FaceFragment {
        basis,
        subjects: Vec::with_capacity(n + 2),
        relationships: Vec::with_capacity(2 * n),
        properties: Vec::with_capacity(11 + 7 * n),
        composition: Vec::new(),
        text: Vec::new(),
        actions: Vec::new(),
        disclosures: Vec::new(),
        temporal_references: Vec::new(),
        temporal_facts: Vec::new(),
    };
    fragment.subjects.push(FaceSubject {
        identity: source.into(),
        name: batch.source_name.clone(),
        role: FaceRole::Collection,
    });
    fragment.subjects.push(FaceSubject {
        identity: channel.into(),
        name: label.into(),
        role: FaceRole::Semantic(QUANTITY_SERIES_ROLE.into()),
    });
    let mut fact = |subject: &str, name: &str, value| {
        fragment.properties.push(FaceProperty {
            subject: subject.into(),
            name: name.into(),
            value,
        })
    };
    use FacePropertyValue as V;
    fact(source, "clock", V::Text("producer-boot-ms".into()));
    for (name, value) in [
        ("producer-host", &batch.producer_host),
        ("producer-boot", &batch.producer_boot),
        ("producer-plan", &batch.producer_plan),
        ("producer-play", &batch.producer_play),
    ] {
        fact(source, name, V::Identity(value.clone()));
    }
    fact(source, "visible-samples", V::Count(n as u64));
    fact(
        source,
        "samples-outside-view",
        V::Count(batch.samples_outside_view),
    );
    fact(
        source,
        "observation-origin",
        V::Text("reported-input".into()),
    );
    fact(channel, "series-source", V::Identity(source.into()));
    fact(channel, "series-value-property", V::Text("value".into()));
    fact(
        channel,
        "reported-channel-name",
        V::Text(batch.channel_name.clone()),
    );
    for observation in batch.observations.as_slice() {
        let mut identity = String::with_capacity(156);
        write!(&mut identity, "{source}/sample/{}", observation.sequence)
            .map_err(|_| R::Fragment)?;
        let mut name = String::with_capacity(32);
        write!(&mut name, "Observation {}", observation.sequence).map_err(|_| R::Fragment)?;
        fragment.subjects.push(FaceSubject {
            identity: identity.clone(),
            name,
            role: FaceRole::Item,
        });
        for parent in [source, channel] {
            fragment.relationships.push(FaceRelationship {
                source: parent.into(),
                target: identity.clone(),
                kind: FaceRelationshipKind::Contains,
            });
        }
        fact(&identity, "series-source", V::Identity(source.into()));
        fact(&identity, "sequence", V::Count(observation.sequence));
        fact(
            &identity,
            "sampled-at-boot-ms",
            V::Count(observation.sampled_at_boot_ms),
        );
        match &observation.outcome {
            QuantityObservationOutcome::Available(value) => {
                fact(&identity, "series-available", V::Flag(true));
                fact(
                    &identity,
                    "value",
                    V::TypedValue {
                        contract: conduit_core::CheckedValueContract::new(
                            conduit_core::QUANTITY_INFO_ID.into(),
                            conduit_core::QUANTITY_ENCODED_LEN as u32,
                            Vec::new(),
                        )
                        .map_err(|_| R::Fragment)?,
                        bytes: value.encode().to_vec(),
                    },
                );
            }
            QuantityObservationOutcome::Unavailable(reason) => {
                fact(&identity, "series-available", V::Flag(false));
                fact(&identity, "series-unavailability", V::Text(reason.clone()));
            }
        }
        if let Some(gap) = &observation.gap_before {
            fact(
                &identity,
                "missing-sequence-first",
                V::Count(gap.first_sequence),
            );
            fact(
                &identity,
                "missing-sequence-last",
                V::Count(gap.last_sequence),
            );
        }
    }
    fragment.validate_bounds().map_err(|_| R::Fragment)?;
    if inspection_fragment_backing_bytes(&fragment).ok_or(R::Backing)?
        > quantity_observation_fragment_backing_bound()
    {
        return Err(R::Backing);
    }
    Ok(fragment)
}

/// The builder reserves these exact vector capacities. Per-property payloads
/// cover a full Quantity, empty checked contract, or bounded provenance text.
pub fn quantity_observation_fragment_backing_bound() -> usize {
    let n = MAX_RESOLVED_QUANTITY_OBSERVATIONS;
    size_of::<FaceFragment>()
        + 3 * MAX_FACE_ID_BYTES
        + (n + 2) * (size_of::<FaceSubject>() + 156 + 128 + QUANTITY_SERIES_ROLE.len())
        + (2 * n) * (size_of::<FaceRelationship>() + 2 * 156)
        + (11 + 7 * n)
            * (size_of::<FaceProperty>() + 156 + 32 + conduit_core::QUANTITY_ENCODED_LEN + 1024)
}
