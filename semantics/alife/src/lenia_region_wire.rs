//! Fixed bounded Lenia region chunks carried as ordinary session payloads.

use crate::{LeniaFieldId, LeniaRegion, LeniaRegionId};

pub const LENIA_REGION_CHUNK_MAX_BYTES: usize = 1_024;
pub const LENIA_REGION_CHUNK_HEADER_BYTES: usize = 52;
pub const LENIA_REGION_CHUNK_MAX_CELLS: usize =
    (LENIA_REGION_CHUNK_MAX_BYTES - LENIA_REGION_CHUNK_HEADER_BYTES) / 4;

const MAGIC: [u8; 4] = *b"LNR1";
const VERSION: u8 = 1;

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
#[repr(u8)]
pub enum LeniaRegionChunkKind {
    Work = 1,
    Result = 2,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LeniaRegionChunkHeader {
    pub kind: LeniaRegionChunkKind,
    pub field_id: LeniaFieldId,
    pub generation: u64,
    pub field_width: u16,
    pub field_height: u16,
    pub region: LeniaRegion,
    pub halo: u16,
    pub total_cells: u32,
    pub cell_offset: u32,
    pub cell_count: u16,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LeniaRegionTransferIdentity {
    pub kind: LeniaRegionChunkKind,
    pub field_id: LeniaFieldId,
    pub generation: u64,
    pub field_width: u16,
    pub field_height: u16,
    pub region: LeniaRegion,
    pub halo: u16,
    pub total_cells: u32,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LeniaRegionChunkRefusal {
    BufferTooSmall,
    WrongLength,
    WrongMagic,
    WrongVersion,
    WrongKind,
    InvalidDimensions,
    InvalidRegion,
    InvalidHalo,
    InvalidCellRange,
    CellOutOfRange,
    WrongTransfer,
    UnexpectedOffset,
    AlreadyComplete,
    Incomplete,
}

#[derive(Debug, Copy, Clone)]
pub struct LeniaRegionChunkView<'a> {
    pub header: LeniaRegionChunkHeader,
    encoded_cells: &'a [u8],
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LeniaRegionChunkAdmission {
    Progress { admitted_cells: u32 },
    Complete,
}

pub struct LeniaRegionChunkAssembler<'a> {
    identity: LeniaRegionTransferIdentity,
    cells: &'a mut [u32],
    admitted_cells: u32,
}

impl LeniaRegionChunkHeader {
    pub fn transfer_identity(self) -> LeniaRegionTransferIdentity {
        LeniaRegionTransferIdentity {
            kind: self.kind,
            field_id: self.field_id,
            generation: self.generation,
            field_width: self.field_width,
            field_height: self.field_height,
            region: self.region,
            halo: self.halo,
            total_cells: self.total_cells,
        }
    }

    pub fn encode(
        self,
        cells: &[u32],
        output: &mut [u8],
    ) -> Result<usize, LeniaRegionChunkRefusal> {
        validate_header(self)?;
        if cells.len() != usize::from(self.cell_count) {
            return Err(LeniaRegionChunkRefusal::InvalidCellRange);
        }
        if cells.iter().any(|cell| *cell > crate::LENIA_Q16_ONE) {
            return Err(LeniaRegionChunkRefusal::CellOutOfRange);
        }
        let length = LENIA_REGION_CHUNK_HEADER_BYTES + cells.len() * 4;
        if length > LENIA_REGION_CHUNK_MAX_BYTES || output.len() < length {
            return Err(LeniaRegionChunkRefusal::BufferTooSmall);
        }
        output[..length].fill(0);
        output[0..4].copy_from_slice(&MAGIC);
        output[4] = VERSION;
        output[5] = self.kind as u8;
        output[6] = self.region.id.0;
        output[8..24].copy_from_slice(&self.field_id.0);
        output[24..32].copy_from_slice(&self.generation.to_le_bytes());
        output[32..34].copy_from_slice(&self.field_width.to_le_bytes());
        output[34..36].copy_from_slice(&self.field_height.to_le_bytes());
        output[36..38].copy_from_slice(&self.region.x.to_le_bytes());
        output[38..40].copy_from_slice(&self.region.width.to_le_bytes());
        output[40..42].copy_from_slice(&self.halo.to_le_bytes());
        output[42..46].copy_from_slice(&self.total_cells.to_le_bytes());
        output[46..50].copy_from_slice(&self.cell_offset.to_le_bytes());
        output[50..52].copy_from_slice(&self.cell_count.to_le_bytes());
        for (index, cell) in cells.iter().enumerate() {
            let offset = LENIA_REGION_CHUNK_HEADER_BYTES + index * 4;
            output[offset..offset + 4].copy_from_slice(&cell.to_le_bytes());
        }
        Ok(length)
    }
}

impl LeniaRegionTransferIdentity {
    pub fn chunk(
        self,
        cell_offset: u32,
        cell_count: u16,
    ) -> Result<LeniaRegionChunkHeader, LeniaRegionChunkRefusal> {
        let header = LeniaRegionChunkHeader {
            kind: self.kind,
            field_id: self.field_id,
            generation: self.generation,
            field_width: self.field_width,
            field_height: self.field_height,
            region: self.region,
            halo: self.halo,
            total_cells: self.total_cells,
            cell_offset,
            cell_count,
        };
        validate_header(header)?;
        Ok(header)
    }
}

impl<'a> LeniaRegionChunkAssembler<'a> {
    pub fn new(
        identity: LeniaRegionTransferIdentity,
        cells: &'a mut [u32],
    ) -> Result<Self, LeniaRegionChunkRefusal> {
        validate_header(identity.chunk(0, 1)?)?;
        if usize::try_from(identity.total_cells).ok() != Some(cells.len()) {
            return Err(LeniaRegionChunkRefusal::InvalidCellRange);
        }
        Ok(Self {
            identity,
            cells,
            admitted_cells: 0,
        })
    }

    pub fn admit(
        &mut self,
        chunk: LeniaRegionChunkView<'_>,
    ) -> Result<LeniaRegionChunkAdmission, LeniaRegionChunkRefusal> {
        if self.admitted_cells == self.identity.total_cells {
            return Err(LeniaRegionChunkRefusal::AlreadyComplete);
        }
        if chunk.header.transfer_identity() != self.identity {
            return Err(LeniaRegionChunkRefusal::WrongTransfer);
        }
        if chunk.header.cell_offset != self.admitted_cells {
            return Err(LeniaRegionChunkRefusal::UnexpectedOffset);
        }
        let start = usize::try_from(self.admitted_cells)
            .map_err(|_| LeniaRegionChunkRefusal::InvalidCellRange)?;
        for index in 0..chunk.cell_count() {
            self.cells[start + index] = chunk.cell(index)?;
        }
        self.admitted_cells += u32::from(chunk.header.cell_count);
        if self.admitted_cells == self.identity.total_cells {
            Ok(LeniaRegionChunkAdmission::Complete)
        } else {
            Ok(LeniaRegionChunkAdmission::Progress {
                admitted_cells: self.admitted_cells,
            })
        }
    }

    pub fn cells(&self) -> Result<&[u32], LeniaRegionChunkRefusal> {
        if self.admitted_cells != self.identity.total_cells {
            return Err(LeniaRegionChunkRefusal::Incomplete);
        }
        Ok(self.cells)
    }

    pub fn work_view(&self) -> Result<crate::LeniaRegionWorkView<'_>, LeniaRegionChunkRefusal> {
        if self.identity.kind != LeniaRegionChunkKind::Work {
            return Err(LeniaRegionChunkRefusal::WrongKind);
        }
        crate::LeniaRegionWorkView::from_expanded(
            self.identity.field_id,
            self.identity.generation,
            self.identity.field_width,
            self.identity.field_height,
            self.identity.region,
            self.identity.halo,
            self.cells()?,
        )
        .map_err(|_| LeniaRegionChunkRefusal::WrongTransfer)
    }
}

impl<'a> LeniaRegionChunkView<'a> {
    pub fn decode(encoded: &'a [u8]) -> Result<Self, LeniaRegionChunkRefusal> {
        if encoded.len() < LENIA_REGION_CHUNK_HEADER_BYTES {
            return Err(LeniaRegionChunkRefusal::WrongLength);
        }
        if encoded[0..4] != MAGIC {
            return Err(LeniaRegionChunkRefusal::WrongMagic);
        }
        if encoded[4] != VERSION || encoded[7] != 0 {
            return Err(LeniaRegionChunkRefusal::WrongVersion);
        }
        let kind = match encoded[5] {
            1 => LeniaRegionChunkKind::Work,
            2 => LeniaRegionChunkKind::Result,
            _ => return Err(LeniaRegionChunkRefusal::WrongKind),
        };
        let header = LeniaRegionChunkHeader {
            kind,
            field_id: LeniaFieldId(read_array(encoded, 8)?),
            generation: u64::from_le_bytes(read_array(encoded, 24)?),
            field_width: u16::from_le_bytes(read_array(encoded, 32)?),
            field_height: u16::from_le_bytes(read_array(encoded, 34)?),
            region: LeniaRegion {
                id: LeniaRegionId(encoded[6]),
                x: u16::from_le_bytes(read_array(encoded, 36)?),
                width: u16::from_le_bytes(read_array(encoded, 38)?),
            },
            halo: u16::from_le_bytes(read_array(encoded, 40)?),
            total_cells: u32::from_le_bytes(read_array(encoded, 42)?),
            cell_offset: u32::from_le_bytes(read_array(encoded, 46)?),
            cell_count: u16::from_le_bytes(read_array(encoded, 50)?),
        };
        validate_header(header)?;
        let expected = LENIA_REGION_CHUNK_HEADER_BYTES + usize::from(header.cell_count) * 4;
        if encoded.len() != expected || encoded.len() > LENIA_REGION_CHUNK_MAX_BYTES {
            return Err(LeniaRegionChunkRefusal::WrongLength);
        }
        let view = Self {
            header,
            encoded_cells: &encoded[LENIA_REGION_CHUNK_HEADER_BYTES..],
        };
        for index in 0..view.cell_count() {
            if view.cell(index)? > crate::LENIA_Q16_ONE {
                return Err(LeniaRegionChunkRefusal::CellOutOfRange);
            }
        }
        Ok(view)
    }

