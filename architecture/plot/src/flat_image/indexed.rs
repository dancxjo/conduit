//! Indexed trusted Make image: metadata and one decoded Gear at a time.
//! This is checked meaning, not a Plan, current offer, or execution authority.
use super::{Image, ImageRefusal as E, Origin};
use crate::{CheckedGear, ExpandedCanonicalPlot, ExpandedGearProvenance, PlotCompletionPolicy};
use alloc::vec::Vec;

const MAGIC: &[u8; 8] = b"CNDGIDX1";
const PREFIX: usize = 16;
pub const MAXIMUM_GEARS: usize = 32;
#[cfg(not(target_pointer_width = "16"))]
pub const MAXIMUM_BYTES: usize = 256 * 1024;
#[cfg(target_pointer_width = "16")]
pub const MAXIMUM_BYTES: usize = 32 * 1024;
const MAXIMUM_METADATA_BYTES: usize = 32 * 1024;
#[cfg(not(target_pointer_width = "16"))]
const MAXIMUM_GEAR_BYTES: usize = 64 * 1024;
#[cfg(target_pointer_width = "16")]
const MAXIMUM_GEAR_BYTES: usize = 32 * 1024;

/// Keep the index and payloads borrowed from flash. Decoding a Gear does not
/// decode or retain its neighbours. No mutable graph is cached in this view.
pub struct IndexedSourceImage<'a> {
    bytes: &'a [u8],
    count: usize,
    metadata_start: usize,
    payload_start: usize,
}
/// A source header is deliberately distinct from a complete checked expansion.
pub struct IndexedSourceMetadata {
    source: ExpandedCanonicalPlot,
    gears: usize,
}
impl IndexedSourceMetadata {
    pub fn identity(&self) -> conduit_core::PlotIdentity {
        conduit_core::PlotIdentity {
            source_document_id: self.source.source_document_id.clone(),
            checked_plot_id: self.source.checked_plot_id.clone(),
            expanded_plot_id: self.source.expanded_plot_id.clone(),
        }
    }
    pub fn name(&self) -> &str {
        &self.source.name
    }
    pub fn completion(&self) -> PlotCompletionPolicy {
        self.source.completion
    }
    pub fn connections(&self) -> &[crate::CheckedConnection] {
        &self.source.connections
    }
    pub fn provenance(&self) -> &[ExpandedGearProvenance] {
        &self.source.provenance
    }
    /// Hosted conformance/reconstruction; an MCU planner need not materialize
    /// this collection. Restores and validates the original complete expansion.
    pub fn into_expanded(mut self, gears: Vec<CheckedGear>) -> Result<ExpandedCanonicalPlot, E> {
        if gears.len() != self.gears {
            return Err(E::UnsupportedGraph);
        }
        self.source.gears = gears;
        self.source
            .validate_expansion()
            .map_err(|_| E::Provenance)?;
        Ok(self.source)
    }
}

