//! Fixed-storage execution of one admitted Lenia region transfer.

use crate::{
    LeniaParameters, LeniaPartitionRefusal, LeniaRegion, LeniaRegionChunkHeader,
    LeniaRegionChunkKind, LeniaRegionChunkRefusal, LeniaRegionChunkView, LeniaRegionId,
    LeniaRegionKernel, LeniaRegionTransferIdentity, LeniaRegionWorkView,
    LENIA_REGION_CHUNK_MAX_CELLS,
};

pub const DISTRIBUTED_LENIA_FIELD_WIDTH: u16 = 32;
pub const DISTRIBUTED_LENIA_FIELD_HEIGHT: u16 = 32;
pub const DISTRIBUTED_LENIA_REGION_WIDTHS: [u16; 3] = [10, 11, 11];
pub const DISTRIBUTED_LENIA_MAXIMUM_WORK_CELLS: usize = (11
    + LeniaParameters::ORBIUM.kernel_radius as usize * 2)
    * (DISTRIBUTED_LENIA_FIELD_HEIGHT as usize
        + LeniaParameters::ORBIUM.kernel_radius as usize * 2);
pub const DISTRIBUTED_LENIA_MAXIMUM_RESULT_CELLS: usize =
    11 * DISTRIBUTED_LENIA_FIELD_HEIGHT as usize;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LeniaWorkerAdmission {
    Progress { admitted_cells: u32 },
    ResultReady,
}

/// One pre-admitted worker slot. It accepts exactly one ordered work transfer,
/// computes exactly one generation, and exposes one immutable result transfer.
pub struct FixedLeniaRegionWorker<
    const WORK_CELLS: usize = DISTRIBUTED_LENIA_MAXIMUM_WORK_CELLS,
    const RESULT_CELLS: usize = DISTRIBUTED_LENIA_MAXIMUM_RESULT_CELLS,
> {
    work_identity: Option<LeniaRegionTransferIdentity>,
    admitted_cells: u32,
    work: [u32; WORK_CELLS],
    result_identity: Option<LeniaRegionTransferIdentity>,
    result_cells: [u32; RESULT_CELLS],
    kernel: LeniaRegionKernel,
}

pub type DistributedLeniaWorker = FixedLeniaRegionWorker<
    DISTRIBUTED_LENIA_MAXIMUM_WORK_CELLS,
    DISTRIBUTED_LENIA_MAXIMUM_RESULT_CELLS,
>;

impl<const WORK_CELLS: usize, const RESULT_CELLS: usize>
    FixedLeniaRegionWorker<WORK_CELLS, RESULT_CELLS>
{
    pub const fn new() -> Self {
        Self {
            work_identity: None,
            admitted_cells: 0,
            work: [0; WORK_CELLS],
            result_identity: None,
            result_cells: [0; RESULT_CELLS],
            kernel: LeniaRegionKernel::new(),
        }
    }

    pub fn prepare(&mut self) -> Result<(), LeniaPartitionRefusal> {
        self.kernel.prepare(LeniaParameters::ORBIUM)
    }

    pub fn admit_encoded(
        &mut self,
        encoded: &[u8],
    ) -> Result<LeniaWorkerAdmission, LeniaRegionChunkRefusal> {
        let chunk = LeniaRegionChunkView::decode(encoded)?;
        self.admit(chunk)
    }

    pub fn admit(
        &mut self,
        chunk: LeniaRegionChunkView<'_>,
    ) -> Result<LeniaWorkerAdmission, LeniaRegionChunkRefusal> {
        if chunk.header.kind != LeniaRegionChunkKind::Work {
            return Err(LeniaRegionChunkRefusal::WrongKind);
        }
        validate_assignment(chunk.header)?;
        let identity = chunk.header.transfer_identity();
        match self.work_identity {
            Some(expected) if expected != identity => {
                return Err(LeniaRegionChunkRefusal::WrongTransfer)
            }
            None => {
                if chunk.header.cell_offset != 0
                    || usize::try_from(identity.total_cells)
                        .map_or(true, |count| count > WORK_CELLS)
                {
                    return Err(LeniaRegionChunkRefusal::InvalidCellRange);
                }
                self.work_identity = Some(identity);
            }
            _ => {}
        }
        if self.result_identity.is_some() {
            return Err(LeniaRegionChunkRefusal::AlreadyComplete);
        }
        if chunk.header.cell_offset != self.admitted_cells {
            return Err(LeniaRegionChunkRefusal::UnexpectedOffset);
        }
        let start = usize::try_from(self.admitted_cells)
            .map_err(|_| LeniaRegionChunkRefusal::InvalidCellRange)?;
        for index in 0..chunk.cell_count() {
            self.work[start + index] = chunk.cell(index)?;
        }
        self.admitted_cells += u32::from(chunk.header.cell_count);
        if self.admitted_cells != identity.total_cells {
            return Ok(LeniaWorkerAdmission::Progress {
                admitted_cells: self.admitted_cells,
            });
        }

        let work_count = usize::try_from(identity.total_cells)
            .map_err(|_| LeniaRegionChunkRefusal::InvalidCellRange)?;
        let view = LeniaRegionWorkView::from_expanded(
            identity.field_id,
            identity.generation,
            identity.field_width,
            identity.field_height,
            identity.region,
            identity.halo,
            &self.work[..work_count],
        )
        .map_err(|_| LeniaRegionChunkRefusal::WrongTransfer)?;
        let result_count = usize::from(identity.region.width) * usize::from(identity.field_height);
        if result_count > RESULT_CELLS {
            return Err(LeniaRegionChunkRefusal::InvalidCellRange);
        }
        self.kernel
            .evolve_into(view, &mut self.result_cells[..result_count])
            .map_err(|_| LeniaRegionChunkRefusal::WrongTransfer)?;
        self.result_identity = Some(LeniaRegionTransferIdentity {
            kind: LeniaRegionChunkKind::Result,
            field_id: identity.field_id,
            generation: identity
                .generation
                .checked_add(1)
                .ok_or(LeniaRegionChunkRefusal::WrongTransfer)?,
            field_width: identity.field_width,
            field_height: identity.field_height,
            region: identity.region,
            halo: 0,
            total_cells: result_count as u32,
        });
        Ok(LeniaWorkerAdmission::ResultReady)
    }

    pub fn result_identity(&self) -> Result<LeniaRegionTransferIdentity, LeniaRegionChunkRefusal> {
        self.result_identity
            .ok_or(LeniaRegionChunkRefusal::Incomplete)
    }

    pub fn encode_result_chunk(
        &self,
        offset: u32,
        output: &mut [u8],
    ) -> Result<usize, LeniaRegionChunkRefusal> {
        let identity = self.result_identity()?;
        let remaining = identity
            .total_cells
            .checked_sub(offset)
            .ok_or(LeniaRegionChunkRefusal::InvalidCellRange)?;
        let count = remaining.min(LENIA_REGION_CHUNK_MAX_CELLS as u32) as u16;
        if count == 0 {
            return Err(LeniaRegionChunkRefusal::InvalidCellRange);
        }
        let start =
            usize::try_from(offset).map_err(|_| LeniaRegionChunkRefusal::InvalidCellRange)?;
        identity.chunk(offset, count)?.encode(
            &self.result_cells[start..start + usize::from(count)],
            output,
        )
    }
}

