//! Representation of a small, already checked flat graph for trusted Make
//! images. This is not a Source checker, planner, grant or native assignment.
use crate::{ExpandedCanonicalPlot, ExpandedGearProvenance, PlotCompletionPolicy, Span};
use alloc::{string::String, vec::Vec};
use serde::{Deserialize, Serialize};

pub const MAXIMUM_IMAGE_BYTES: usize = 32 * 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageRefusal {
    UnsupportedGraph,
    Oversized,
    Representation,
    Provenance,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Image {
    version: u8,
    source_document_id: conduit_core::SourceDocumentId,
    checked_plot_id: conduit_core::CheckedPlotId,
    expanded_plot_id: conduit_core::ExpandedPlotId,
    name: String,
    semantic_completion: bool,
    gears: Vec<crate::CheckedGear>,
    connections: Vec<crate::CheckedConnection>,
    provenance: Vec<Origin>,
    connection_provenance: Vec<crate::ExpandedConnectionProvenance>,
    provenance_digest: String,
}
#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct Origin {
    gear_id: String,
    plot_path: Vec<String>,
    source_plot: String,
    source_gear: String,
    source_span: Span,
}

/// Export exact checked meaning and Source provenance. Advanced expansion and
/// compiler-owned construction scopes are explicitly outside this small profile.
pub fn encode(source: &ExpandedCanonicalPlot) -> Result<Vec<u8>, ImageRefusal> {
    if source.gears.is_empty()
        || source.gears.len() > 8
        || source.connections.len() > 16
        || !source.activations.is_empty()
        || !source.shared_pools.is_empty()
        || !source.realization_backs.is_empty()
        || source
            .provenance
            .iter()
            .any(|p| p.construction_owner.is_some())
    {
        return Err(ImageRefusal::UnsupportedGraph);
    }
    source
        .validate_source_provenance()
        .map_err(|_| ImageRefusal::Provenance)?;
    let image = Image {
        version: 1,
        source_document_id: source.source_document_id.clone(),
        checked_plot_id: source.checked_plot_id.clone(),
        expanded_plot_id: source.expanded_plot_id.clone(),
        name: source.name.clone(),
        semantic_completion: source.completion == PlotCompletionPolicy::SemanticCompletion,
        gears: source.gears.clone(),
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
    let bytes = serde_json::to_vec(&image).map_err(|_| ImageRefusal::Representation)?;
    if bytes.len() > MAXIMUM_IMAGE_BYTES {
        return Err(ImageRefusal::Oversized);
    }
    Ok(bytes)
}

/// Reconstitute build-time checked meaning, without inventing a Plan or Boot.
/// The image never carries compiler-owned construction authority.
///
/// # Safety
/// Caller must authenticate the complete bytes and their checked-Source origin
/// against an independently trusted Make image before calling this function.
/// Byte size, JSON decoding and retained provenance are not Source checking or
/// evidence that caller-supplied identities describe authored meaning. This
/// function is not an ingress for arbitrary network or user-provided graphs.
pub unsafe fn decode_trusted(bytes: &[u8]) -> Result<ExpandedCanonicalPlot, ImageRefusal> {
    if bytes.len() > MAXIMUM_IMAGE_BYTES {
        return Err(ImageRefusal::Oversized);
    }
    let image: Image = serde_json::from_slice(bytes).map_err(|_| ImageRefusal::Representation)?;
    if image.version != 1
        || image.gears.is_empty()
        || image.gears.len() > 8
        || image.connections.len() > 16
    {
        return Err(ImageRefusal::UnsupportedGraph);
    }
    let source = ExpandedCanonicalPlot {
        source_document_id: image.source_document_id,
        checked_plot_id: image.checked_plot_id,
        expanded_plot_id: image.expanded_plot_id,
        name: image.name,
        completion: if image.semantic_completion {
            PlotCompletionPolicy::SemanticCompletion
        } else {
            PlotCompletionPolicy::Live
        },
        gears: image.gears,
        connections: image.connections,
        connection_provenance: image.connection_provenance,
        shared_pools: Vec::new(),
        realization_backs: Vec::new(),
        activations: Vec::new(),
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
    };
    source
        .validate_source_provenance()
        .map_err(|_| ImageRefusal::Provenance)?;
    Ok(source)
}
