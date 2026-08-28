//! Capability-granular usefulness without a whole-Host quality judgment.

use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{
    BootId, CapabilityId, HostBaseKindId, HostId, ImplementationId, KindId, OfferGeneration,
    ResourceClassId, ResourcePoolId, SignId,
};
use serde::{Deserialize, Serialize};

pub const MAX_USEFULNESS_ENTRIES: usize = 64;
pub const MAX_USEFULNESS_RESOURCES: usize = 16;
pub const MAX_USEFULNESS_TEXT_LINES: usize = 256;
pub const MAX_USEFULNESS_FIELD_BYTES: usize = 256;
pub const MAX_USEFULNESS_TEXT_BYTES: usize = 64 * 1024;

/// The same prerequisite names used by the std-gap proof, with runtime-only
/// refusal classes kept distinct rather than folded into a generic gap.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityDisposition {
    Offered,
    Unsupported,
    ImplementationMissing,
    ResourceMissing {
        class_id: ResourceClassId,
    },
    BaseMissing {
        kind_id: HostBaseKindId,
    },
    CapacityInsufficient {
        pool_id: ResourcePoolId,
        required_units: u32,
        reservable_units: u32,
    },
    PolicyAuthorityRefusal {
        requirement_id: String,
    },
    LineUnavailable {
        line_id: String,
    },
}

impl CapabilityDisposition {
    pub fn is_useful(&self) -> bool {
        matches!(self, Self::Offered)
    }