impl<const WORK_CELLS: usize, const RESULT_CELLS: usize> Default
    for FixedLeniaRegionWorker<WORK_CELLS, RESULT_CELLS>
{
    fn default() -> Self {
        Self::new()
    }
}

fn validate_assignment(header: LeniaRegionChunkHeader) -> Result<(), LeniaRegionChunkRefusal> {
    if header.field_width != DISTRIBUTED_LENIA_FIELD_WIDTH
        || header.field_height != DISTRIBUTED_LENIA_FIELD_HEIGHT
        || header.generation != 0
        || header.halo != LeniaParameters::ORBIUM.kernel_radius
    {
        return Err(LeniaRegionChunkRefusal::WrongTransfer);
    }
    let expected = match header.region.id {
        LeniaRegionId(0) => LeniaRegion {
            id: LeniaRegionId(0),
            x: 0,
            width: 10,
        },
        LeniaRegionId(1) => LeniaRegion {
            id: LeniaRegionId(1),
            x: 10,
            width: 11,
        },
        LeniaRegionId(2) => LeniaRegion {
            id: LeniaRegionId(2),
            x: 21,
            width: 11,
        },
        _ => return Err(LeniaRegionChunkRefusal::InvalidRegion),
    };
    if header.region != expected {
        return Err(LeniaRegionChunkRefusal::InvalidRegion);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{orbium_seed, LeniaPartition};

    #[test]
    fn fixed_worker_computes_one_exact_region_and_refuses_duplicate_completion() {
        let initial = orbium_seed(32, 32, 1).unwrap();
        let partition =
            LeniaPartition::vertical(&initial, &DISTRIBUTED_LENIA_REGION_WIDTHS).unwrap();
        let work = partition
            .prepare_region(&initial, LeniaRegionId(1), LeniaParameters::ORBIUM)
            .unwrap();
        let identity = LeniaRegionTransferIdentity {
            kind: LeniaRegionChunkKind::Work,
            field_id: work.field_id,
            generation: work.generation,
            field_width: work.field_width,
            field_height: work.field_height,
            region: work.region,
            halo: work.halo,
            total_cells: work.expanded_cells().len() as u32,
        };
        let mut worker = DistributedLeniaWorker::new();
        worker.prepare().unwrap();
        let mut encoded = [0; crate::LENIA_REGION_CHUNK_MAX_BYTES];
        let mut offset = 0;
        while offset < identity.total_cells {
            let count =
                (identity.total_cells - offset).min(LENIA_REGION_CHUNK_MAX_CELLS as u32) as u16;
            let start = offset as usize;
            let length = identity
                .chunk(offset, count)
                .unwrap()
                .encode(
                    &work.expanded_cells()[start..start + count as usize],
                    &mut encoded,
                )
                .unwrap();
            worker.admit_encoded(&encoded[..length]).unwrap();
            offset += u32::from(count);
        }
        let expected = work.evolve(LeniaParameters::ORBIUM).unwrap();
        let mut result = [0; crate::LENIA_REGION_CHUNK_MAX_BYTES];
        let length = worker.encode_result_chunk(0, &mut result).unwrap();
        let view = LeniaRegionChunkView::decode(&result[..length]).unwrap();
        assert_eq!(view.header.region, expected.region);
        assert_eq!(view.cell(0).unwrap(), expected.cells()[0]);
        assert_eq!(
            worker.admit_encoded(&encoded[..1]),
            Err(LeniaRegionChunkRefusal::WrongLength)
        );
    }
}
