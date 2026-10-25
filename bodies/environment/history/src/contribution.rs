use alloc::{format, string::String, vec, vec::Vec};
use conduit_core::{validate_canonical_structured_value, Quantity};
use conduit_environment_face::{contribution, SensorFailure, SensorStatus};
use conduit_environment_runtime::{
    observation::ObservationContract,
    recording::{
        PreparedRecordingHistory, RecordingContext, RecordingDisposition as D,
        RecordingHistoryRefusal, RecoveredHistory, MAXIMUM_CHUNK_SAMPLES,
    },
};
use conduit_face::{
    FaceContributionBasis, FaceFragment, FaceProperty, FacePropertyValue as V, FaceRelationship,
    FaceRelationshipKind, FaceRole, FaceSubject,
};

pub const HISTORY_SUBJECT: &str = "environment-history";
#[derive(Debug, PartialEq, Eq)]
pub enum HistoryFaceRefusal {
    Recording(RecordingHistoryRefusal),
    Observation,
    Fragment,
    Window,
}

/// Explicit sample budget for allocating Face preparation. This selects a tail
/// of the loaded chunk; it does not trim the recording or promise that a whole
/// composed Face fits any particular Host's separately admitted byte budget.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HistoryFaceWindow {
    maximum_samples: usize,
}
impl HistoryFaceWindow {
    pub fn prepare(maximum_samples: usize) -> Result<Self, HistoryFaceRefusal> {
        if !(1..=MAXIMUM_CHUNK_SAMPLES).contains(&maximum_samples) {
            return Err(HistoryFaceRefusal::Window);
        }
        Ok(Self { maximum_samples })
    }
    pub const fn complete_chunk() -> Self {
        Self {
            maximum_samples: MAXIMUM_CHUNK_SAMPLES,
        }
    }
}

/// One exact producer context and the recorder's existing one-chunk workspace.
/// Recovery admits a maximum chunk count before reading. There is no additional
/// retained measurement collection, source of persistence, or renderer cache.
/// Fragment construction allocates and belongs in separately admitted hosted
/// preparation, not an allocation-sealed kernel Step.
pub struct PreparedHistoryFace {
    history: PreparedRecordingHistory,
    producer: [String; 4],
}
impl PreparedHistoryFace {
    pub fn prepare(
        context: RecordingContext<'_>,
        contract: &ObservationContract,
        maximum_chunks: u16,
    ) -> Result<Self, HistoryFaceRefusal> {
        let identifiers = [
            context.host.as_str(),
            context.boot.as_str(),
            context.plan.as_str(),
            context.play.as_str(),
        ];
        let history = PreparedRecordingHistory::prepare(context, contract, maximum_chunks)
            .map_err(HistoryFaceRefusal::Recording)?;
        Ok(Self {
            history,
            producer: identifiers.map(String::from),
        })
    }
    /// Caller independently authorizes and retrieves the selected immutable
    /// generation. This method validates content; it does not attest durability.
    pub fn read_next(&mut self, json: &str) -> Result<(), HistoryFaceRefusal> {
        self.history
            .read_next(json)
            .map_err(HistoryFaceRefusal::Recording)
    }
    pub fn recovered(&self) -> RecoveredHistory {
        self.history.recovered()
    }

    /// Latest loaded chunk, at most 64 samples. Earlier recovered chunks are
    /// counted as outside this view; no preceding completeness is inferred.
    /// The caller's contribution basis is the inspecting Plot's execution,
    /// while producer Host/Boot/Plan/Play remain distinct semantic properties.
    pub fn contribution(
        &mut self,
        basis: FaceContributionBasis,
    ) -> Result<FaceFragment, HistoryFaceRefusal> {
        self.contribution_window(basis, HistoryFaceWindow::complete_chunk())
    }

