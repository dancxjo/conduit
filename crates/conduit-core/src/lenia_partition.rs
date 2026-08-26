//! Finite realization-only Lenia region work and deterministic joining.
//!
//! These identities belong to a selected Plan, never to an authored Form.

use alloc::{vec, vec::Vec};

use crate::{LeniaFieldId, LeniaFieldState, LeniaParameters, LeniaRefusal, LENIA_MAXIMUM_CELLS};

pub const LENIA_MAXIMUM_REGIONS: usize = 8;
pub const LENIA_MAXIMUM_KERNEL_SAMPLES: usize =
    (crate::LENIA_MAXIMUM_KERNEL_RADIUS as usize * 2 + 1).pow(2);

#[derive(Debug, Copy, Clone, PartialEq, Eq, PartialOrd, Ord)]
pub struct LeniaRegionId(pub u8);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LeniaRegion {
    pub id: LeniaRegionId,
    pub x: u16,
    pub width: u16,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeniaPartitionRefusal {
    InvalidPartition,
    InvalidRegion,
    WrongField,
    WrongGeneration,
    WrongDimensions,
    WrongHalo,
    DuplicateRegion,
    MissingRegion,
    CellCountMismatch,
    Evolution(LeniaRefusal),
}

impl From<LeniaRefusal> for LeniaPartitionRefusal {
    fn from(value: LeniaRefusal) -> Self {
        Self::Evolution(value)
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeniaPartition {
    field_id: LeniaFieldId,
    generation: u64,
    width: u16,
    height: u16,
    regions: Vec<LeniaRegion>,
}

impl LeniaPartition {
    pub fn vertical(
        field: &LeniaFieldState,
        region_widths: &[u16],
    ) -> Result<Self, LeniaPartitionRefusal> {
        if region_widths.len() < 2 || region_widths.len() > LENIA_MAXIMUM_REGIONS {
            return Err(LeniaPartitionRefusal::InvalidPartition);
        }
        let mut x = 0_u16;
        let mut regions = Vec::with_capacity(region_widths.len());
        for (index, width) in region_widths.iter().copied().enumerate() {
            if width == 0 {
                return Err(LeniaPartitionRefusal::InvalidPartition);
            }
            regions.push(LeniaRegion {
                id: LeniaRegionId(index as u8),
                x,
                width,
            });
            x = x
                .checked_add(width)
                .ok_or(LeniaPartitionRefusal::InvalidPartition)?;
        }
        if x != field.width {
            return Err(LeniaPartitionRefusal::InvalidPartition);
        }
        Ok(Self {
            field_id: field.field_id,
            generation: field.generation,
            width: field.width,
            height: field.height,
            regions,
        })
    }

    pub fn regions(&self) -> &[LeniaRegion] {
        &self.regions
    }

    pub fn prepare_region(
        &self,
        field: &LeniaFieldState,
        region_id: LeniaRegionId,
        parameters: LeniaParameters,
    ) -> Result<LeniaRegionWork, LeniaPartitionRefusal> {
        self.validate_field(field)?;
        parameters.validate()?;
        let region = self
            .regions
            .iter()
            .copied()
            .find(|candidate| candidate.id == region_id)
            .ok_or(LeniaPartitionRefusal::InvalidRegion)?;
        let halo = parameters.kernel_radius;
        let expanded_width = usize::from(region.width + halo * 2);
        let expanded_height = usize::from(self.height + halo * 2);
        let mut expanded = Vec::with_capacity(expanded_width * expanded_height);
        for expanded_y in 0..expanded_height {
            let source_y =
                (expanded_y as i32 - i32::from(halo)).rem_euclid(i32::from(self.height)) as usize;
            for expanded_x in 0..expanded_width {
                let source_x = (i32::from(region.x) + expanded_x as i32 - i32::from(halo))
                    .rem_euclid(i32::from(self.width)) as usize;
                expanded.push(field.cells()[source_y * usize::from(self.width) + source_x]);
            }
        }
        Ok(LeniaRegionWork {
            field_id: self.field_id,
            generation: self.generation,
            field_width: self.width,
            field_height: self.height,
            region,
            halo,
            expanded,
        })
    }

    pub fn join(
        &self,
        results: &[LeniaRegionResult],
    ) -> Result<LeniaFieldState, LeniaPartitionRefusal> {
        if results.len() != self.regions.len() {
            return Err(LeniaPartitionRefusal::MissingRegion);
        }
        let output_generation = self
            .generation
            .checked_add(1)
            .ok_or(LeniaPartitionRefusal::WrongGeneration)?;
        let mut seen = Vec::with_capacity(results.len());
        let mut cells = vec![0; usize::from(self.width) * usize::from(self.height)];
        for result in results {
            if result.field_id != self.field_id {
                return Err(LeniaPartitionRefusal::WrongField);
            }
            if result.generation != output_generation {
                return Err(LeniaPartitionRefusal::WrongGeneration);
            }
            if result.field_width != self.width || result.field_height != self.height {
                return Err(LeniaPartitionRefusal::WrongDimensions);
            }
            let region = self
                .regions
                .iter()
                .find(|region| region.id == result.region.id)
                .ok_or(LeniaPartitionRefusal::InvalidRegion)?;
            if region != &result.region {
                return Err(LeniaPartitionRefusal::InvalidRegion);
            }
            if seen.contains(&region.id) {
                return Err(LeniaPartitionRefusal::DuplicateRegion);
            }
            seen.push(region.id);
            if result.cells.len() != usize::from(region.width) * usize::from(self.height) {
                return Err(LeniaPartitionRefusal::CellCountMismatch);
            }
            for y in 0..usize::from(self.height) {
                let source = y * usize::from(region.width);
                let destination = y * usize::from(self.width) + usize::from(region.x);
                cells[destination..destination + usize::from(region.width)]
                    .copy_from_slice(&result.cells[source..source + usize::from(region.width)]);
            }
        }
        if self.regions.iter().any(|region| !seen.contains(&region.id)) {
            return Err(LeniaPartitionRefusal::MissingRegion);
        }
        LeniaFieldState::from_cells(
            self.field_id,
            output_generation,
            self.width,
            self.height,
            cells,
        )
        .map_err(Into::into)
    }

    fn validate_field(&self, field: &LeniaFieldState) -> Result<(), LeniaPartitionRefusal> {
        if field.field_id != self.field_id {
            return Err(LeniaPartitionRefusal::WrongField);
        }
        if field.generation != self.generation {
            return Err(LeniaPartitionRefusal::WrongGeneration);
        }
        if field.width != self.width || field.height != self.height {
            return Err(LeniaPartitionRefusal::WrongDimensions);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeniaRegionWork {
    pub field_id: LeniaFieldId,
    pub generation: u64,
    pub field_width: u16,
    pub field_height: u16,
    pub region: LeniaRegion,
    pub halo: u16,
    expanded: Vec<u32>,
}

impl LeniaRegionWork {
    pub fn expanded_cells(&self) -> &[u32] {
        &self.expanded
    }

    pub fn view(&self) -> LeniaRegionWorkView<'_> {
        LeniaRegionWorkView {
            field_id: self.field_id,
            generation: self.generation,
            field_width: self.field_width,
            field_height: self.field_height,
            region: self.region,
            halo: self.halo,
            expanded: &self.expanded,
        }
    }

    pub fn evolve(
        &self,
        parameters: LeniaParameters,
    ) -> Result<LeniaRegionResult, LeniaPartitionRefusal> {
        let mut kernel = LeniaRegionKernel::new();
        kernel.prepare(parameters)?;
        let mut cells = vec![0; usize::from(self.region.width) * usize::from(self.field_height)];
        kernel.evolve_into(self.view(), &mut cells)?;
        Ok(LeniaRegionResult {
            field_id: self.field_id,
            generation: self
                .generation
                .checked_add(1)
                .ok_or(LeniaPartitionRefusal::WrongGeneration)?,
            field_width: self.field_width,
            field_height: self.field_height,
            region: self.region,
            cells,
        })
    }
}

#[derive(Debug, Copy, Clone)]
pub struct LeniaRegionWorkView<'a> {
    pub field_id: LeniaFieldId,
    pub generation: u64,
    pub field_width: u16,
    pub field_height: u16,
    pub region: LeniaRegion,
    pub halo: u16,
    expanded: &'a [u32],
}

impl<'a> LeniaRegionWorkView<'a> {
    pub fn from_expanded(
        field_id: LeniaFieldId,
        generation: u64,
        field_width: u16,
        field_height: u16,
        region: LeniaRegion,
        halo: u16,
        expanded: &'a [u32],
    ) -> Result<Self, LeniaPartitionRefusal> {
        if region.width == 0
            || region.x.checked_add(region.width).is_none()
            || region.x + region.width > field_width
            || halo == 0
        {
            return Err(LeniaPartitionRefusal::InvalidRegion);
        }
        let halo_extent = halo
            .checked_mul(2)
            .ok_or(LeniaPartitionRefusal::CellCountMismatch)?;
        let expanded_width = region
            .width
            .checked_add(halo_extent)
            .ok_or(LeniaPartitionRefusal::CellCountMismatch)?;
        let expanded_height = field_height
            .checked_add(halo_extent)
            .ok_or(LeniaPartitionRefusal::CellCountMismatch)?;
        let expected = usize::from(expanded_width) * usize::from(expanded_height);
        if expanded.len() != expected {
            return Err(LeniaPartitionRefusal::CellCountMismatch);
        }
        Ok(Self {
            field_id,
            generation,
            field_width,
            field_height,
            region,
            halo,
            expanded,
        })
    }

    pub fn expanded_cells(self) -> &'a [u32] {
        self.expanded
    }
}

/// Fixed-capacity kernel preparation suitable for constrained Hosts without a
/// runtime allocator. The owning Plan admits this storage before Play start.
pub struct LeniaRegionKernel {
    parameters: Option<LeniaParameters>,
    samples: [crate::lenia_evolution::KernelSample; LENIA_MAXIMUM_KERNEL_SAMPLES],
    sample_count: usize,
    weight: u64,
}

impl LeniaRegionKernel {
    pub const fn new() -> Self {
        Self {
            parameters: None,
            samples: [crate::lenia_evolution::KernelSample::EMPTY; LENIA_MAXIMUM_KERNEL_SAMPLES],
            sample_count: 0,
            weight: 0,
        }
    }

    pub fn prepare(&mut self, parameters: LeniaParameters) -> Result<(), LeniaPartitionRefusal> {
        parameters.validate()?;
        let (sample_count, weight) =
            crate::lenia_evolution::build_kernel_into(parameters, &mut self.samples)?;
        self.parameters = Some(parameters);
        self.sample_count = sample_count;
        self.weight = weight;
        Ok(())
    }

    pub fn evolve_into(
        &self,
        work: LeniaRegionWorkView<'_>,
        output: &mut [u32],
    ) -> Result<(), LeniaPartitionRefusal> {
        let parameters = self.parameters.ok_or(LeniaPartitionRefusal::Evolution(
            LeniaRefusal::Uninitialized,
        ))?;
        if work.halo != parameters.kernel_radius {
            return Err(LeniaPartitionRefusal::WrongHalo);
        }
        let expected = usize::from(work.region.width + work.halo * 2)
            * usize::from(work.field_height + work.halo * 2);
        if work.expanded.len() != expected
            || usize::from(work.field_width) * usize::from(work.field_height)
                > LENIA_MAXIMUM_CELLS as usize
        {
            return Err(LeniaPartitionRefusal::CellCountMismatch);
        }
        crate::lenia_evolution::evolve_region_generation(
            work.expanded,
            output,
            crate::lenia_evolution::RegionDimensions {
                width: usize::from(work.region.width),
                height: usize::from(work.field_height),
                halo: usize::from(work.halo),
            },
            parameters,
            &self.samples[..self.sample_count],
            self.weight,
        )?;
        Ok(())
    }
}

impl Default for LeniaRegionKernel {
    fn default() -> Self {
        Self::new()
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeniaRegionResult {
    pub field_id: LeniaFieldId,
    pub generation: u64,
    pub field_width: u16,
    pub field_height: u16,
    pub region: LeniaRegion,
    cells: Vec<u32>,
}

impl LeniaRegionResult {
    pub fn from_cells(
        field_id: LeniaFieldId,
        generation: u64,
        field_width: u16,
        field_height: u16,
        region: LeniaRegion,
        cells: Vec<u32>,
    ) -> Result<Self, LeniaPartitionRefusal> {
        if field_width == 0
            || field_height == 0
            || region.width == 0
            || u32::from(region.x) + u32::from(region.width) > u32::from(field_width)
        {
            return Err(LeniaPartitionRefusal::InvalidRegion);
        }
        if cells.len() != usize::from(region.width) * usize::from(field_height) {
            return Err(LeniaPartitionRefusal::CellCountMismatch);
        }
        if cells.iter().any(|cell| *cell > crate::LENIA_Q16_ONE) {
            return Err(LeniaPartitionRefusal::Evolution(
                LeniaRefusal::CellOutOfRange,
            ));
        }
        Ok(Self {
            field_id,
            generation,
            field_width,
            field_height,
            region,
            cells,
        })
    }

    pub fn cells(&self) -> &[u32] {
        &self.cells
    }
}
