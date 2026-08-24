//! Host-local reaction-diffusion work and independently evolved region truth.

use alloc::{vec, vec::Vec};

use crate::{
    GrayScottParameters, PartitionedReactionDiffusionGeneration, ReactionDiffusionBoundaryEdge,
    ReactionDiffusionBoundaryState, ReactionDiffusionCell, ReactionDiffusionFieldId,
    ReactionDiffusionFieldState, ReactionDiffusionPartition, ReactionDiffusionPartitionRefusal,
    ReactionDiffusionRegion, ReactionDiffusionRegionId, REACTION_DIFFUSION_MAXIMUM_BOUNDARIES,
    REACTION_DIFFUSION_MAXIMUM_CELLS, REACTION_DIFFUSION_MAXIMUM_EXTENT,
    REACTION_DIFFUSION_MINIMUM_EXTENT,
};

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionBoundaryRequirement {
    pub boundary_id: u32,
    pub source_region: ReactionDiffusionRegionId,
    pub destination_region: ReactionDiffusionRegionId,
    pub destination_edge: ReactionDiffusionBoundaryEdge,
    pub destination_offset: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionRegionWorkContract {
    pub field_id: ReactionDiffusionFieldId,
    pub generation: u64,
    pub field_width: u16,
    pub field_height: u16,
    pub parameters: GrayScottParameters,
    pub region: ReactionDiffusionRegion,
    pub required_boundaries: Vec<ReactionDiffusionBoundaryRequirement>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionRegionWork {
    contract: ReactionDiffusionRegionWorkContract,
    cells: Vec<ReactionDiffusionCell>,
    boundaries: Vec<ReactionDiffusionBoundaryState>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EvolvedReactionDiffusionRegion {
    pub field_id: ReactionDiffusionFieldId,
    pub source_generation: u64,
    pub generation: u64,
    pub field_width: u16,
    pub field_height: u16,
    pub parameters: GrayScottParameters,
    pub region: ReactionDiffusionRegion,
    pub cells: Vec<ReactionDiffusionCell>,
}

impl PartitionedReactionDiffusionGeneration {
    pub fn region_work_basis(
        &self,
        region_id: ReactionDiffusionRegionId,
    ) -> Result<
        (
            ReactionDiffusionRegionWorkContract,
            Vec<ReactionDiffusionCell>,
        ),
        ReactionDiffusionPartitionRefusal,
    > {
        self.validate()?;
        let state = self
            .regions
            .iter()
            .find(|state| state.region.region_id == region_id)
            .ok_or(ReactionDiffusionPartitionRefusal::UnknownRegionIdentity)?;
        let required_boundaries = self
            .boundaries
            .iter()
            .filter(|boundary| boundary.destination_region == region_id)
            .map(|boundary| ReactionDiffusionBoundaryRequirement {
                boundary_id: boundary.boundary_id,
                source_region: boundary.source_region,
                destination_region: boundary.destination_region,
                destination_edge: boundary.destination_edge,
                destination_offset: boundary.destination_offset,
            })
            .collect();
        Ok((
            ReactionDiffusionRegionWorkContract {
                field_id: self.field_id,
                generation: self.generation,
                field_width: self.width,
                field_height: self.height,
                parameters: self.parameters,
                region: state.region,
                required_boundaries,
            },
            state.cells.clone(),
        ))
    }
}

impl ReactionDiffusionRegionWork {
    pub fn new(
        contract: ReactionDiffusionRegionWorkContract,
        cells: Vec<ReactionDiffusionCell>,
    ) -> Result<Self, ReactionDiffusionPartitionRefusal> {
        let work = Self {
            contract,
            cells,
            boundaries: Vec::new(),
        };
        work.validate_partial()?;
        Ok(work)
    }

    pub fn contract(&self) -> &ReactionDiffusionRegionWorkContract {
        &self.contract
    }

    pub fn cells(&self) -> &[ReactionDiffusionCell] {
        &self.cells
    }

    pub fn boundaries(&self) -> &[ReactionDiffusionBoundaryState] {
        &self.boundaries
    }

    pub fn admit_boundary(
        &mut self,
        boundary: ReactionDiffusionBoundaryState,
    ) -> Result<(), ReactionDiffusionPartitionRefusal> {
        self.validate_boundary(&boundary)?;
        if self
            .boundaries
            .iter()
            .any(|existing| existing.boundary_id == boundary.boundary_id)
        {
            return Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryIdentity);
        }
        self.boundaries.push(boundary);
        Ok(())
    }

    pub fn evolve(
        self,
    ) -> Result<EvolvedReactionDiffusionRegion, ReactionDiffusionPartitionRefusal> {
        self.validate_complete()?;
        let next_generation = self
            .contract
            .generation
            .checked_add(1)
            .ok_or(ReactionDiffusionPartitionRefusal::GenerationOverflow)?;
        let width = usize::from(self.contract.region.width);
        let height = usize::from(self.contract.region.height);
        let mut next = vec![ReactionDiffusionCell::REST; self.cells.len()];
        for y in 0..height {
            for x in 0..width {
                let center = self.cells[y * width + x];
                let north = self.neighbor(x, y, ReactionDiffusionBoundaryEdge::North)?;
                let south = self.neighbor(x, y, ReactionDiffusionBoundaryEdge::South)?;
                let west = self.neighbor(x, y, ReactionDiffusionBoundaryEdge::West)?;
                let east = self.neighbor(x, y, ReactionDiffusionBoundaryEdge::East)?;
                next[y * width + x] = crate::reaction_diffusion_evolution::evolve_cell(
                    center,
                    north,
                    south,
                    west,
                    east,
                    self.contract.parameters,
                )?;
            }
        }
        Ok(EvolvedReactionDiffusionRegion {
            field_id: self.contract.field_id,
            source_generation: self.contract.generation,
            generation: next_generation,
            field_width: self.contract.field_width,
            field_height: self.contract.field_height,
            parameters: self.contract.parameters,
            region: self.contract.region,
            cells: next,
        })
    }

    fn validate_partial(&self) -> Result<(), ReactionDiffusionPartitionRefusal> {
        let region = self.contract.region;
        if self.cells.len() != usize::from(region.width) * usize::from(region.height) {
            return Err(ReactionDiffusionPartitionRefusal::RegionStateMismatch);
        }
        if self.contract.required_boundaries.len() > REACTION_DIFFUSION_MAXIMUM_BOUNDARIES as usize
        {
            return Err(ReactionDiffusionPartitionRefusal::ExcessBoundaryTruth);
        }
        if !(REACTION_DIFFUSION_MINIMUM_EXTENT..=REACTION_DIFFUSION_MAXIMUM_EXTENT)
            .contains(&self.contract.field_width)
            || !(REACTION_DIFFUSION_MINIMUM_EXTENT..=REACTION_DIFFUSION_MAXIMUM_EXTENT)
                .contains(&self.contract.field_height)
            || usize::from(self.contract.field_width) * usize::from(self.contract.field_height)
                > REACTION_DIFFUSION_MAXIMUM_CELLS as usize
        {
            return Err(ReactionDiffusionPartitionRefusal::Field(
                crate::ReactionDiffusionRefusal::InvalidDimensions,
            ));
        }
        self.contract.parameters.validate()?;
        ReactionDiffusionPartition {
            regions: vec![region],
        }
        .validate_region_within(self.contract.field_width, self.contract.field_height)?;
        let expected = usize::from(2 * (region.width + region.height));
        if self.contract.required_boundaries.len() < expected {
            return Err(ReactionDiffusionPartitionRefusal::MissingBoundaryTruth);
        }
        if self.contract.required_boundaries.len() > expected {
            return Err(ReactionDiffusionPartitionRefusal::ExcessBoundaryTruth);
        }
        let mut ids = Vec::with_capacity(expected);
        let mut keys = Vec::with_capacity(expected);
        for required in &self.contract.required_boundaries {
            if ids.contains(&required.boundary_id) {
                return Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryIdentity);
            }
            ids.push(required.boundary_id);
            if required.destination_region != region.region_id {
                return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryDestination);
            }
            let edge_length = match required.destination_edge {
                ReactionDiffusionBoundaryEdge::North | ReactionDiffusionBoundaryEdge::South => {
                    region.width
                }
                ReactionDiffusionBoundaryEdge::West | ReactionDiffusionBoundaryEdge::East => {
                    region.height
                }
            };
            if required.destination_offset >= edge_length {
                return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge);
            }
            let key = (required.destination_edge, required.destination_offset);
            if keys.contains(&key) {
                return Err(ReactionDiffusionPartitionRefusal::DuplicateBoundaryTruth);
            }
            keys.push(key);
        }
        for cell in &self.cells {
            ReactionDiffusionCell::new(cell.u_ppm, cell.v_ppm)
                .map_err(ReactionDiffusionPartitionRefusal::Field)?;
        }
        Ok(())
    }

    fn validate_complete(&self) -> Result<(), ReactionDiffusionPartitionRefusal> {
        self.validate_partial()?;
        if self.boundaries.len() < self.contract.required_boundaries.len() {
            return Err(ReactionDiffusionPartitionRefusal::MissingBoundaryTruth);
        }
        if self.boundaries.len() > self.contract.required_boundaries.len() {
            return Err(ReactionDiffusionPartitionRefusal::ExcessBoundaryTruth);
        }
        for boundary in &self.boundaries {
            self.validate_boundary(boundary)?;
        }
        Ok(())
    }

    fn validate_boundary(
        &self,
        boundary: &ReactionDiffusionBoundaryState,
    ) -> Result<(), ReactionDiffusionPartitionRefusal> {
        if boundary.field_id != self.contract.field_id {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryField);
        }
        if boundary.generation != self.contract.generation {
            return Err(ReactionDiffusionPartitionRefusal::StaleBoundaryGeneration);
        }
        if boundary.destination_region != self.contract.region.region_id {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryDestination);
        }
        if boundary.values.len() != 1 {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge);
        }
        let Some(requirement) = self
            .contract
            .required_boundaries
            .iter()
            .find(|required| required.boundary_id == boundary.boundary_id)
        else {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge);
        };
        if requirement.source_region != boundary.source_region {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundarySource);
        }
        if requirement.destination_region != boundary.destination_region {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryDestination);
        }
        if requirement.destination_edge != boundary.destination_edge
            || requirement.destination_offset != boundary.destination_offset
        {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge);
        }
        ReactionDiffusionCell::new(boundary.values[0].u_ppm, boundary.values[0].v_ppm)
            .map_err(ReactionDiffusionPartitionRefusal::Field)?;
        Ok(())
    }

    fn neighbor(
        &self,
        x: usize,
        y: usize,
        edge: ReactionDiffusionBoundaryEdge,
    ) -> Result<ReactionDiffusionCell, ReactionDiffusionPartitionRefusal> {
        let width = usize::from(self.contract.region.width);
        let height = usize::from(self.contract.region.height);
        let local = match edge {
            ReactionDiffusionBoundaryEdge::North if y > 0 => Some((y - 1) * width + x),
            ReactionDiffusionBoundaryEdge::South if y + 1 < height => Some((y + 1) * width + x),
            ReactionDiffusionBoundaryEdge::West if x > 0 => Some(y * width + x - 1),
            ReactionDiffusionBoundaryEdge::East if x + 1 < width => Some(y * width + x + 1),
            _ => None,
        };
        if let Some(index) = local {
            return Ok(self.cells[index]);
        }
        let offset = match edge {
            ReactionDiffusionBoundaryEdge::North | ReactionDiffusionBoundaryEdge::South => x as u16,
            ReactionDiffusionBoundaryEdge::West | ReactionDiffusionBoundaryEdge::East => y as u16,
        };
        self.boundaries
            .iter()
            .find(|boundary| {
                boundary.destination_edge == edge && boundary.destination_offset == offset
            })
            .map(|boundary| boundary.values[0])
            .ok_or(ReactionDiffusionPartitionRefusal::MissingBoundaryTruth)
    }
}

