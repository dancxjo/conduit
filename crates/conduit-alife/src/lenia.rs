//! Portable bounded Lenia scalar-field semantics.
//!
//! The reviewed profile stores every cell as unsigned Q16.16 and performs the
//! convolution, Gaussian shell, Gaussian growth, Euler integration, and clamp
//! with specified integer arithmetic. It contains no placement, partition,
//! transport, renderer, or platform identity.

use alloc::{vec, vec::Vec};
use sha2::{Digest, Sha256};

pub const SCALAR_FIELD2_INFO_ID: &str = "alife/scalar-field2@1";
pub const LENIA_NUMERIC_PROFILE: &str = "alife/fixed-q16.16@1";
pub const LENIA_MINIMUM_EXTENT: u16 = 32;
pub const LENIA_MAXIMUM_EXTENT: u16 = 128;
pub const LENIA_MAXIMUM_CELLS: u32 = 16_384;
pub const LENIA_MAXIMUM_KERNEL_RADIUS: u16 = 16;
pub const LENIA_MAXIMUM_FIELD_BYTES: u32 =
    LENIA_FIELD_HEADER_BYTES as u32 + LENIA_MAXIMUM_CELLS * 4;
pub const LENIA_Q16_ONE: u32 = 1 << 16;

const LENIA_FIELD_MAGIC: [u8; 8] = *b"CNDLEN01";
const LENIA_PROFILE_TAG: u32 = 0x5131_3631;
const LENIA_FIELD_HEADER_BYTES: usize = 48;
const LENIA_FIELD_DIGEST_DOMAIN: &[u8] = b"conduit.alife.scalar-field2.v1";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct LeniaFieldId(pub [u8; 16]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub enum LeniaBoundary {
    Wrap,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LeniaParameters {
    pub kernel_radius: u16,
    pub kernel_mu_q16: u32,
    pub kernel_sigma_q16: u32,
    pub growth_mu_q16: u32,
    pub growth_sigma_q16: u32,
    pub dt_q16: u32,
    pub boundary: LeniaBoundary,
}

impl LeniaParameters {
    pub const ORBIUM: Self = Self {
        kernel_radius: 13,
        kernel_mu_q16: LENIA_Q16_ONE / 2,
        kernel_sigma_q16: 9_830,
        growth_mu_q16: 9_830,
        growth_sigma_q16: 983,
        dt_q16: 6_554,
        boundary: LeniaBoundary::Wrap,
    };

    pub fn validate(self) -> Result<(), LeniaRefusal> {
        if self.kernel_radius == 0
            || self.kernel_radius > LENIA_MAXIMUM_KERNEL_RADIUS
            || self.kernel_mu_q16 > LENIA_Q16_ONE
            || self.kernel_sigma_q16 == 0
            || self.kernel_sigma_q16 > LENIA_Q16_ONE
            || self.growth_mu_q16 > LENIA_Q16_ONE
            || self.growth_sigma_q16 == 0
            || self.growth_sigma_q16 > LENIA_Q16_ONE
            || self.dt_q16 == 0
            || self.dt_q16 > LENIA_Q16_ONE
        {
            return Err(LeniaRefusal::InvalidParameters);
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct LeniaFieldHeader {
    pub field_id: LeniaFieldId,
    pub generation: u64,
    pub width: u16,
    pub height: u16,
    pub cell_count: u32,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct LeniaFieldState {
    pub field_id: LeniaFieldId,
    pub generation: u64,
    pub width: u16,
    pub height: u16,
    cells: Vec<u32>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LeniaRefusal {
    WrongLength { expected: usize, actual: usize },
    WrongMagic,
    WrongNumericProfile,
    InvalidDimensions,
    CellCountMismatch,
    CellOutOfRange,
    InvalidParameters,
    Uninitialized,
    GenerationOverflow,
    ArithmeticOverflow,
    InvalidSeed,
}

impl LeniaFieldState {
    pub fn from_cells(
        field_id: LeniaFieldId,
        generation: u64,
        width: u16,
        height: u16,
        cells: Vec<u32>,
    ) -> Result<Self, LeniaRefusal> {
        let state = Self {
            field_id,
            generation,
            width,
            height,
            cells,
        };
        state.validate()?;
        Ok(state)
    }

    pub fn cells(&self) -> &[u32] {
        &self.cells
    }

    pub fn validate(&self) -> Result<(), LeniaRefusal> {
        let count = validate_lenia_dimensions(self.width, self.height)?;
        if self.cells.len() != count {
            return Err(LeniaRefusal::CellCountMismatch);
        }
        validate_cells(&self.cells)
    }

    pub fn encode(&self) -> Result<Vec<u8>, LeniaRefusal> {
        self.validate()?;
        let mut encoded = Vec::with_capacity(LENIA_FIELD_HEADER_BYTES + self.cells.len() * 4);
        encode_field_into(
            LeniaFieldHeader {
                field_id: self.field_id,
                generation: self.generation,
                width: self.width,
                height: self.height,
                cell_count: self.cells.len() as u32,
            },
            &self.cells,
            &mut encoded,
        )?;
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, LeniaRefusal> {
        let header = decode_lenia_field_header(encoded)?;
        let cells = decode_cells(encoded)?.collect::<Result<Vec<_>, _>>()?;
        Self::from_cells(
            header.field_id,
            header.generation,
            header.width,
            header.height,
            cells,
        )
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], LeniaRefusal> {
        let mut digest = Sha256::new();
        digest.update(LENIA_FIELD_DIGEST_DOMAIN);
        digest.update(SCALAR_FIELD2_INFO_ID.as_bytes());
        digest.update(self.encode()?);
        Ok(digest.finalize().into())
    }

    pub fn evolve_reference(
        &self,
        parameters: LeniaParameters,
        generations: u16,
    ) -> Result<Self, LeniaRefusal> {
        if generations == 0 || generations > 64 {
            return Err(LeniaRefusal::InvalidParameters);
        }
        let encoded = self.encode()?;
        let mut engine = LeniaEngine::new(parameters)?;
        engine.initialize(&encoded)?;
        let mut output = Vec::with_capacity(LENIA_MAXIMUM_FIELD_BYTES as usize);
        for _ in 0..generations {
            engine.step_into(&mut output)?;
        }
        Self::decode(&output)
    }
}

/// A maximum-capacity engine prepared before Play start and mutated without
/// allocating while admitted Lenia initialization and step operations run.
pub struct LeniaEngine {
    parameters: LeniaParameters,
    kernel: Vec<crate::lenia_evolution::KernelSample>,
    kernel_weight: u64,
    field_id: Option<LeniaFieldId>,
    generation: u64,
    width: u16,
    height: u16,
    cell_count: usize,
    current: Vec<u32>,
    next: Vec<u32>,
}

impl LeniaEngine {
    pub fn new(parameters: LeniaParameters) -> Result<Self, LeniaRefusal> {
        parameters.validate()?;
        let mut kernel = Vec::with_capacity(usize::from(parameters.kernel_radius * 2 + 1).pow(2));
        let kernel_weight = crate::lenia_evolution::build_kernel(parameters, &mut kernel)?;
        Ok(Self {
            parameters,
            kernel,
            kernel_weight,
            field_id: None,
            generation: 0,
            width: 0,
            height: 0,
            cell_count: 0,
            current: vec![0; LENIA_MAXIMUM_CELLS as usize],
            next: vec![0; LENIA_MAXIMUM_CELLS as usize],
        })
    }

    pub fn initialize(&mut self, encoded: &[u8]) -> Result<(), LeniaRefusal> {
        let header = decode_lenia_field_header(encoded)?;
        let count =
            usize::try_from(header.cell_count).map_err(|_| LeniaRefusal::CellCountMismatch)?;
        for (destination, source) in self.current[..count].iter_mut().zip(decode_cells(encoded)?) {
            *destination = source?;
        }
        self.field_id = Some(header.field_id);
        self.generation = header.generation;
        self.width = header.width;
        self.height = header.height;
        self.cell_count = count;
        Ok(())
    }

    pub fn step_into(&mut self, output: &mut Vec<u8>) -> Result<(), LeniaRefusal> {
        let field_id = self.field_id.ok_or(LeniaRefusal::Uninitialized)?;
        let generation = self
            .generation
            .checked_add(1)
            .ok_or(LeniaRefusal::GenerationOverflow)?;
        crate::lenia_evolution::evolve_generation(
            &self.current[..self.cell_count],
            &mut self.next[..self.cell_count],
            usize::from(self.width),
            usize::from(self.height),
            self.parameters,
            &self.kernel,
            self.kernel_weight,
        )?;
        core::mem::swap(&mut self.current, &mut self.next);
        self.generation = generation;
        encode_field_into(
            LeniaFieldHeader {
                field_id,
                generation,
                width: self.width,
                height: self.height,
                cell_count: self.cell_count as u32,
            },
            &self.current[..self.cell_count],
            output,
        )
    }

    pub fn allocation_capacity(&self) -> usize {
        self.kernel.capacity() + self.current.capacity() + self.next.capacity()
    }
}

pub fn decode_lenia_field_header(encoded: &[u8]) -> Result<LeniaFieldHeader, LeniaRefusal> {
    if encoded.len() < LENIA_FIELD_HEADER_BYTES {
        return Err(LeniaRefusal::WrongLength {
            expected: LENIA_FIELD_HEADER_BYTES,
            actual: encoded.len(),
        });
    }
    if encoded[0..8] != LENIA_FIELD_MAGIC {
        return Err(LeniaRefusal::WrongMagic);
    }
    if read_u32(encoded, 8)? != LENIA_PROFILE_TAG || encoded[44..48] != [0; 4] {
        return Err(LeniaRefusal::WrongNumericProfile);
    }
    let width = read_u16(encoded, 12)?;
    let height = read_u16(encoded, 14)?;
    let expected_count = validate_lenia_dimensions(width, height)?;
    let cell_count = read_u32(encoded, 40)?;
    if usize::try_from(cell_count).ok() != Some(expected_count) {
        return Err(LeniaRefusal::CellCountMismatch);
    }
    let expected = LENIA_FIELD_HEADER_BYTES + expected_count * 4;
    if encoded.len() != expected {
        return Err(LeniaRefusal::WrongLength {
            expected,
            actual: encoded.len(),
        });
    }
    let mut field_id = [0; 16];
    field_id.copy_from_slice(&encoded[24..40]);
    Ok(LeniaFieldHeader {
        field_id: LeniaFieldId(field_id),
        generation: read_u64(encoded, 16)?,
        width,
        height,
        cell_count,
    })
}

pub fn lenia_field_cell(encoded: &[u8], index: usize) -> Result<u32, LeniaRefusal> {
    LeniaFieldView::decode(encoded)?.cell(index)
}

pub struct LeniaFieldView<'a> {
    pub header: LeniaFieldHeader,
    encoded: &'a [u8],
}

impl<'a> LeniaFieldView<'a> {
    pub fn decode(encoded: &'a [u8]) -> Result<Self, LeniaRefusal> {
        let header = decode_lenia_field_header(encoded)?;
        for index in 0..header.cell_count as usize {
            let value = read_u32(encoded, LENIA_FIELD_HEADER_BYTES + index * 4)?;
            if value > LENIA_Q16_ONE {
                return Err(LeniaRefusal::CellOutOfRange);
            }
        }
        Ok(Self { header, encoded })
    }

    pub fn cell(&self, index: usize) -> Result<u32, LeniaRefusal> {
        if index >= self.header.cell_count as usize {
            return Err(LeniaRefusal::CellCountMismatch);
        }
        read_u32(self.encoded, LENIA_FIELD_HEADER_BYTES + index * 4)
    }

    pub fn cells(&self) -> impl ExactSizeIterator<Item = u32> + '_ {
        (0..self.header.cell_count as usize).map(|index| {
            read_u32(self.encoded, LENIA_FIELD_HEADER_BYTES + index * 4)
                .expect("validated ScalarField2 cell")
        })
    }
}

pub(crate) fn validate_lenia_dimensions(width: u16, height: u16) -> Result<usize, LeniaRefusal> {
    if !(LENIA_MINIMUM_EXTENT..=LENIA_MAXIMUM_EXTENT).contains(&width)
        || !(LENIA_MINIMUM_EXTENT..=LENIA_MAXIMUM_EXTENT).contains(&height)
    {
        return Err(LeniaRefusal::InvalidDimensions);
    }
    let count = usize::from(width) * usize::from(height);
    if count > LENIA_MAXIMUM_CELLS as usize {
        return Err(LeniaRefusal::InvalidDimensions);
    }
    Ok(count)
}

fn encode_field_into(
    header: LeniaFieldHeader,
    cells: &[u32],
    output: &mut Vec<u8>,
) -> Result<(), LeniaRefusal> {
    let count = validate_lenia_dimensions(header.width, header.height)?;
    if cells.len() != count || header.cell_count as usize != count {
        return Err(LeniaRefusal::CellCountMismatch);
    }
    validate_cells(cells)?;
    output.clear();
    output.extend_from_slice(&LENIA_FIELD_MAGIC);
    output.extend_from_slice(&LENIA_PROFILE_TAG.to_le_bytes());
    output.extend_from_slice(&header.width.to_le_bytes());
    output.extend_from_slice(&header.height.to_le_bytes());
    output.extend_from_slice(&header.generation.to_le_bytes());
    output.extend_from_slice(&header.field_id.0);
    output.extend_from_slice(&header.cell_count.to_le_bytes());
    output.extend_from_slice(&[0; 4]);
    for value in cells {
        output.extend_from_slice(&value.to_le_bytes());
    }
    Ok(())
}

fn decode_cells(
    encoded: &[u8],
) -> Result<impl Iterator<Item = Result<u32, LeniaRefusal>> + '_, LeniaRefusal> {
    let header = decode_lenia_field_header(encoded)?;
    Ok((0..header.cell_count as usize).map(|index| {
        let value = read_u32(encoded, LENIA_FIELD_HEADER_BYTES + index * 4)?;
        (value <= LENIA_Q16_ONE)
            .then_some(value)
            .ok_or(LeniaRefusal::CellOutOfRange)
    }))
}

fn validate_cells(cells: &[u32]) -> Result<(), LeniaRefusal> {
    cells
        .iter()
        .all(|value| *value <= LENIA_Q16_ONE)
        .then_some(())
        .ok_or(LeniaRefusal::CellOutOfRange)
}

fn read_u16(encoded: &[u8], offset: usize) -> Result<u16, LeniaRefusal> {
    encoded
        .get(offset..offset + 2)
        .and_then(|value| value.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(LeniaRefusal::WrongLength {
            expected: offset + 2,
            actual: encoded.len(),
        })
}

fn read_u32(encoded: &[u8], offset: usize) -> Result<u32, LeniaRefusal> {
    encoded
        .get(offset..offset + 4)
        .and_then(|value| value.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(LeniaRefusal::WrongLength {
            expected: offset + 4,
            actual: encoded.len(),
        })
}

fn read_u64(encoded: &[u8], offset: usize) -> Result<u64, LeniaRefusal> {
    encoded
        .get(offset..offset + 8)
        .and_then(|value| value.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(LeniaRefusal::WrongLength {
            expected: offset + 8,
            actual: encoded.len(),
        })
}

#[cfg(test)]
#[path = "lenia_tests.rs"]
mod tests;
