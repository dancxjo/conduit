//! One attributed preparation document: borrowed scope followed by exact tables.
use crate::{body_play_identity_matches, PreparedBodyFragment, MAX_LIFECYCLE_ID_BYTES};
use alloc::vec::Vec;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// Public wire data, not permission or evidence of full Plan validation.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct BodyFragmentApprovalMetadata<'a> {
    pub schema: u16,
    #[serde(borrow)]
    pub body: &'a str,
    pub wake: &'a str,
    pub body_plan: &'a str,
    pub body_play: &'a str,
    pub play_sequence: u64,
    pub workload_revision: u64,
    pub source: &'a str,
    pub checked_plot: &'a str,
    pub expanded_plot: &'a str,
    pub plot_plan: &'a str,
    pub fragment: &'a str,
    pub host: &'a str,
    pub boot: &'a str,
    pub offer_generation: u64,
    pub local_fragment_count: u16,
    pub local_fragment_digest: [u8; 32],
    #[serde(borrow)]
    pub resource_bindings: &'a [u8],
    #[serde(borrow)]
    pub fore_ports: &'a [u8],
    pub projection_length: u32,
    pub projection_digest: [u8; 32],
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum BodyFragmentDocumentRefusal {
    Bounds,
    Metadata,
    Projection,
}

impl<'a> BodyFragmentApprovalMetadata<'a> {
    /// Encoding aid only. The sender must independently generate the exact
    /// canonical projection from this prepared fragment before signing it.
    pub fn from_prepared(
        scope: PreparedBodyFragment<'a>,
        projection: &[u8],
        resource_bindings: &'a [u8],
        fore_ports: &'a [u8],
    ) -> Result<Self, BodyFragmentDocumentRefusal> {
        let plan = scope.plan();
        let play = scope.play();
        let f = scope.fragment();
        let (local_fragment_count, local_fragment_digest) = workset::local_fragment_scope(scope)?;
        let metadata = Self {
            schema: 3,
            body: plan.body_id.as_str(),
            wake: plan.wake_id.as_str(),
            body_plan: plan.plan_id.as_str(),
            body_play: play.active_play_id.as_str(),
            play_sequence: play.play_sequence,
            workload_revision: plan.workload_revision,
            source: f.source_document_id.as_str(),
            checked_plot: f.checked_plot_id.as_str(),
            expanded_plot: f.expanded_plot_id.as_str(),
            plot_plan: f.plan_id.as_str(),
            fragment: f.fragment_id.as_str(),
            host: f.host_id.as_str(),
            boot: f.boot_id.as_str(),
            offer_generation: f.offer_generation.0,
            local_fragment_count,
            local_fragment_digest,
            resource_bindings,
            fore_ports,
            projection_length: u32::try_from(projection.len())
                .map_err(|_| BodyFragmentDocumentRefusal::Bounds)?,
            projection_digest: Sha256::digest(projection).into(),
        };
        metadata.check()?;
        Ok(metadata)
    }
    pub(super) fn check(&self) -> Result<(), BodyFragmentDocumentRefusal> {
        if self.schema != 3
            || self.offer_generation == 0
            || self.local_fragment_count == 0
            || self.projection_length == 0
            || ![
                self.body,
                self.wake,
                self.body_plan,
                self.body_play,
                self.source,
                self.checked_plot,
                self.expanded_plot,
                self.plot_plan,
                self.fragment,
                self.host,
                self.boot,
            ]
            .iter()
            .all(|s| !s.is_empty() && s.len() <= MAX_LIFECYCLE_ID_BYTES)
            || !body_play_identity_matches(
                self.body,
                self.wake,
                self.body_plan,
                self.play_sequence,
                self.body_play,
            )
        {
            return Err(BodyFragmentDocumentRefusal::Metadata);
        }
        Ok(())
    }
}

/// Postcard is self-delimiting; the remainder is the existing assigned format.
/// This encoder issues no approval, signature, admission or execution grant.
pub fn encode_body_fragment_document(
    metadata: &BodyFragmentApprovalMetadata<'_>,
    projection: &[u8],
    maximum_bytes: u32,
) -> Result<Vec<u8>, BodyFragmentDocumentRefusal> {
    metadata.check()?;
    if projection.len() as u64 != u64::from(metadata.projection_length)
        || Sha256::digest(projection).as_slice() != metadata.projection_digest
    {
        return Err(BodyFragmentDocumentRefusal::Projection);
    }
    let header = postcard::experimental::serialized_size(metadata)
        .map_err(|_| BodyFragmentDocumentRefusal::Metadata)?;
    let total = header
        .checked_add(projection.len())
        .ok_or(BodyFragmentDocumentRefusal::Bounds)?;
    if total as u64 > u64::from(maximum_bytes) {
        return Err(BodyFragmentDocumentRefusal::Bounds);
    }
    let mut bytes = Vec::new();
    bytes
        .try_reserve_exact(total)
        .map_err(|_| BodyFragmentDocumentRefusal::Bounds)?;
    bytes.resize(total, 0);
    postcard::to_slice(metadata, &mut bytes[..header])
        .map_err(|_| BodyFragmentDocumentRefusal::Metadata)?;
    bytes[header..].copy_from_slice(projection);
    Ok(bytes)
}

#[cfg(feature = "authenticated-admission")]
mod receiving;
#[cfg(feature = "authenticated-admission")]
pub use receiving::*;

mod resources;
pub use resources::*;

mod workset;

/// Parse public document data, not an approval, reservation or execution right.
/// A receiving Root must separately authenticate and apply installed delegation.
pub fn decode_body_fragment_document(
    bytes: &[u8],
) -> Result<(BodyFragmentApprovalMetadata<'_>, &[u8]), BodyFragmentDocumentRefusal> {
    let (metadata, projection): (BodyFragmentApprovalMetadata<'_>, _) =
        postcard::take_from_bytes(bytes).map_err(|_| BodyFragmentDocumentRefusal::Metadata)?;
    metadata.check()?;
    if projection.len() as u64 != u64::from(metadata.projection_length)
        || Sha256::digest(projection).as_slice() != metadata.projection_digest
    {
        return Err(BodyFragmentDocumentRefusal::Projection);
    }
    Ok((metadata, projection))
}

#[cfg(feature = "authenticated-admission")]
mod approved_fore;
#[cfg(feature = "authenticated-admission")]
pub use approved_fore::*;

#[cfg(feature = "authenticated-admission")]
mod line_fore;
#[cfg(feature = "authenticated-admission")]
pub use line_fore::*;

mod preparation_workset;
pub use preparation_workset::*;

#[cfg(feature = "authenticated-admission")]
mod continuation;
#[cfg(feature = "authenticated-admission")]
pub use continuation::*;

/// Canonical commitment of sorted unique local fragment identities; encoding
/// data only, not proof of completeness or execution permission.
pub fn local_fragment_set_commitment(
    ids: &[&str],
) -> Result<(u16, [u8; 32]), BodyFragmentDocumentRefusal> {
    workset::fragment_set_commitment(ids)
}