impl ReactionDiffusionPartition {
    fn validate_region_within(
        &self,
        width: u16,
        height: u16,
    ) -> Result<(), ReactionDiffusionPartitionRefusal> {
        let region = self.regions[0];
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
        Ok(())
    }
}

pub fn join_evolved_reaction_diffusion_regions(
    field_id: ReactionDiffusionFieldId,
    source_generation: u64,
    width: u16,
    height: u16,
    parameters: GrayScottParameters,
    partition: &ReactionDiffusionPartition,
    regions: &[EvolvedReactionDiffusionRegion],
) -> Result<ReactionDiffusionFieldState, ReactionDiffusionPartitionRefusal> {
    partition.validate(width, height)?;
    if regions.len() < partition.regions.len() {
        return Err(ReactionDiffusionPartitionRefusal::MissingRegionResult);
    }
    if regions.len() > partition.regions.len() {
        return Err(ReactionDiffusionPartitionRefusal::DuplicateRegionResult);
    }
    let generation = source_generation
        .checked_add(1)
        .ok_or(ReactionDiffusionPartitionRefusal::GenerationOverflow)?;
    let mut joined = vec![ReactionDiffusionCell::REST; usize::from(width) * usize::from(height)];
    let mut seen = Vec::with_capacity(regions.len());
    for result in regions {
        if seen.contains(&result.region.region_id) {
            return Err(ReactionDiffusionPartitionRefusal::DuplicateRegionResult);
        }
        seen.push(result.region.region_id);
        if result.field_id != field_id
            || result.source_generation != source_generation
            || result.generation != generation
            || result.field_width != width
            || result.field_height != height
            || result.parameters != parameters
            || !partition.regions.contains(&result.region)
            || result.cells.len()
                != usize::from(result.region.width) * usize::from(result.region.height)
        {
            return Err(ReactionDiffusionPartitionRefusal::WrongRegionResult);
        }
        for local_y in 0..usize::from(result.region.height) {
            for local_x in 0..usize::from(result.region.width) {
                let global_x = usize::from(result.region.origin_x) + local_x;
                let global_y = usize::from(result.region.origin_y) + local_y;
                joined[global_y * usize::from(width) + global_x] =
                    result.cells[local_y * usize::from(result.region.width) + local_x];
            }
        }
    }
    ReactionDiffusionFieldState::from_cells(field_id, generation, width, height, parameters, joined)
        .map_err(ReactionDiffusionPartitionRefusal::Field)
}