    pub fn refusal_class(&self) -> Option<&'static str> {
        match self {
            Self::Offered => None,
            Self::Unsupported => Some("unsupported-on-this-machine"),
            Self::ImplementationMissing => Some("portable-implementation-missing"),
            Self::ResourceMissing { .. } => Some("missing-resource"),
            Self::BaseMissing { .. } => Some("missing-base"),
            Self::CapacityInsufficient { .. } => Some("capacity-insufficient"),
            Self::PolicyAuthorityRefusal { .. } => Some("policy-authority-refusal"),
            Self::LineUnavailable { .. } => Some("line-unavailable"),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResourceCeiling {
    pub pool_id: ResourcePoolId,
    pub class_id: ResourceClassId,
    pub capacity_units: u32,
    pub reservable_units: u32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityUsefulnessEntry {
    /// A capability id or demanded profile id; never a whole-Host rating.
    pub subject_id: String,
    pub label: String,
    pub kind_id: Option<KindId>,
    pub capability_id: Option<CapabilityId>,
    pub implementation_id: Option<ImplementationId>,
    pub disposition: CapabilityDisposition,
    pub resources: Vec<ResourceCeiling>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CapabilityUsefulnessReport {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub offer_generation: OfferGeneration,
    pub observed_at_millis: u64,
    pub expires_at_millis: u64,
    pub observation_sign_id: SignId,
    pub entries: Vec<CapabilityUsefulnessEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CapabilityUsefulnessError {
    EmptyIdentity,
    EmptyEntries,
    TooManyEntries,
    TooManyResources,
    InvalidTimeRange,
    InvalidResourceCeiling,
    InvalidCapacityRelation,
    OfferedWithoutExactImplementation,
    RefusalWithoutSubject,
    PresentationTooLarge,
}

impl core::fmt::Display for CapabilityUsefulnessError {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        write!(formatter, "invalid capability usefulness report: {self:?}")
    }
}

impl core::error::Error for CapabilityUsefulnessError {}

impl CapabilityUsefulnessReport {
    pub fn validate(&self) -> Result<(), CapabilityUsefulnessError> {
        if self.host_id.as_str().is_empty()
            || self.boot_id.as_str().is_empty()
            || self.observation_sign_id.as_str().is_empty()
            || !bounded(self.host_id.as_str())
            || !bounded(self.boot_id.as_str())
            || !bounded(self.observation_sign_id.as_str())
        {
            return Err(CapabilityUsefulnessError::EmptyIdentity);
        }
        if self.entries.is_empty() {
            return Err(CapabilityUsefulnessError::EmptyEntries);
        }
        if self.entries.len() > MAX_USEFULNESS_ENTRIES {
            return Err(CapabilityUsefulnessError::TooManyEntries);
        }
        if self.observed_at_millis > self.expires_at_millis {
            return Err(CapabilityUsefulnessError::InvalidTimeRange);
        }
        for entry in &self.entries {
            if entry.subject_id.is_empty()
                || entry.label.is_empty()
                || !bounded(&entry.subject_id)
                || !bounded(&entry.label)
                || entry
                    .kind_id
                    .as_ref()
                    .is_some_and(|id| !bounded(id.as_str()))
                || entry
                    .capability_id
                    .as_ref()
                    .is_some_and(|id| !bounded(id.as_str()))
                || entry
                    .implementation_id
                    .as_ref()
                    .is_some_and(|id| !bounded(id.as_str()))
            {
                return Err(CapabilityUsefulnessError::RefusalWithoutSubject);
            }
            if entry.resources.len() > MAX_USEFULNESS_RESOURCES {
                return Err(CapabilityUsefulnessError::TooManyResources);
            }
            if entry.resources.iter().any(|resource| {
                resource.pool_id.as_str().is_empty()
                    || resource.class_id.as_str().is_empty()
                    || !bounded(resource.pool_id.as_str())
                    || !bounded(resource.class_id.as_str())
                    || resource.reservable_units > resource.capacity_units
            }) {
                return Err(CapabilityUsefulnessError::InvalidResourceCeiling);
            }
            match &entry.disposition {
                CapabilityDisposition::Offered
                    if entry.capability_id.is_none() || entry.implementation_id.is_none() =>
                {
                    return Err(CapabilityUsefulnessError::OfferedWithoutExactImplementation);
                }
                CapabilityDisposition::CapacityInsufficient {
                    pool_id,
                    required_units,
                    reservable_units,
                } if pool_id.as_str().is_empty()
                    || !bounded(pool_id.as_str())
                    || required_units <= reservable_units =>
                {
                    return Err(CapabilityUsefulnessError::InvalidCapacityRelation);
                }
                CapabilityDisposition::ResourceMissing { class_id }
                    if class_id.as_str().is_empty() || !bounded(class_id.as_str()) =>
                {
                    return Err(CapabilityUsefulnessError::RefusalWithoutSubject);
                }
                CapabilityDisposition::BaseMissing { kind_id }
                    if kind_id.as_str().is_empty() || !bounded(kind_id.as_str()) =>
                {
                    return Err(CapabilityUsefulnessError::RefusalWithoutSubject);
                }
                CapabilityDisposition::PolicyAuthorityRefusal { requirement_id }
                    if requirement_id.is_empty() || !bounded(requirement_id) =>
                {
                    return Err(CapabilityUsefulnessError::RefusalWithoutSubject);
                }
                CapabilityDisposition::LineUnavailable { line_id }
                    if line_id.is_empty() || !bounded(line_id) =>
                {
                    return Err(CapabilityUsefulnessError::RefusalWithoutSubject);
                }
                _ => {}
            }
        }
        Ok(())
    }

    pub fn is_stale_at(&self, now_millis: u64) -> bool {
        now_millis > self.expires_at_millis
    }

    /// Human text is a projection of this structured report. Useful remaining
    /// roles sort first; neither ordering nor wording mutates the facts.
    pub fn render_text(&self, now_millis: u64) -> Result<Vec<String>, CapabilityUsefulnessError> {
        self.validate()?;
        let mut lines = Vec::new();
        push_line(
            &mut lines,
            format!(
                "HOST {} BOOT {} generation={} observed-at-millis={} expires-at-millis={} freshness={} sign={}",
                self.host_id.as_str(),
                self.boot_id.as_str(),
                self.offer_generation.0,
                self.observed_at_millis,
                self.expires_at_millis,
                if self.is_stale_at(now_millis) { "STALE" } else { "CURRENT" },
                self.observation_sign_id.as_str()
            ),
        )?;
        for useful in [true, false] {
            for entry in self
                .entries
                .iter()
                .filter(|entry| entry.disposition.is_useful() == useful)
            {
                push_line(&mut lines, render_entry(entry))?;
                for resource in &entry.resources {
                    push_line(
                        &mut lines,
                        format!(
                            "    RESOURCE {} class={} ceiling={} reservable={}",
                            resource.pool_id.as_str(),
                            resource.class_id.as_str(),
                            resource.capacity_units,
                            resource.reservable_units
                        ),
                    )?;
                }
            }
        }
        Ok(lines)
    }
}

fn render_entry(entry: &CapabilityUsefulnessEntry) -> String {
    let exact = format!(
        "subject={} kind={} capability={} implementation={}",
        entry.subject_id,
        entry.kind_id.as_ref().map_or("none", |id| id.as_str()),
        entry
            .capability_id
            .as_ref()
            .map_or("none", |id| id.as_str()),
        entry
            .implementation_id
            .as_ref()
            .map_or("none", |id| id.as_str())
    );
    match &entry.disposition {
        CapabilityDisposition::Offered => format!("  {} AVAILABLE {exact}", entry.label),
        CapabilityDisposition::Unsupported => format!("  {} UNSUPPORTED {exact}", entry.label),
        CapabilityDisposition::ImplementationMissing => {
            format!("  {} DOES NOT FIT class=portable-implementation-missing {exact}", entry.label)
        }
        CapabilityDisposition::ResourceMissing { class_id } => format!(
            "  {} DOES NOT FIT class=missing-resource resource-class={} {exact}",
            entry.label,
            class_id.as_str()
        ),
        CapabilityDisposition::BaseMissing { kind_id } => format!(
            "  {} DOES NOT FIT class=missing-base base-kind={} {exact}",
            entry.label,
            kind_id.as_str()
        ),
        CapabilityDisposition::CapacityInsufficient {
            pool_id,
            required_units,
            reservable_units,
        } => format!(
            "  {} DOES NOT FIT class=capacity-insufficient pool={} required={} reservable={} short-by={} {exact}",
            entry.label,
            pool_id.as_str(),
            required_units,
            reservable_units,
            required_units - reservable_units
        ),
        CapabilityDisposition::PolicyAuthorityRefusal { requirement_id } => format!(
            "  {} REFUSED class=policy-authority-refusal requirement={} {exact}",
            entry.label, requirement_id
        ),
        CapabilityDisposition::LineUnavailable { line_id } => format!(
            "  {} UNAVAILABLE class=line-unavailable line={} {exact}",
            entry.label, line_id
        ),
    }
}

fn push_line(lines: &mut Vec<String>, line: String) -> Result<(), CapabilityUsefulnessError> {
    let projected_bytes = lines
        .iter()
        .try_fold(line.len(), |total, existing| {
            total.checked_add(existing.len())
        })
        .ok_or(CapabilityUsefulnessError::PresentationTooLarge)?;
    if lines.len() == MAX_USEFULNESS_TEXT_LINES || projected_bytes > MAX_USEFULNESS_TEXT_BYTES {
        return Err(CapabilityUsefulnessError::PresentationTooLarge);
    }
    lines.push(line);
    Ok(())
}

fn bounded(value: &str) -> bool {
    value.len() <= MAX_USEFULNESS_FIELD_BYTES
}
