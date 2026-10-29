//! Indexed trusted Make image: metadata and one decoded Gear at a time.
//! This is checked meaning, not a Plan, current offer, or execution authority.
use super::{Image, ImageRefusal as E, Origin};
use crate::{CheckedGear, ExpandedCanonicalPlot, ExpandedGearProvenance, PlotCompletionPolicy};
use alloc::vec::Vec;

#[path = "indexed_decoders.rs"]
mod decoders;

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
    gear_driver: fn(&IndexedSourceImage<'_>, usize) -> Result<CheckedGear, E>,
    configuration_driver: fn(&IndexedSourceImage<'_>, usize) -> Result<IndexedGearConfiguration, E>,
    gear_decoder: Option<fn(usize) -> Result<CheckedGear, E>>,
    configuration_decoder: Option<fn(usize) -> Result<IndexedGearConfiguration, E>>,
    planning_facts: Option<fn() -> Result<crate::LinkedPlanningFactsRef<'static>, E>>,
    count: usize,
    metadata_start: usize,
    payload_start: usize,
}
/// Exact configuration projection of one checked Gear; no execution authority.
#[derive(serde::Deserialize)]
pub struct IndexedGearConfiguration {
    pub gear_id: conduit_core::GearId,
    pub configuration: Vec<conduit_core::ConfigurationEntry>,
}
/// A source header is deliberately distinct from a complete checked expansion.
pub struct IndexedSourceMetadata {
    source: ExpandedCanonicalPlot,
    gears: usize,
}
/// Planning's projection of an already checked Make image. Complete expansion
/// provenance remains in the authenticated image; this view cannot reconstruct
/// or masquerade as an ExpandedCanonicalPlot.
pub struct IndexedPlanningMetadata(PlanningMetadataWire);
#[derive(serde::Deserialize)]
struct PlanningMetadataWire {
    version: u8,
    source_document_id: conduit_core::SourceDocumentId,
    checked_plot_id: conduit_core::CheckedPlotId,
    expanded_plot_id: conduit_core::ExpandedPlotId,
    name: alloc::string::String,
    semantic_completion: bool,
    gears: decoders::EmptyGears,
    connections: Vec<crate::CheckedConnection>,
    provenance: Vec<PlanningOrigin>,
}
#[derive(serde::Deserialize)]
struct PlanningOrigin {
    gear_id: alloc::string::String,
    source_span: crate::Span,
}
impl IndexedPlanningMetadata {
    pub fn identity(&self) -> conduit_core::PlotIdentity {
        conduit_core::PlotIdentity {
            source_document_id: self.0.source_document_id.clone(),
            checked_plot_id: self.0.checked_plot_id.clone(),
            expanded_plot_id: self.0.expanded_plot_id.clone(),
        }
    }
    pub fn name(&self) -> &str {
        &self.0.name
    }
    pub fn completion(&self) -> PlotCompletionPolicy {
        if self.0.semantic_completion {
            PlotCompletionPolicy::SemanticCompletion
        } else {
            PlotCompletionPolicy::Live
        }
    }
    pub fn connections(&self) -> &[crate::CheckedConnection] {
        &self.0.connections
    }
    /// Transfer decoded Cord preparation data. The complete checked Source
    /// remains immutable in the Make image and available through `metadata`.
    pub fn take_connections(&mut self) -> Vec<crate::CheckedConnection> {
        core::mem::take(&mut self.0.connections)
    }
    pub fn source_span(&self, gear: &str) -> Option<crate::Span> {
        self.0
            .provenance
            .iter()
            .find(|p| p.gear_id == gear)
            .map(|p| p.source_span)
    }
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
    /// Use exact linked reconstruction rather than copying immutable default
    /// programs from JSON. The index and identity remain in this Make image.
    /// # Safety
    /// The trusted Make owner must verify that every decoder result is exactly
    /// the complete corresponding Gear in this same independently pinned image.
    /// A received callback or a matching claimed identity cannot establish this.
    pub unsafe fn with_make_gear_decoder(
        mut self,
        decode: fn(usize) -> Result<CheckedGear, E>,
    ) -> Self {
        self.gear_decoder = Some(decode);
        self.gear_driver = decoders::linked_gear;
        self
    }
    /// Install the exact configuration projection emitted by the same Make.
    /// # Safety
    /// Every result must equal the Gear ID and complete configuration in this
    /// independently pinned image. This must never come from received input.
    pub unsafe fn with_make_configuration_decoder(
        mut self,
        decode: fn(usize) -> Result<IndexedGearConfiguration, E>,
    ) -> Self {
        self.configuration_decoder = Some(decode);
        self.configuration_driver = decoders::linked_configuration;
        self
    }
    /// Install complete immutable planning facts emitted by the same Make.
    /// # Safety
    /// All fields, ordering and origins must exactly match this independently
    /// pinned image. A received callback or claimed IDs cannot establish this.
    pub unsafe fn with_make_planning_facts(
        mut self,
        facts: fn() -> Result<crate::LinkedPlanningFactsRef<'static>, E>,
    ) -> Self {
        self.planning_facts = Some(facts);
        self
    }
    /// Borrow complete linked facts, or explicitly refuse an unsupported image.
    /// This description does not verify a Plan or authorize effects.
    pub fn linked_planning_facts(&self) -> Result<crate::LinkedPlanningFactsRef<'static>, E> {
        self.planning_facts.ok_or(E::UnsupportedGraph)?()
    }
    /// Read a Gear's exact identity from the independently retained image,
    /// skipping configuration and contracts. This is meaning, not readiness or
    /// permission; complete configuration has its separate checked projection.
    pub fn gear_identity(
        &self,
        index: usize,
    ) -> Result<(conduit_core::GearId, conduit_core::KindId), E> {
        #[derive(serde::Deserialize)]
        struct Identity {
            gear_id: conduit_core::GearId,
            kind_id: conduit_core::KindId,
        }
        if index >= self.count {
            return Err(E::Representation);
        }
        let start = word(self.bytes, PREFIX + index * 4)?;
        let end = word(self.bytes, PREFIX + (index + 1) * 4)?;
        let value: Identity = serde_json::from_slice(
            &self.bytes[self.payload_start + start..self.payload_start + end],
        )
        .map_err(|_| E::Representation)?;
        Ok((value.gear_id, value.kind_id))
    }
    pub fn gear_count(&self) -> usize {
        self.count
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
    /// Visit exact checked Cords sequentially without retaining expansion
    /// provenance or a Cord vector. Root has already authenticated this Make
    /// image; callbacks are inspection only and may run before a later decode
    /// refusal. At most one complete Cord is live during this visit.
    pub fn visit_connections(
        &self,
        mut visit: impl FnMut(&crate::CheckedConnection),
    ) -> Result<usize, E> {
        connection_visit::visit(
            &self.bytes[self.metadata_start..self.payload_start],
            self.count,
            &mut visit,
        )
    }
    pub fn planning_metadata(&self) -> Result<IndexedPlanningMetadata, E> {
        let mut metadata: PlanningMetadataWire =
            serde_json::from_slice(&self.bytes[self.metadata_start..self.payload_start])
                .map_err(|_| E::Representation)?;
        if metadata.version != 1
            || !metadata.gears.is_empty()
            || metadata.connections.len() > 64
            || metadata.provenance.len() != self.count
        {
            return Err(E::UnsupportedGraph);
        }
        metadata.connections.shrink_to_fit();
        metadata.provenance.shrink_to_fit();
        Ok(IndexedPlanningMetadata(metadata))
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

mod connection_visit {
    use super::*;
    use core::fmt;
    use serde::de::{self, DeserializeSeed, IgnoredAny, MapAccess, SeqAccess, Visitor};
    #[derive(serde::Deserialize)]
    #[serde(field_identifier, rename_all = "snake_case")]
    enum Field {
        Version,
        Gears,
        Connections,
        Provenance,
        #[serde(other)]
        Other,
    }
    struct CountSeed;
    impl<'de> DeserializeSeed<'de> for CountSeed {
        type Value = usize;
        fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<usize, D::Error> {
            struct Count;
            impl<'de> Visitor<'de> for Count {
                type Value = usize;
                fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                    f.write_str("finite metadata array")
                }
                fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<usize, A::Error> {
                    let mut count = 0;
                    while seq.next_element::<IgnoredAny>()?.is_some() {
                        count += 1;
                        if count > MAXIMUM_GEARS {
                            return Err(de::Error::custom("metadata array capacity"));
                        }
                    }
                    Ok(count)
                }
            }
            d.deserialize_seq(Count)
        }
    }
    struct Cords<'a, F>(&'a mut F);
    impl<'de, F: FnMut(&crate::CheckedConnection)> DeserializeSeed<'de> for Cords<'_, F> {
        type Value = usize;
        fn deserialize<D: de::Deserializer<'de>>(self, d: D) -> Result<usize, D::Error> {
            d.deserialize_seq(self)
        }
    }
    impl<'de, F: FnMut(&crate::CheckedConnection)> Visitor<'de> for Cords<'_, F> {
        type Value = usize;
        fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
            f.write_str("finite checked Cords")
        }
        fn visit_seq<A: SeqAccess<'de>>(self, mut seq: A) -> Result<usize, A::Error> {
            let mut count = 0;
            while let Some(cord) = seq.next_element::<crate::CheckedConnection>()? {
                count += 1;
                if count > 64 {
                    return Err(de::Error::custom("Cord capacity"));
                }
                (self.0)(&cord);
            }
            Ok(count)
        }
    }
    pub(super) fn visit(
        bytes: &[u8],
        gears: usize,
        callback: &mut impl FnMut(&crate::CheckedConnection),
    ) -> Result<usize, E> {
        struct Metadata<'a, F> {
            gears: usize,
            callback: &'a mut F,
        }
        impl<'de, F: FnMut(&crate::CheckedConnection)> Visitor<'de> for Metadata<'_, F> {
            type Value = usize;
            fn expecting(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
                f.write_str("checked indexed metadata")
            }
            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<usize, A::Error> {
                let (mut version, mut empty, mut cords, mut origins) = (None, None, None, None);
                while let Some(field) = map.next_key::<Field>()? {
                    match field {
                        Field::Version if version.is_none() => {
                            version = Some(map.next_value::<u8>()?)
                        }
                        Field::Gears if empty.is_none() => {
                            empty = Some(map.next_value_seed(CountSeed)?)
                        }
                        Field::Connections if cords.is_none() => {
                            cords = Some(map.next_value_seed(Cords(self.callback))?)
                        }
                        Field::Provenance if origins.is_none() => {
                            origins = Some(map.next_value_seed(CountSeed)?)
                        }
                        Field::Other => {
                            map.next_value::<IgnoredAny>()?;
                        }
                        _ => return Err(de::Error::custom("duplicate indexed metadata field")),
                    }
                }
                if version != Some(1) || empty != Some(0) || origins != Some(self.gears) {
                    return Err(de::Error::custom("indexed metadata scope"));
                }
                cords.ok_or_else(|| de::Error::custom("missing checked Cords"))
            }
        }
        let mut decoder = serde_json::Deserializer::from_slice(bytes);
        let count =
            serde::Deserializer::deserialize_map(&mut decoder, Metadata { gears, callback })
                .map_err(|_| E::Representation)?;
        decoder.end().map_err(|_| E::Representation)?;
        Ok(count)
    }
}
