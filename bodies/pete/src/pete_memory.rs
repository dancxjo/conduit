//! Finite autobiographical experience selection without a Pete-private runtime.

use conduit_ai::{
    ClockBasis, TemporalEvidenceBatch, TemporalEvidenceCandidate, TemporalEvidenceSelection,
    TemporalEvidenceSelectionRefusal, TemporalProvenance, TemporalReference,
    TemporalRetrievalIntent, TemporalSource, TemporalValidity,
};
use conduit_core::{
    kind_id, port_id, CapabilityLimits, KindId, KindIdentity, PortDescriptor, PortDirection,
    PortTemporal,
};
use conduit_form::{KindProjection, KindSignature, ProfileCatalog, StartupCatalog};
use serde::{Deserialize, Serialize};

pub const PETE_MEMORY_RETAIN_KIND: &str = "pete/memory-retain";
pub const PETE_MEMORY_REVISION: &str = "conduit.pete/memory-retain@1";
pub const PETE_EXPERIENCE_CANDIDATE_KIND: &str = "pete/experience-candidate@1";
pub const PETE_RETAINED_EXPERIENCE_KIND: &str = "pete/retained-experience@1";
pub const MAXIMUM_EXPERIENCE_BYTES: usize = 8_192;
pub const MAXIMUM_SOURCE_REFS: usize = 8;
pub const MAXIMUM_RETAINED_EXPERIENCES: usize = 128;

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum ExperienceKind {
    ObservedSign,
    HumanStatement,
    ModelDerived,
    ActionRequest,
    EffectResult,
    BodyBiography,
    OperatorCorrection,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub enum Sensitivity {
    LocalPrivate,
    LocalRedacted,
    Shareable,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExperienceProvenance {
    pub source_identity: String,
    pub event_at_millis: u64,
    pub recorded_at_millis: u64,
    pub body_identity: String,
    pub host_identity: Option<String>,
    pub boot_identity: Option<String>,
    pub plan_identity: Option<String>,
    pub play_identity: Option<String>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct ExperienceCandidate {
    pub identity: String,
    pub kind: ExperienceKind,
    pub content: Vec<u8>,
    pub provenance: Vec<ExperienceProvenance>,
    pub sensitivity: Sensitivity,
    pub supersedes: Option<String>,
    pub explicit_remember: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize, Deserialize)]
pub struct RetainedExperience {
    pub candidate: ExperienceCandidate,
    pub sequence: u64,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryRefusal {
    NotSelected,
    EmptyIdentity,
    EmptyContent,
    TooLarge,
    TooManySources,
    InvalidSource,
    InvalidTime,
    InvalidCorrection,
    CapacityPressure,
    ProviderUnavailable,
    AuthorityRefused,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum MemoryQueryRefusal {
    ProviderUnavailable,
    AuthorityRefused,
    Temporal(TemporalEvidenceSelectionRefusal),
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PeteMemoryContract {
    pub kind_id: KindId,
    pub revision: KindIdentity,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub limits: CapabilityLimits,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct BoundedAutobiography {
    records: Vec<RetainedExperience>,
    capacity: usize,
    provider_available: bool,
}

impl BoundedAutobiography {
    pub fn new(capacity: usize) -> Result<Self, MemoryRefusal> {
        if capacity == 0 || capacity > MAXIMUM_RETAINED_EXPERIENCES {
            return Err(MemoryRefusal::CapacityPressure);
        }
        Ok(Self {
            records: Vec::with_capacity(capacity),
            capacity,
            provider_available: true,
        })
    }
    pub fn set_provider_available(&mut self, available: bool) {
        self.provider_available = available;
    }
    pub fn retain(
        &mut self,
        candidate: ExperienceCandidate,
        write_authorized: bool,
    ) -> Result<&RetainedExperience, MemoryRefusal> {
        if !self.provider_available {
            return Err(MemoryRefusal::ProviderUnavailable);
        }
        if !write_authorized {
            return Err(MemoryRefusal::AuthorityRefused);
        }
        validate(&candidate)?;
        if !candidate.explicit_remember
            && !matches!(
                candidate.kind,
                ExperienceKind::OperatorCorrection
                    | ExperienceKind::EffectResult
                    | ExperienceKind::BodyBiography
            )
        {
            return Err(MemoryRefusal::NotSelected);
        }
        if self.records.len() == self.capacity {
            return Err(MemoryRefusal::CapacityPressure);
        }
        if self
            .records
            .iter()
            .any(|record| record.candidate.identity == candidate.identity)
        {
            return Err(MemoryRefusal::InvalidSource);
        }
        if let Some(original) = &candidate.supersedes {
            if !matches!(candidate.kind, ExperienceKind::OperatorCorrection)
                || !self
                    .records
                    .iter()
                    .any(|record| &record.candidate.identity == original)
            {
                return Err(MemoryRefusal::InvalidCorrection);
            }
        }
        let sequence = self.records.len() as u64;
        self.records.push(RetainedExperience {
            candidate,
            sequence,
        });
        Ok(self.records.last().unwrap())
    }
    pub fn records(&self, read_authorized: bool) -> Result<&[RetainedExperience], MemoryRefusal> {
        if !self.provider_available {
            Err(MemoryRefusal::ProviderUnavailable)
        } else if !read_authorized {
            Err(MemoryRefusal::AuthorityRefused)
        } else {
            Ok(&self.records)
        }
    }
    pub fn forget(
        &mut self,
        identity: &str,
        delete_authorized: bool,
    ) -> Result<RetainedExperience, MemoryRefusal> {
        if !self.provider_available {
            return Err(MemoryRefusal::ProviderUnavailable);
        }
        if !delete_authorized {
            return Err(MemoryRefusal::AuthorityRefused);
        }
        let index = self
            .records
            .iter()
            .position(|record| record.candidate.identity == identity)
            .ok_or(MemoryRefusal::InvalidSource)?;
        Ok(self.records.remove(index))
    }

    pub fn select_temporal(
        &self,
        reference_at_millis: u64,
        intent: &TemporalRetrievalIntent,
        read_authorized: bool,
    ) -> Result<TemporalEvidenceSelection, MemoryQueryRefusal> {
        let records = self
            .records(read_authorized)
            .map_err(|refusal| match refusal {
                MemoryRefusal::ProviderUnavailable => MemoryQueryRefusal::ProviderUnavailable,
                _ => MemoryQueryRefusal::AuthorityRefused,
            })?;
        let candidates = records
            .iter()
            .map(|record| TemporalEvidenceCandidate {
                identity: record.candidate.identity.clone(),
                provenance: TemporalProvenance {
                    event_at: Some(record.candidate.provenance[0].event_at_millis),
                    valid_from: Some(record.candidate.provenance[0].event_at_millis),
                    valid_until: None,
                    observed_at: None,
                    recorded_at: Some(record.candidate.provenance[0].recorded_at_millis),
                    ingested_at: Some(record.candidate.provenance[0].recorded_at_millis),
                    retrieved_at: reference_at_millis,
                    reference_at: reference_at_millis,
                    clock_basis: ClockBasis::UnixEpochMilliseconds,
                    uncertainty_millis: None,
                },
                source: TemporalSource::Event,
                boundary: None,
                transition: None,
                validity: if records.iter().any(|later| {
                    later.candidate.supersedes.as_deref()
                        == Some(record.candidate.identity.as_str())
                }) {
                    TemporalValidity::Superseded
                } else {
                    TemporalValidity::Historical
                },
            })
            .collect();
        TemporalEvidenceBatch {
            reference: TemporalReference {
                reference_at: reference_at_millis,
                clock_basis: ClockBasis::UnixEpochMilliseconds,
            },
            candidates,
            earliest_history_complete: true,
        }
        .select(intent)
        .map_err(MemoryQueryRefusal::Temporal)
    }
}

fn validate(candidate: &ExperienceCandidate) -> Result<(), MemoryRefusal> {
    if candidate.identity.is_empty() {
        return Err(MemoryRefusal::EmptyIdentity);
    }
    if candidate.content.is_empty() {
        return Err(MemoryRefusal::EmptyContent);
    }
    if candidate.content.len() > MAXIMUM_EXPERIENCE_BYTES {
        return Err(MemoryRefusal::TooLarge);
    }
    if candidate.provenance.is_empty() || candidate.provenance.len() > MAXIMUM_SOURCE_REFS {
        return Err(MemoryRefusal::TooManySources);
    }
    for source in &candidate.provenance {
        if source.source_identity.is_empty() || source.body_identity.is_empty() {
            return Err(MemoryRefusal::InvalidSource);
        }
        if source.event_at_millis > source.recorded_at_millis {
            return Err(MemoryRefusal::InvalidTime);
        }
    }
    Ok(())
}

pub fn pete_memory_contract() -> PeteMemoryContract {
    let port = |name, kind, direction| PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal: PortTemporal::Value,
    };
    PeteMemoryContract {
        kind_id: kind_id(PETE_MEMORY_RETAIN_KIND),
        revision: KindIdentity::from(PETE_MEMORY_REVISION),
        inputs: vec![port(
            "candidate",
            PETE_EXPERIENCE_CANDIDATE_KIND,
            PortDirection::Input,
        )],
        outputs: vec![port(
            "retained",
            PETE_RETAINED_EXPERIENCE_KIND,
            PortDirection::Output,
        )],
        limits: CapabilityLimits {
            max_active_instances: 1,
            max_queue_items: MAXIMUM_RETAINED_EXPERIENCES as u16,
            max_queue_bytes: MAXIMUM_EXPERIENCE_BYTES as u32,
        },
    }
}

pub fn install_pete_memory_catalog(
    startup: &mut StartupCatalog,
    profile: &mut ProfileCatalog,
) -> Result<(), String> {
    startup.insert_value_kind_alias(
        "ExperienceCandidate",
        kind_id(PETE_EXPERIENCE_CANDIDATE_KIND),
    )?;
    startup
        .insert_value_kind_alias("RetainedExperience", kind_id(PETE_RETAINED_EXPERIENCE_KIND))?;
    startup.insert(KindSignature {
        kind: PETE_MEMORY_RETAIN_KIND.into(),
        startup_parameters: vec![],
    })?;
    let contract = pete_memory_contract();
    profile
        .insert(KindProjection {
            kind_id: contract.kind_id,
            kind_contract_revision: contract.revision,
            inputs: contract.inputs,
            outputs: contract.outputs,
            configuration: vec![],
        })
        .map_err(|error| error.to_string())
}

#[cfg(test)]
#[path = "pete_memory_tests.rs"]
mod tests;
