//! Allocation-free admission of resident, Make-validated computation artifacts.
//! This validates exact encoded configuration, not evaluator heap/work or authority.
use crate::{read_identity, read_u16, sha256, AssignedIdentity, AssignedPlanRefusal};

/// Supplied by the trusted image from an already validated fixed-image program.
/// The digest covers canonical hexadecimal text exactly, including its case.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AssignedComputationRequirement {
    pub node: u16,
    pub key: AssignedIdentity,
    pub text_bytes: u16,
    pub text_digest: [u8; 32],
}
impl AssignedComputationRequirement {
    pub fn from_text(node: u16, key: &str, text: &str) -> Result<Self, AssignedPlanRefusal> {
        if !matches!(key, "program" | "selector") || !canonical_hex(text.as_bytes()) {
            return Err(AssignedPlanRefusal::MalformedRecord);
        }
        Ok(Self {
            node,
            key: AssignedIdentity::from_text(key),
            text_bytes: u16::try_from(text.len())
                .map_err(|_| AssignedPlanRefusal::ComputationCapacityExceeded)?,
            text_digest: sha256::digest(text.as_bytes()),
        })
    }
}

/// This inventory must come from the resident trusted image, never the incoming
/// assigned Plan. Both per-record and aggregate limits cover encoded text bytes.
#[derive(Clone, Copy)]
pub struct AssignedComputationProfile<'a> {
    pub maximum_text_bytes: u16,
    pub maximum_total_text_bytes: u32,
    pub requirements: &'a [AssignedComputationRequirement],
}

pub(crate) struct Validation<'a> {
    profile: Option<AssignedComputationProfile<'a>>,
    seen: [bool; 32],
    total: u32,
}
impl<'a> Validation<'a> {
    pub(crate) fn new(
        profile: Option<AssignedComputationProfile<'a>>,
    ) -> Result<Self, AssignedPlanRefusal> {
        if profile.is_some_and(|p| p.requirements.len() > 32) {
            return Err(AssignedPlanRefusal::ComputationCapacityExceeded);
        }
        if let Some(p) = profile {
            for (index, requirement) in p.requirements.iter().enumerate() {
                if p.requirements[..index].iter().any(|previous| {
                    previous.node == requirement.node && previous.key == requirement.key
                }) {
                    return Err(AssignedPlanRefusal::MalformedRecord);
                }
            }
        }
        Ok(Self {
            profile,
            seen: [false; 32],
            total: 0,
        })
    }
    pub(crate) fn text(&mut self, payload: &[u8]) -> Result<bool, AssignedPlanRefusal> {
        let text = core::str::from_utf8(&payload[19..])
            .map_err(|_| AssignedPlanRefusal::MalformedRecord)?;
        let ordinary_text =
            || text.len() <= 128 && !text.trim().is_empty() && !text.chars().any(char::is_control);
        let Some(profile) = self.profile else {
            return Ok(ordinary_text());
        };
        let key = read_identity(payload, 2)?;
        if key != AssignedIdentity::from_text("program")
            && key != AssignedIdentity::from_text("selector")
        {
            return Ok(ordinary_text());
        }
        if text.len() > usize::from(profile.maximum_text_bytes) {
            return Err(AssignedPlanRefusal::ComputationCapacityExceeded);
        }
        self.total = self
            .total
            .checked_add(text.len() as u32)
            .ok_or(AssignedPlanRefusal::ComputationCapacityExceeded)?;
        if self.total > profile.maximum_total_text_bytes {
            return Err(AssignedPlanRefusal::ComputationCapacityExceeded);
        }
        if !canonical_hex(text.as_bytes()) {
            return Ok(false);
        }
        let node = read_u16(payload, 0)?;
        let digest = sha256::digest(text.as_bytes());
        let index = profile
            .requirements
            .iter()
            .enumerate()
            .position(|(index, r)| {
                !self.seen[index]
                    && r.node == node
                    && r.key == key
                    && usize::from(r.text_bytes) == text.len()
                    && r.text_digest == digest
            })
            .ok_or(AssignedPlanRefusal::UnknownComputation)?;
        self.seen[index] = true;
        Ok(true)
    }
    pub(crate) fn finish(self) -> Result<(), AssignedPlanRefusal> {
        if self
            .profile
            .is_some_and(|p| self.seen[..p.requirements.len()].iter().any(|seen| !seen))
        {
            return Err(AssignedPlanRefusal::MissingComputation);
        }
        Ok(())
    }
}
fn canonical_hex(bytes: &[u8]) -> bool {
    !bytes.is_empty()
        && bytes.len().is_multiple_of(2)
        && bytes
            .iter()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(b))
}