    /// Project the selected tail without advancing recovery. Sequence numbers,
    /// producer times, failures and the original boundary gap remain exact.
    pub fn contribution_window(
        &mut self,
        basis: FaceContributionBasis,
        window: HistoryFaceWindow,
    ) -> Result<FaceFragment, HistoryFaceRefusal> {
        let recovered = self.history.recovered();
        let loaded = self.history.loaded_samples();
        let start = loaded.saturating_sub(window.maximum_samples);
        let count = loaded - start;
        let mut fragment = FaceFragment {
            basis: basis.clone(),
            subjects: vec![FaceSubject {
                identity: HISTORY_SUBJECT.into(),
                role: FaceRole::Collection,
                name: "Recorded room conditions".into(),
            }],
            relationships: vec![],
            composition: vec![],
            properties: vec![],
            text: vec![],
            actions: vec![],
            disclosures: vec![],
            temporal_references: vec![],
            temporal_facts: vec![],
        };
        for (name, value) in [
            "producer-host",
            "producer-boot",
            "producer-plan",
            "producer-play",
        ]
        .into_iter()
        .zip(&self.producer)
        {
            push(
                &mut fragment.properties,
                HISTORY_SUBJECT,
                name,
                V::Identity(value.clone()),
            );
        }
        for (name, value) in [
            ("schema", V::Text("environment/history@1".into())),
            ("clock", V::Text("producer-boot-ms".into())),
            ("preceding-history-known", V::Flag(false)),
            ("recovered-chunks", V::Count(u64::from(recovered.chunks))),
            ("recovered-samples", V::Count(u64::from(recovered.samples))),
            ("visible-samples", V::Count(count as u64)),
            (
                "samples-outside-view",
                V::Count(u64::from(recovered.samples) - count as u64),
            ),
        ] {
            push(&mut fragment.properties, HISTORY_SUBJECT, name, value);
        }
        for index in start..loaded {
            let sample = self
                .history
                .sample(index)
                .ok_or(HistoryFaceRefusal::Observation)?;
            let subject = format!("{HISTORY_SUBJECT}/sample/{}", sample.sequence);
            let status = status(sample.disposition);
            let quantities = match sample.observation {
                Some(bytes) => {
                    let observation = validate_canonical_structured_value(bytes)
                        .map_err(|_| HistoryFaceRefusal::Observation)?;
                    let quantity = |name| {
                        let value = observation.record_field(name).ok()??;
                        Quantity::decode(value.value_node().get(5..)?).ok()
                    };
                    Some(
                        quantity("temperature")
                            .zip(quantity("humidity"))
                            .ok_or(HistoryFaceRefusal::Observation)?,
                    )
                }
                None => None,
            };
            let current = contribution(
                quantities.map(|q| q.0),
                quantities.map(|q| q.1),
                false,
                status,
                basis.clone(),
            )
            .map_err(|_| HistoryFaceRefusal::Observation)?;
            fragment.subjects.push(FaceSubject {
                identity: subject.clone(),
                role: FaceRole::Item,
                name: format!("Recorded sample {}", sample.sequence),
            });
            fragment.relationships.push(FaceRelationship {
                source: HISTORY_SUBJECT.into(),
                target: subject.clone(),
                kind: FaceRelationshipKind::Contains,
            });
            for mut property in current.properties {
                property.subject = subject.clone();
                fragment.properties.push(property);
            }
            for (name, value) in [
                ("sequence", sample.sequence),
                ("sampled-at-boot-ms", sample.sampled_at_boot_ms),
            ] {
                push(&mut fragment.properties, &subject, name, V::Count(value));
            }
            if let Some((first, last)) = sample.gap_before {
                push(
                    &mut fragment.properties,
                    &subject,
                    "missing-sequence-first",
                    V::Count(first),
                );
                push(
                    &mut fragment.properties,
                    &subject,
                    "missing-sequence-last",
                    V::Count(last),
                );
            }
        }
        fragment
            .validate_bounds()
            .map_err(|_| HistoryFaceRefusal::Fragment)?;
        Ok(fragment)
    }
}
fn push(properties: &mut Vec<FaceProperty>, subject: &str, name: &str, value: V) {
    properties.push(FaceProperty {
        subject: subject.into(),
        name: name.into(),
        value,
    });
}
fn status(disposition: D) -> SensorStatus {
    SensorStatus::Unavailable(match disposition {
        D::Available => return SensorStatus::Available,
        D::CaptureTimeout => SensorFailure::CaptureTimeout,
        D::ProviderFailed => SensorFailure::ProviderFailed,
        D::GeometryRefused => SensorFailure::GeometryRefused,
        D::ShortFrame => SensorFailure::ShortFrame,
        D::MalformedLength => SensorFailure::MalformedLength,
        D::Checksum => SensorFailure::Checksum,
        D::InvalidPayload => SensorFailure::InvalidPayload,
    })
}