    pub fn cell_count(self) -> usize {
        usize::from(self.header.cell_count)
    }

    pub fn cell(self, index: usize) -> Result<u32, LeniaRegionChunkRefusal> {
        if index >= self.cell_count() {
            return Err(LeniaRegionChunkRefusal::InvalidCellRange);
        }
        let offset = index * 4;
        Ok(u32::from_le_bytes(read_array(self.encoded_cells, offset)?))
    }
}

fn validate_header(header: LeniaRegionChunkHeader) -> Result<(), LeniaRegionChunkRefusal> {
    if header.field_width < crate::LENIA_MINIMUM_EXTENT
        || header.field_width > crate::LENIA_MAXIMUM_EXTENT
        || header.field_height < crate::LENIA_MINIMUM_EXTENT
        || header.field_height > crate::LENIA_MAXIMUM_EXTENT
    {
        return Err(LeniaRegionChunkRefusal::InvalidDimensions);
    }
    let region_end = header
        .region
        .x
        .checked_add(header.region.width)
        .ok_or(LeniaRegionChunkRefusal::InvalidRegion)?;
    if header.region.width == 0 || region_end > header.field_width {
        return Err(LeniaRegionChunkRefusal::InvalidRegion);
    }
    let expected_cells = match header.kind {
        LeniaRegionChunkKind::Work => {
            if header.halo == 0 || header.halo > crate::LENIA_MAXIMUM_KERNEL_RADIUS {
                return Err(LeniaRegionChunkRefusal::InvalidHalo);
            }
            let halo_extent = header
                .halo
                .checked_mul(2)
                .ok_or(LeniaRegionChunkRefusal::InvalidDimensions)?;
            u32::from(
                header
                    .region
                    .width
                    .checked_add(halo_extent)
                    .ok_or(LeniaRegionChunkRefusal::InvalidDimensions)?,
            ) * u32::from(
                header
                    .field_height
                    .checked_add(halo_extent)
                    .ok_or(LeniaRegionChunkRefusal::InvalidDimensions)?,
            )
        }
        LeniaRegionChunkKind::Result => {
            if header.halo != 0 {
                return Err(LeniaRegionChunkRefusal::InvalidHalo);
            }
            u32::from(header.region.width) * u32::from(header.field_height)
        }
    };
    if header.total_cells != expected_cells
        || header.cell_count == 0
        || usize::from(header.cell_count) > LENIA_REGION_CHUNK_MAX_CELLS
        || header
            .cell_offset
            .checked_add(u32::from(header.cell_count))
            .is_none_or(|end| end > header.total_cells)
    {
        return Err(LeniaRegionChunkRefusal::InvalidCellRange);
    }
    Ok(())
}

fn read_array<const N: usize>(
    bytes: &[u8],
    offset: usize,
) -> Result<[u8; N], LeniaRegionChunkRefusal> {
    bytes
        .get(offset..offset + N)
        .and_then(|slice| slice.try_into().ok())
        .ok_or(LeniaRegionChunkRefusal::WrongLength)
}
