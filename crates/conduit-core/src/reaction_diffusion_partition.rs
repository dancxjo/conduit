//! Finite realization-only partition and directed boundary truth.

use alloc::vec;
use alloc::vec::Vec;

use crate::{
    GrayScottParameters, ReactionDiffusionCell, ReactionDiffusionFieldId,
    ReactionDiffusionFieldState, ReactionDiffusionRefusal, REACTION_DIFFUSION_MAXIMUM_BOUNDARIES,
    REACTION_DIFFUSION_MAXIMUM_CELLS, REACTION_DIFFUSION_MAXIMUM_EXTENT,
    REACTION_DIFFUSION_MAXIMUM_REGIONS, REACTION_DIFFUSION_MINIMUM_EXTENT,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct ReactionDiffusionRegionId(pub u16);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionRegion {
    pub region_id: ReactionDiffusionRegionId,
    pub origin_x: u16,
    pub origin_y: u16,
    pub width: u16,
    pub height: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionPartition {
    pub regions: Vec<ReactionDiffusionRegion>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum ReactionDiffusionBoundaryEdge {
    North,
    South,
    West,
    East,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionBoundaryState {
    pub boundary_id: u32,
    pub field_id: ReactionDiffusionFieldId,
    pub generation: u64,
    pub source_region: ReactionDiffusionRegionId,
    pub destination_region: ReactionDiffusionRegionId,
    pub destination_edge: ReactionDiffusionBoundaryEdge,
    pub destination_offset: u16,
    pub values: Vec<ReactionDiffusionCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionRegionState {
    pub region: ReactionDiffusionRegion,
    pub cells: Vec<ReactionDiffusionCell>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PartitionedReactionDiffusionGeneration {
    pub field_id: ReactionDiffusionFieldId,
    pub generation: u64,
    pub width: u16,
    pub height: u16,
    pub parameters: GrayScottParameters,
    pub partition: ReactionDiffusionPartition,
    pub regions: Vec<ReactionDiffusionRegionState>,
    pub boundaries: Vec<ReactionDiffusionBoundaryState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReactionDiffusionPartitionRefusal {
    InvalidRegionCount,
    DuplicateRegionIdentity,
    ZeroExtent,
    RegionOutOfRange,
    OverlappingRegions,
    IncompleteCoverage,
    RegionStateMismatch,
    ExcessBoundaryTruth,
    MissingBoundaryTruth,
    DuplicateBoundaryIdentity,
    DuplicateBoundaryTruth,
    StaleBoundaryGeneration,
    WrongBoundaryField,
    WrongBoundaryProfile,
    WrongBoundaryEdge,
    WrongBoundarySource,
    WrongBoundaryDestination,
    WrongBoundaryValue,
    UnknownRegionIdentity,
    DuplicateRegionResult,
    MissingRegionResult,
    WrongRegionResult,
    MalformedBoundaryLength,
    GenerationOverflow,
    Field(ReactionDiffusionRefusal),
}

impl From<ReactionDiffusionRefusal> for ReactionDiffusionPartitionRefusal {
    fn from(value: ReactionDiffusionRefusal) -> Self {
        Self::Field(value)
    }
}

impl ReactionDiffusionPartition {
    pub fn validate(
        &self,
        width: u16,
        height: u16,
    ) -> Result<(), ReactionDiffusionPartitionRefusal> {
        owner_map(self, width, height).map(|_| ())
    }
}

pub fn partition_reaction_diffusion_generation(
    state: &ReactionDiffusionFieldState,
    partition: ReactionDiffusionPartition,
) -> Result<PartitionedReactionDiffusionGeneration, ReactionDiffusionPartitionRefusal> {
    state.validate()?;
    let owners = owner_map(&partition, state.width, state.height)?;
    let mut regions = Vec::with_capacity(partition.regions.len());
    for region in &partition.regions {
        let mut cells = Vec::with_capacity(usize::from(region.width) * usize::from(region.height));
        for y in region.origin_y..region.origin_y + region.height {
            for x in region.origin_x..region.origin_x + region.width {
                cells.push(
                    state.cells()[usize::from(y) * usize::from(state.width) + usize::from(x)],
                );
            }
        }
        regions.push(ReactionDiffusionRegionState {
            region: *region,
            cells,
        });
    }
    let boundaries = derive_boundaries(state, &partition, &owners)?;
    let generation = PartitionedReactionDiffusionGeneration {
        field_id: state.field_id,
        generation: state.generation,
        width: state.width,
        height: state.height,
        parameters: state.parameters,
        partition,
        regions,
        boundaries,
    };
    generation.validate()?;
    Ok(generation)
}

impl PartitionedReactionDiffusionGeneration {
    pub fn validate(&self) -> Result<(), ReactionDiffusionPartitionRefusal> {
        let owners = owner_map(&self.partition, self.width, self.height)?;
        if self.regions.len() != self.partition.regions.len() {
            return Err(ReactionDiffusionPartitionRefusal::RegionStateMismatch);
        }
        for expected in &self.partition.regions {
            let Some(actual) = self
                .regions
                .iter()
                .find(|state| state.region.region_id == expected.region_id)
            else {
                return Err(ReactionDiffusionPartitionRefusal::RegionStateMismatch);
            };
            if actual.region != *expected
                || actual.cells.len() != usize::from(expected.width) * usize::from(expected.height)
            {
                return Err(ReactionDiffusionPartitionRefusal::RegionStateMismatch);
            }
        }
        let source_cells = crate::reaction_diffusion_partition_join::join_region_cells(self)?;
        ReactionDiffusionFieldState::from_cells(
            self.field_id,
            self.generation,
            self.width,
            self.height,
            self.parameters,
            source_cells.clone(),
        )?;
        validate_boundaries(self, &owners, &source_cells)
    }

    pub fn evolve_and_join(
        &self,
    ) -> Result<ReactionDiffusionFieldState, ReactionDiffusionPartitionRefusal> {
        self.validate()?;
        let next_generation = self
            .generation
            .checked_add(1)
            .ok_or(ReactionDiffusionPartitionRefusal::GenerationOverflow)?;
        let mut joined =
            vec![ReactionDiffusionCell::REST; usize::from(self.width) * usize::from(self.height)];
        for region_state in &self.regions {
            let region = region_state.region;
            for local_y in 0..usize::from(region.height) {
                for local_x in 0..usize::from(region.width) {
                    let local_index = local_y * usize::from(region.width) + local_x;
                    let center = region_state.cells[local_index];
                    let north = neighbor(
                        self,
                        region_state,
                        local_x,
                        local_y,
                        ReactionDiffusionBoundaryEdge::North,
                    )?;
                    let south = neighbor(
                        self,
                        region_state,
                        local_x,
                        local_y,
                        ReactionDiffusionBoundaryEdge::South,
                    )?;
                    let west = neighbor(
                        self,
                        region_state,
                        local_x,
                        local_y,
                        ReactionDiffusionBoundaryEdge::West,
                    )?;
                    let east = neighbor(
                        self,
                        region_state,
                        local_x,
                        local_y,
                        ReactionDiffusionBoundaryEdge::East,
                    )?;
                    let value = crate::reaction_diffusion_evolution::evolve_cell(
                        center,
                        north,
                        south,
                        west,
                        east,
                        self.parameters,
                    )?;
                    let global_x = usize::from(region.origin_x) + local_x;
                    let global_y = usize::from(region.origin_y) + local_y;
                    joined[global_y * usize::from(self.width) + global_x] = value;
                }
            }
        }
        Ok(ReactionDiffusionFieldState::from_cells(
            self.field_id,
            next_generation,
            self.width,
            self.height,
            self.parameters,
            joined,
        )?)
    }
}

fn owner_map(
    partition: &ReactionDiffusionPartition,
    width: u16,
    height: u16,
) -> Result<Vec<ReactionDiffusionRegionId>, ReactionDiffusionPartitionRefusal> {
    if !(REACTION_DIFFUSION_MINIMUM_EXTENT..=REACTION_DIFFUSION_MAXIMUM_EXTENT).contains(&width)
        || !(REACTION_DIFFUSION_MINIMUM_EXTENT..=REACTION_DIFFUSION_MAXIMUM_EXTENT)
            .contains(&height)
        || usize::from(width) * usize::from(height) > REACTION_DIFFUSION_MAXIMUM_CELLS as usize
    {
        return Err(ReactionDiffusionPartitionRefusal::Field(
            ReactionDiffusionRefusal::InvalidDimensions,
        ));
    }
    if partition.regions.is_empty()
        || partition.regions.len() > usize::from(REACTION_DIFFUSION_MAXIMUM_REGIONS)
    {
        return Err(ReactionDiffusionPartitionRefusal::InvalidRegionCount);
    }
    let mut owners = vec![None; usize::from(width) * usize::from(height)];
    let mut ids = Vec::with_capacity(partition.regions.len());
    for region in &partition.regions {
        if ids.contains(&region.region_id) {
            return Err(ReactionDiffusionPartitionRefusal::DuplicateRegionIdentity);
        }
        ids.push(region.region_id);
        if region.width == 0 || region.height == 0 {
            return Err(ReactionDiffusionPartitionRefusal::ZeroExtent);
        }
        let end_x = region
            .origin_x
            .checked_add(region.width)
            .ok_or(ReactionDiffusionPartitionRefusal::RegionOutOfRange)?;
        let end_y = region
            .origin_y
            .checked_add(region.height)
            .ok_or(ReactionDiffusionPartitionRefusal::RegionOutOfRange)?;
        if end_x > width || end_y > height {
            return Err(ReactionDiffusionPartitionRefusal::RegionOutOfRange);
        }
        for y in region.origin_y..end_y {
            for x in region.origin_x..end_x {
                let slot = &mut owners[usize::from(y) * usize::from(width) + usize::from(x)];
                if slot.replace(region.region_id).is_some() {
                    return Err(ReactionDiffusionPartitionRefusal::OverlappingRegions);
                }
            }
        }
    }
    owners
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or(ReactionDiffusionPartitionRefusal::IncompleteCoverage)
}

fn derive_boundaries(
    state: &ReactionDiffusionFieldState,
    partition: &ReactionDiffusionPartition,
    owners: &[ReactionDiffusionRegionId],
) -> Result<Vec<ReactionDiffusionBoundaryState>, ReactionDiffusionPartitionRefusal> {
    let mut boundaries = Vec::new();
    for destination in &partition.regions {
        for edge in [
            ReactionDiffusionBoundaryEdge::North,
            ReactionDiffusionBoundaryEdge::South,
            ReactionDiffusionBoundaryEdge::West,
            ReactionDiffusionBoundaryEdge::East,
        ] {
            let length = edge_length(*destination, edge);
            for offset in 0..length {
                let (x, y) = boundary_source_coordinate(
                    *destination,
                    edge,
                    offset,
                    state.width,
                    state.height,
                );
                let source_region =
                    owners[usize::from(y) * usize::from(state.width) + usize::from(x)];
                let value =
                    state.cells()[usize::from(y) * usize::from(state.width) + usize::from(x)];
                boundaries.push(ReactionDiffusionBoundaryState {
                    boundary_id: boundaries.len() as u32,
                    field_id: state.field_id,
                    generation: state.generation,
                    source_region,
                    destination_region: destination.region_id,
                    destination_edge: edge,
                    destination_offset: offset,
                    values: vec![value],
                });
            }
        }
    }
    if boundaries.len() > REACTION_DIFFUSION_MAXIMUM_BOUNDARIES as usize {
        return Err(ReactionDiffusionPartitionRefusal::ExcessBoundaryTruth);
    }
    Ok(boundaries)
}

fn validate_boundaries(
    generation: &PartitionedReactionDiffusionGeneration,
    owners: &[ReactionDiffusionRegionId],
    source_cells: &[ReactionDiffusionCell],
) -> Result<(), ReactionDiffusionPartitionRefusal> {
    if generation.boundaries.len() > REACTION_DIFFUSION_MAXIMUM_BOUNDARIES as usize {
        return Err(ReactionDiffusionPartitionRefusal::ExcessBoundaryTruth);
    }
    let expected_count = generation
        .partition
        .regions
        .iter()
        .map(|region| usize::from(2 * (region.width + region.height)))
        .sum::<usize>();
    if generation.boundaries.len() < expected_count {
        return Err(ReactionDiffusionPartitionRefusal::MissingBoundaryTruth);
    }
    if generation.boundaries.len() > expected_count {
        return Err(ReactionDiffusionPartitionRefusal::ExcessBoundaryTruth);
    }
    let mut ids = Vec::with_capacity(generation.boundaries.len());
    let mut keys = Vec::with_capacity(generation.boundaries.len());
    for boundary in &generation.boundaries {
        if ids.contains(&boundary.boundary_id) {
            return Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryIdentity);
        }
        ids.push(boundary.boundary_id);
        if boundary.field_id != generation.field_id {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryField);
        }
        if boundary.generation != generation.generation {
            return Err(ReactionDiffusionPartitionRefusal::StaleBoundaryGeneration);
        }
        let Some(destination) = generation
            .partition
            .regions
            .iter()
            .find(|region| region.region_id == boundary.destination_region)
        else {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryDestination);
        };
        if boundary.values.len() != 1
            || boundary.destination_offset >= edge_length(*destination, boundary.destination_edge)
        {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge);
        }
        let key = (
            boundary.destination_region,
            crate::reaction_diffusion_boundary_codec::edge_tag(boundary.destination_edge),
            boundary.destination_offset,
        );
        if keys.contains(&key) {
            return Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryTruth);
        }
        keys.push(key);
        let (x, y) = boundary_source_coordinate(
            *destination,
            boundary.destination_edge,
            boundary.destination_offset,
            generation.width,
            generation.height,
        );
        let expected_source =
            owners[usize::from(y) * usize::from(generation.width) + usize::from(x)];
        if boundary.source_region != expected_source {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundarySource);
        }
        let expected_value =
            source_cells[usize::from(y) * usize::from(generation.width) + usize::from(x)];
        if boundary.values[0] != expected_value {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryValue);
        }
    }
    Ok(())
}

