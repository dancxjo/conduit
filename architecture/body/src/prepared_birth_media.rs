//! Inert, bounded transport of an already recorded pristine birth. Decoding
//! retains evidence; it does not birth, admit, wake, create a Boot or plan work.
use crate::{
    Body, BodyBiographyEvidence, BodyBiographyRecord, BodyBiographyRecordKind,
    BodyIdentityDerivation, BodyLifecycleEvent, BodyMembership, BodyMembershipRevision, BodyState,
    BodyWorkset,
};
use alloc::{string::String, vec, vec::Vec};
use conduit_core::SignId;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const PREPARED_BIRTH_MEDIA_BYTES: usize = 32 * 1024;
pub const PREPARED_BIRTH_MEDIA_HEADER_BYTES: usize = 48;
pub const MAX_PREPARED_BIRTH_BINDING_BYTES: usize = 512;
pub const PREPARED_BIRTH_MEDIA_MAGIC: &[u8; 8] = b"CDBIRTH1";
pub const PREPARED_BIRTH_MEDIA_SCHEMA: &str = "conduit.body/prepared-birth-media@1";
const VERSION: u16 = 1;
const PAYLOAD_BYTES: usize = PREPARED_BIRTH_MEDIA_BYTES - PREPARED_BIRTH_MEDIA_HEADER_BYTES;

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct PreparedBirthMediaBinding {
    pub host_entry: String,
    pub first_host_entry: String,
    pub construction_id: String,
    pub configuration_id: String,
    pub profile_id: String,
    pub build_id: String,
    pub target: String,
}
impl PreparedBirthMediaBinding {
    fn validate(&self) -> Result<(), PreparedBirthMediaError> {
        for value in [
            &self.host_entry,
            &self.first_host_entry,
            &self.construction_id,
            &self.configuration_id,
            &self.profile_id,
            &self.build_id,
            &self.target,
        ] {
            if value.trim().is_empty()
                || value.len() > MAX_PREPARED_BIRTH_BINDING_BYTES
                || value.chars().any(char::is_control)
            {
                return Err(PreparedBirthMediaError::InvalidBinding);
            }
        }
        Ok(())
    }
}
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum PreparedBirthHostRole {
    /// Retain the prepared birth, then perform ordinary first-Host admission.
    FirstHost,
    /// Wait for ordinary join to the existing Body. This media is not rebirth.
    LaterHost,
}
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PreparedBirthMediaError {
    InvalidFrame,
    UnsupportedVersion,
    InvalidDigest,
    NonCanonical,
    InvalidBirth,
    InvalidBinding,
    CapacityExceeded,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PreparedBirthMedia {
    evidence: BodyBiographyEvidence,
    binding: PreparedBirthMediaBinding,
}
// Positional postcard schema deliberately contains no conditional serde fields.
// Existing biography JSON structs skip absent fields and cannot be used as a
// positional binary schema. Every omitted lifecycle field is fixed by pristine
// birth validation, then reconstructed exactly without executing birth again.
#[derive(Serialize, Deserialize)]
struct Payload {
    body_id: crate::BodyId,
    derivation: BodyIdentityDerivation,
    workset: BirthWorkset,
    birth_sequence: u64,
    birth_sign: SignId,
    friendly_name: String,
    binding: PreparedBirthMediaBinding,
}
// Reject a forged sequence length before allocation. The wire has at most the
// ordinary bounded Body workset, even when its input frame is untrusted.
#[derive(Serialize)]
struct BirthWorkset(Vec<crate::ResidentPlot>);
impl<'de> Deserialize<'de> for BirthWorkset {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        struct Visitor;
        impl<'de> serde::de::Visitor<'de> for Visitor {
            type Value = BirthWorkset;
            fn expecting(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                f.write_str("a bounded retained birth workset")
            }
            fn visit_seq<A: serde::de::SeqAccess<'de>>(
                self,
                mut seq: A,
            ) -> Result<Self::Value, A::Error> {
                use serde::de::Error;
                if seq.size_hint().is_some_and(|n| n > crate::MAX_BODY_PLOTS) {
                    return Err(A::Error::custom("birth workset capacity"));
                }
                let mut plots = Vec::with_capacity(crate::MAX_BODY_PLOTS);
                let mut identity_bytes = 0usize;
                while let Some(plot) = seq.next_element::<crate::ResidentPlot>()? {
                    if plots.len() == crate::MAX_BODY_PLOTS {
                        return Err(A::Error::custom("birth workset capacity"));
                    }
                    identity_bytes = identity_bytes
                        .checked_add(plot.source_document_id.as_str().len())
                        .and_then(|n| n.checked_add(plot.checked_plot_id.as_str().len()))
                        .ok_or_else(|| A::Error::custom("birth identity capacity"))?;
                    if identity_bytes > crate::MAX_BODY_PLOT_IDENTITY_BYTES {
                        return Err(A::Error::custom("birth identity capacity"));
                    }
                    plots.push(plot);
                }
                Ok(BirthWorkset(plots))
            }
        }
        deserializer.deserialize_seq(Visitor)
    }
}
impl PreparedBirthMedia {
    pub fn from_evidence(
        evidence: &BodyBiographyEvidence,
        binding: PreparedBirthMediaBinding,
    ) -> Result<Self, PreparedBirthMediaError> {
        validate_pristine(evidence)?;
        binding.validate()?;
        let media = Self {
            evidence: evidence.clone(),
            binding,
        };
        let mut scratch = vec![0; PAYLOAD_BYTES];
        postcard::to_slice(&media.payload(), &mut scratch)
            .map_err(|_| PreparedBirthMediaError::CapacityExceeded)?;
        Ok(media)
    }
    pub fn evidence(&self) -> &BodyBiographyEvidence {
        &self.evidence
    }
    pub fn binding(&self) -> &PreparedBirthMediaBinding {
        &self.binding
    }
    pub fn role(&self) -> PreparedBirthHostRole {
        if self.binding.host_entry == self.binding.first_host_entry {
            PreparedBirthHostRole::FirstHost
        } else {
            PreparedBirthHostRole::LaterHost
        }
    }
    pub fn into_evidence(self) -> BodyBiographyEvidence {
        self.evidence
    }
    pub fn encode_into(&self, destination: &mut [u8]) -> Result<usize, PreparedBirthMediaError> {
        validate_pristine(&self.evidence)?;
        self.binding.validate()?;
        let frame = destination
            .get_mut(..PREPARED_BIRTH_MEDIA_BYTES)
            .ok_or(PreparedBirthMediaError::CapacityExceeded)?;
        frame.fill(0);
        let encoded = postcard::to_slice(
            &self.payload(),
            &mut frame[PREPARED_BIRTH_MEDIA_HEADER_BYTES..],
        )
        .map_err(|_| PreparedBirthMediaError::CapacityExceeded)?;
        let length = encoded.len();
        let digest = Sha256::digest(encoded);
        frame[..8].copy_from_slice(PREPARED_BIRTH_MEDIA_MAGIC);
        frame[8..10].copy_from_slice(&VERSION.to_le_bytes());
        frame[12..16].copy_from_slice(&(length as u32).to_le_bytes());
        frame[16..48].copy_from_slice(&digest);
        Ok(PREPARED_BIRTH_MEDIA_BYTES)
    }
    pub fn decode(frame: &[u8]) -> Result<Self, PreparedBirthMediaError> {
        if frame.len() != PREPARED_BIRTH_MEDIA_BYTES
            || &frame[..8] != PREPARED_BIRTH_MEDIA_MAGIC
            || frame[10..12] != [0, 0]
        {
            return Err(PreparedBirthMediaError::InvalidFrame);
        }
        if u16::from_le_bytes([frame[8], frame[9]]) != VERSION {
            return Err(PreparedBirthMediaError::UnsupportedVersion);
        }
        let length = u32::from_le_bytes(
            frame[12..16]
                .try_into()
                .map_err(|_| PreparedBirthMediaError::InvalidFrame)?,
        ) as usize;
        if length == 0 || length > PAYLOAD_BYTES {
            return Err(PreparedBirthMediaError::InvalidFrame);
        }
        let end = PREPARED_BIRTH_MEDIA_HEADER_BYTES + length;
        if frame[end..].iter().any(|byte| *byte != 0) {
            return Err(PreparedBirthMediaError::NonCanonical);
        }
        let encoded = &frame[PREPARED_BIRTH_MEDIA_HEADER_BYTES..end];
        if Sha256::digest(encoded).as_slice() != &frame[16..48] {
            return Err(PreparedBirthMediaError::InvalidDigest);
        }
        let payload: Payload =
            postcard::from_bytes(encoded).map_err(|_| PreparedBirthMediaError::InvalidFrame)?;
        let mut scratch = vec![0; PAYLOAD_BYTES];
        let canonical = postcard::to_slice(&payload, &mut scratch)
            .map_err(|_| PreparedBirthMediaError::CapacityExceeded)?;
        if canonical != encoded {
            return Err(PreparedBirthMediaError::NonCanonical);
        }
        payload.into_media()
    }
    fn payload(&self) -> Payload {
        Payload {
            body_id: self.evidence.body_id.clone(),
            derivation: self.evidence.body.identity_derivation,
            workset: BirthWorkset(self.evidence.body.workset.plots().to_vec()),
            birth_sequence: self.evidence.body.birth_sequence,
            birth_sign: self.evidence.body.sign_ids[0].clone(),
            friendly_name: self.evidence.friendly_name.clone(),
            binding: self.binding.clone(),
        }
    }
}
fn validate_pristine(evidence: &BodyBiographyEvidence) -> Result<(), PreparedBirthMediaError> {
    evidence
        .validate()
        .map_err(|_| PreparedBirthMediaError::InvalidBirth)?;
    let body = &evidence.body;
    let membership = &evidence.membership;
    if body.state != BodyState::Lulled
        || body.workload_revision != 0
        || body.events.len() != 1
        || body.sign_ids.len() != 1
        || body.history_checkpoint.is_some()
        || !matches!(
            body.events.first(),
            Some(BodyLifecycleEvent::Born {
                workload_revision: 0,
                ..
            })
        )
        || membership.revision != BodyMembershipRevision(0)
        || !membership.parts.is_empty()
        || !membership.events.is_empty()
        || membership.checkpoint.is_some()
        || membership.fulfilled_sign_id.is_some()
        || evidence.records.len() != 1
        || !evidence.wakes.is_empty()
        || evidence.compaction.is_some()
        || evidence.graduation.is_some()
        || evidence.emergency.is_some()
    {
        return Err(PreparedBirthMediaError::InvalidBirth);
    }
    Ok(())
}
impl Payload {
    fn into_media(self) -> Result<PreparedBirthMedia, PreparedBirthMediaError> {
        self.binding.validate()?;
        let workset = BodyWorkset::from_plots(self.workset.0)
            .map_err(|_| PreparedBirthMediaError::InvalidBirth)?;
        let body = Body {
            body_id: self.body_id.clone(),
            identity_derivation: self.derivation,
            workset: workset.clone(),
            workload_revision: 0,
            birth_sequence: self.birth_sequence,
            state: BodyState::Lulled,
            history_checkpoint: None,
            sign_ids: vec![self.birth_sign.clone()],
            events: vec![BodyLifecycleEvent::Born {
                initial_workset: workset.clone(),
                workload_revision: 0,
                sign_id: self.birth_sign.clone(),
            }],
        };
        let evidence = BodyBiographyEvidence {
            schema: "conduit.body/biography-evidence@2".into(),
            body_id: self.body_id.clone(),
            friendly_name: self.friendly_name,
            body,
            emergency: None,
            membership: BodyMembership {
                body_id: self.body_id,
                revision: BodyMembershipRevision(0),
                parts: Vec::new(),
                checkpoint: None,
                events: Vec::new(),
                fulfilled_sign_id: None,
            },
            graduation: None,
            records: vec![BodyBiographyRecord {
                sequence: self.birth_sequence,
                sign_id: self.birth_sign,
                kind: BodyBiographyRecordKind::Born {
                    initial_workset: workset,
                    workload_revision: 0,
                },
            }],
            compaction: None,
            wakes: Vec::new(),
        };
        validate_pristine(&evidence)?;
        Ok(PreparedBirthMedia {
            evidence,
            binding: self.binding,
        })
    }
}
#[cfg(test)]
#[path = "prepared_birth_media/tests.rs"]
mod tests;