pub fn encode(source: &ExpandedCanonicalPlot) -> Result<Vec<u8>, E> {
    if source.gears.is_empty()
        || source.gears.len() > MAXIMUM_GEARS
        || source.connections.len() > 64
        || !source.activations.is_empty()
        || !source.shared_pools.is_empty()
        || !source.realization_backs.is_empty()
        || source
            .provenance
            .iter()
            .any(|p| p.construction_owner.is_some())
    {
        return Err(E::UnsupportedGraph);
    }
    source.validate_expansion().map_err(|_| E::Provenance)?;
    let metadata = Image {
        version: 1,
        source_document_id: source.source_document_id.clone(),
        checked_plot_id: source.checked_plot_id.clone(),
        expanded_plot_id: source.expanded_plot_id.clone(),
        name: source.name.clone(),
        semantic_completion: source.completion == PlotCompletionPolicy::SemanticCompletion,
        gears: Vec::new(),
        connections: source.connections.clone(),
        provenance: source
            .provenance
            .iter()
            .map(|p| Origin {
                gear_id: p.gear_id.clone(),
                plot_path: p.plot_path.clone(),
                source_plot: p.source_plot.clone(),
                source_gear: p.source_gear.clone(),
                source_span: p.source_span,
            })
            .collect(),
        connection_provenance: source.connection_provenance.clone(),
        provenance_digest: source.provenance_digest.clone(),
    };
    let metadata = serde_json::to_vec(&metadata).map_err(|_| E::Representation)?;
    if metadata.len() > MAXIMUM_METADATA_BYTES {
        return Err(E::Oversized);
    }
    let count = source.gears.len();
    let mut bytes = Vec::new();
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&(count as u16).to_le_bytes());
    bytes.extend_from_slice(&0u16.to_le_bytes());
    bytes.extend_from_slice(&(metadata.len() as u32).to_le_bytes());
    bytes.resize(PREFIX + (count + 1) * 4, 0);
    bytes.extend_from_slice(&metadata);
    let payload_start = bytes.len();
    for (i, gear) in source.gears.iter().enumerate() {
        let gear = serde_json::to_vec(gear).map_err(|_| E::Representation)?;
        if gear.len() > MAXIMUM_GEAR_BYTES {
            return Err(E::Oversized);
        }
        if bytes
            .len()
            .checked_add(gear.len())
            .is_none_or(|n| n > MAXIMUM_BYTES)
        {
            return Err(E::Oversized);
        }
        bytes.extend_from_slice(&gear);
        let offset = (bytes.len() - payload_start) as u32;
        bytes[PREFIX + (i + 1) * 4..PREFIX + (i + 2) * 4].copy_from_slice(&offset.to_le_bytes());
    }
    Ok(bytes)
}
impl<'a> IndexedSourceImage<'a> {
    /// Authenticate the complete immutable bytes and their checked Make origin
    /// independently before calling. Bounds/index validation is not Source
    /// checking. Never use this as an ingress for network/user-supplied graphs.
    /// # Safety
    /// Same trusted installed-image contract as `flat_image::decode_trusted`.
    pub unsafe fn from_make_image(bytes: &'a [u8]) -> Result<Self, E> {
        if bytes.len() > MAXIMUM_BYTES {
            return Err(E::Oversized);
        }
        if bytes.get(..8) != Some(MAGIC) || bytes.get(10..12) != Some(&[0, 0]) {
            return Err(E::Representation);
        }
        let count = usize::from(u16::from_le_bytes(
            bytes
                .get(8..10)
                .ok_or(E::Representation)?
                .try_into()
                .unwrap(),
        ));
        if count == 0 || count > MAXIMUM_GEARS {
            return Err(E::UnsupportedGraph);
        }
        let metadata_start = PREFIX + (count + 1) * 4;
        let length = word(bytes, 12)?;
        if length > MAXIMUM_METADATA_BYTES {
            return Err(E::Oversized);
        }
        let payload_start = metadata_start
            .checked_add(length)
            .filter(|n| *n <= bytes.len())
            .ok_or(E::Representation)?;
        let mut previous = word(bytes, PREFIX)?;
        if previous != 0 {
            return Err(E::Representation);
        }
        for i in 1..=count {
            let end = word(bytes, PREFIX + i * 4)?;
            if end <= previous
                || end - previous > MAXIMUM_GEAR_BYTES
                || end > bytes.len() - payload_start
            {
                return Err(E::Representation);
            }
            previous = end;
        }
        if previous != bytes.len() - payload_start {
            return Err(E::Representation);
        }
        Ok(Self {
            bytes,
            count,
            metadata_start,
            payload_start,
        })
    }
    pub fn gear_count(&self) -> usize {
        self.count
    }
    pub fn gear(&self, index: usize) -> Result<CheckedGear, E> {
        if index >= self.count {
            return Err(E::Representation);
        }
        let start = word(self.bytes, PREFIX + index * 4)?;
        let end = word(self.bytes, PREFIX + (index + 1) * 4)?;
        serde_json::from_slice(&self.bytes[self.payload_start + start..self.payload_start + end])
            .map_err(|_| E::Representation)
    }
    /// Read only the checked identity for installation/workset matching. Serde
    /// skips the remaining metadata without materializing its graph records.
    pub fn identity(&self) -> Result<conduit_core::PlotIdentity, E> {
        #[derive(serde::Deserialize)]
        struct Identity {
            version: u8,
            source_document_id: conduit_core::SourceDocumentId,
            checked_plot_id: conduit_core::CheckedPlotId,
            expanded_plot_id: conduit_core::ExpandedPlotId,
        }
        let identity: Identity =
            serde_json::from_slice(&self.bytes[self.metadata_start..self.payload_start])
                .map_err(|_| E::Representation)?;
        if identity.version != 1 {
            return Err(E::UnsupportedGraph);
        }
        Ok(conduit_core::PlotIdentity {
            source_document_id: identity.source_document_id,
            checked_plot_id: identity.checked_plot_id,
            expanded_plot_id: identity.expanded_plot_id,
        })
    }
    pub fn metadata(&self) -> Result<IndexedSourceMetadata, E> {
        let image: Image =
            serde_json::from_slice(&self.bytes[self.metadata_start..self.payload_start])
                .map_err(|_| E::Representation)?;
        if image.version != 1
            || !image.gears.is_empty()
            || image.connections.len() > 64
            || image.provenance.len() != self.count
        {
            return Err(E::UnsupportedGraph);
        }
        Ok(IndexedSourceMetadata {
            gears: self.count,
            source: ExpandedCanonicalPlot {
                source_document_id: image.source_document_id,
                checked_plot_id: image.checked_plot_id,
                expanded_plot_id: image.expanded_plot_id,
                name: image.name,
                completion: if image.semantic_completion {
                    PlotCompletionPolicy::SemanticCompletion
                } else {
                    PlotCompletionPolicy::Live
                },
                gears: Vec::new(),
                connections: image.connections,
                connection_provenance: image.connection_provenance,
                provenance: image
                    .provenance
                    .into_iter()
                    .map(|p| ExpandedGearProvenance {
                        construction_owner: None,
                        gear_id: p.gear_id,
                        plot_path: p.plot_path,
                        source_plot: p.source_plot,
                        source_gear: p.source_gear,
                        source_span: p.source_span,
                    })
                    .collect(),
                provenance_digest: image.provenance_digest,
                activations: Vec::new(),
                shared_pools: Vec::new(),
                realization_backs: Vec::new(),
            },
        })
    }
}
fn word(bytes: &[u8], at: usize) -> Result<usize, E> {
    usize::try_from(u32::from_le_bytes(
        bytes
            .get(at..at + 4)
            .ok_or(E::Representation)?
            .try_into()
            .unwrap(),
    ))
    .map_err(|_| E::Oversized)
}
