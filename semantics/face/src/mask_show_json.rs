//! Prepared, lossless JSON encoding for ordinary Mask Shows.
//! Plan compression and model traversal happen during preparation. Encoding
//! copies three prepared immutable spans; it does not admit or acknowledge Show.
use crate::{MaskShow, PlannedMaskPlot};
use alloc::{boxed::Box, vec::Vec};
use serde::Serialize;
use sha2::{Digest, Sha256};
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskJsonRefusal {
    InvalidPlan,
    DifferentPlan,
    Capacity,
    Json,
}

/// Immutable wire material for one complete ordinary Plan, never a digest-only
/// substitute for its configuration. The fingerprint only guards cache reuse.
pub struct PreparedMaskPlanJson {
    fingerprint: [u8; 32],
    bytes: Box<[u8]>,
}
impl PreparedMaskPlanJson {
    /// Cold preparation allocates temporary model and compression storage.
    /// `maximum_bytes` bounds retained wire, not preparation heap.
    pub fn prepare(
        planned: &PlannedMaskPlot,
        maximum_bytes: usize,
    ) -> Result<Self, MaskJsonRefusal> {
        if maximum_bytes == 0 {
            return Err(MaskJsonRefusal::Capacity);
        }
        PlannedMaskPlot::admit(&planned.mask, &planned.plan)
            .map_err(|_| MaskJsonRefusal::InvalidPlan)?;
        let fingerprint = fingerprint(planned)?;
        let bytes = serde_json::to_vec(planned).map_err(|_| MaskJsonRefusal::Json)?;
        if bytes.len() > maximum_bytes {
            return Err(MaskJsonRefusal::Capacity);
        }
        Ok(Self {
            fingerprint,
            bytes: bytes.into_boxed_slice(),
        })
    }
    pub fn bytes(&self) -> &[u8] {
        &self.bytes
    }
    /// Preparation compares the full uncompressed model, including configuration,
    /// rather than trusting a copied Plan ID. The resulting encoder owns its
    /// metadata snapshot and borrows only this immutable wire, not the rich DTO.
    pub fn bind(
        &self,
        show: &MaskShow,
        maximum_bytes: usize,
    ) -> Result<PreparedMaskShowJson<'_>, MaskJsonRefusal> {
        if maximum_bytes == 0 {
            return Err(MaskJsonRefusal::Capacity);
        }
        if fingerprint(&show.planned_mask)? != self.fingerprint {
            return Err(MaskJsonRefusal::DifferentPlan);
        }
        #[derive(Serialize)]
        struct Prefix<'a> {
            show_id: &'a crate::MaskShowId,
            mask_plot: &'a conduit_core::PlotIdentity,
            face_plot: &'a Option<conduit_core::PlotIdentity>,
            face_plan_id: &'a Option<conduit_core::PlanId>,
            face_id: &'a crate::FaceContentId,
            face_revision: u64,
        }
        let mut prefix = serde_json::to_vec(&Prefix {
            show_id: &show.show_id,
            mask_plot: &show.mask_plot,
            face_plot: &show.face_plot,
            face_plan_id: &show.face_plan_id,
            face_id: &show.face_id,
            face_revision: show.face_revision,
        })
        .map_err(|_| MaskJsonRefusal::Json)?;
        if prefix.pop() != Some(b'}') {
            return Err(MaskJsonRefusal::Json);
        }
        prefix.extend_from_slice(b",\"planned_mask\":");
        let manifestation = serde_json::to_vec(&show.show).map_err(|_| MaskJsonRefusal::Json)?;
        let mut suffix = Vec::with_capacity(manifestation.len() + 9);
        suffix.extend_from_slice(b",\"show\":");
        suffix.extend_from_slice(&manifestation);
        suffix.push(b'}');
        let length = prefix
            .len()
            .checked_add(self.bytes.len())
            .and_then(|n| n.checked_add(suffix.len()))
            .filter(|n| *n <= maximum_bytes)
            .ok_or(MaskJsonRefusal::Capacity)?;
        Ok(PreparedMaskShowJson {
            prefix: prefix.into_boxed_slice(),
            plan: &self.bytes,
            suffix: suffix.into_boxed_slice(),
            length,
        })
    }
}
/// Representation of one immutable Show snapshot. It deliberately retains no
/// cloned Plan/Face/Show model. Current basis, lifecycle and delivery authority
/// still belong to the ordinary owner and execution, never to this encoder.
pub struct PreparedMaskShowJson<'a> {
    prefix: Box<[u8]>,
    plan: &'a [u8],
    suffix: Box<[u8]>,
    length: usize,
}
impl PreparedMaskShowJson<'_> {
    pub fn encoded_bytes(&self) -> usize {
        self.length
    }
    /// Too-small output refuses before mutation. No allocation, recompression,
    /// graph traversal or mutation of semantic identity occurs during encoding.
    pub fn encode(&self, output: &mut [u8]) -> Result<usize, MaskJsonRefusal> {
        let output = output
            .get_mut(..self.length)
            .ok_or(MaskJsonRefusal::Capacity)?;
        let mut offset = 0;
        for span in [&self.prefix[..], self.plan, &self.suffix[..]] {
            output[offset..offset + span.len()].copy_from_slice(span);
            offset += span.len();
        }
        Ok(self.length)
    }
}
fn fingerprint(planned: &PlannedMaskPlot) -> Result<[u8; 32], MaskJsonRefusal> {
    // Serialize the complete semantic members without invoking the outer Plan
    // compression again. This work is explicitly preparation, not encoding.
    let bytes =
        serde_json::to_vec(&(&planned.mask, &planned.plan)).map_err(|_| MaskJsonRefusal::Json)?;
    let mut hash = Sha256::new();
    hash.update(b"conduit.face/prepared-mask-json@1");
    hash.update(bytes);
    Ok(hash.finalize().into())
}