fn neighbor(
    generation: &PartitionedReactionDiffusionGeneration,
    region_state: &ReactionDiffusionRegionState,
    x: usize,
    y: usize,
    edge: ReactionDiffusionBoundaryEdge,
) -> Result<ReactionDiffusionCell, ReactionDiffusionPartitionRefusal> {
    let width = usize::from(region_state.region.width);
    let height = usize::from(region_state.region.height);
    let local = match edge {
        ReactionDiffusionBoundaryEdge::North if y > 0 => Some((y - 1) * width + x),
        ReactionDiffusionBoundaryEdge::South if y + 1 < height => Some((y + 1) * width + x),
        ReactionDiffusionBoundaryEdge::West if x > 0 => Some(y * width + x - 1),
        ReactionDiffusionBoundaryEdge::East if x + 1 < width => Some(y * width + x + 1),
        _ => None,
    };
    if let Some(index) = local {
        return Ok(region_state.cells[index]);
    }
    let offset = match edge {
        ReactionDiffusionBoundaryEdge::North | ReactionDiffusionBoundaryEdge::South => x as u16,
        ReactionDiffusionBoundaryEdge::West | ReactionDiffusionBoundaryEdge::East => y as u16,
    };
    generation
        .boundaries
        .iter()
        .find(|boundary| {
            boundary.destination_region == region_state.region.region_id
                && boundary.destination_edge == edge
                && boundary.destination_offset == offset
        })
        .map(|boundary| boundary.values[0])
        .ok_or(ReactionDiffusionPartitionRefusal::MissingBoundaryTruth)
}

fn edge_length(region: ReactionDiffusionRegion, edge: ReactionDiffusionBoundaryEdge) -> u16 {
    match edge {
        ReactionDiffusionBoundaryEdge::North | ReactionDiffusionBoundaryEdge::South => region.width,
        ReactionDiffusionBoundaryEdge::West | ReactionDiffusionBoundaryEdge::East => region.height,
    }
}

fn boundary_source_coordinate(
    region: ReactionDiffusionRegion,
    edge: ReactionDiffusionBoundaryEdge,
    offset: u16,
    field_width: u16,
    field_height: u16,
) -> (u16, u16) {
    match edge {
        ReactionDiffusionBoundaryEdge::North => (
            (region.origin_x + offset) % field_width,
            (region.origin_y + field_height - 1) % field_height,
        ),
        ReactionDiffusionBoundaryEdge::South => (
            (region.origin_x + offset) % field_width,
            (region.origin_y + region.height) % field_height,
        ),
        ReactionDiffusionBoundaryEdge::West => (
            (region.origin_x + field_width - 1) % field_width,
            (region.origin_y + offset) % field_height,
        ),
        ReactionDiffusionBoundaryEdge::East => (
            (region.origin_x + region.width) % field_width,
            (region.origin_y + offset) % field_height,
        ),
    }
}
