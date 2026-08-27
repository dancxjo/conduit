//! Host-neutral bounded reaction-diffusion field semantics.
//!
//! This module fixes one deterministic Gray-Scott numeric profile. It contains
//! no placement, partition, transport, renderer, or platform identity.
//! Each generation samples the prior generation through a five-point stencil
//! with toroidal boundaries. Products divide by 1,000,000 with signed integer
//! truncation toward zero; all terms are calculated before the destination
//! cell is written, and final concentrations clamp to the closed [0, 1] range.

use alloc::vec;
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

pub const REACTION_DIFFUSION_STATE_INFO_ID: &str = "field/reaction-diffusion-state@1";
pub const REACTION_DIFFUSION_REQUEST_INFO_ID: &str = "field/evolve-request@1";
pub const REACTION_DIFFUSION_NUMERIC_PROFILE: &str = "field/gray-scott-ppm-sync-torus@1";
pub const REACTION_DIFFUSION_MINIMUM_EXTENT: u16 = 3;
pub const REACTION_DIFFUSION_MAXIMUM_EXTENT: u16 = 64;
pub const REACTION_DIFFUSION_MAXIMUM_CELLS: u32 = 4_096;
pub const REACTION_DIFFUSION_MAXIMUM_GENERATIONS: u16 = 64;
pub const REACTION_DIFFUSION_MAXIMUM_WORK: u32 =
    REACTION_DIFFUSION_MAXIMUM_CELLS * REACTION_DIFFUSION_MAXIMUM_GENERATIONS as u32;
pub const REACTION_DIFFUSION_MAXIMUM_STATE_BYTES: u32 =
    FIELD_HEADER_BYTES as u32 + REACTION_DIFFUSION_MAXIMUM_CELLS * 8;
pub const REACTION_DIFFUSION_REQUEST_BYTES: u32 = 44;

const CONCENTRATION_SCALE: i64 = 1_000_000;
const FIELD_MAGIC: [u8; 8] = *b"CNDFLD01";
const NUMERIC_PROFILE_TAG: u32 = 0x4753_5031;
const FIELD_HEADER_BYTES: usize = 64;
const FIELD_DIGEST_DOMAIN: &[u8] = b"conduit.field.state.v1";
const REQUEST_MAGIC: [u8; 8] = *b"CNDFRQ01";

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash)]
pub struct ReactionDiffusionFieldId(pub [u8; 16]);

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct GrayScottParameters {
    pub diffusion_u_ppm: u32,
    pub diffusion_v_ppm: u32,
    pub feed_ppm: u32,
    pub kill_ppm: u32,
    pub time_step_ppm: u32,
}

impl GrayScottParameters {
    pub const REFERENCE: Self = Self {
        diffusion_u_ppm: 160_000,
        diffusion_v_ppm: 80_000,
        feed_ppm: 35_000,
        kill_ppm: 65_000,
        time_step_ppm: 1_000_000,
    };

    pub fn validate(self) -> Result<(), ReactionDiffusionRefusal> {
        let bounded = [
            self.diffusion_u_ppm,
            self.diffusion_v_ppm,
            self.feed_ppm,
            self.kill_ppm,
            self.time_step_ppm,
        ]
        .into_iter()
        .all(|value| value <= CONCENTRATION_SCALE as u32);
        if !bounded
            || self.diffusion_u_ppm == 0
            || self.diffusion_v_ppm == 0
            || self.time_step_ppm == 0
            || self.feed_ppm.saturating_add(self.kill_ppm) > CONCENTRATION_SCALE as u32
        {
            return Err(ReactionDiffusionRefusal::InvalidParameters);
        }
        Ok(())
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionCell {
    pub u_ppm: u32,
    pub v_ppm: u32,
}

impl ReactionDiffusionCell {
    pub const REST: Self = Self {
        u_ppm: CONCENTRATION_SCALE as u32,
        v_ppm: 0,
    };

    pub fn new(u_ppm: u32, v_ppm: u32) -> Result<Self, ReactionDiffusionRefusal> {
        let cell = Self { u_ppm, v_ppm };
        cell.validate()?;
        Ok(cell)
    }

    fn validate(self) -> Result<(), ReactionDiffusionRefusal> {
        if self.u_ppm > CONCENTRATION_SCALE as u32 || self.v_ppm > CONCENTRATION_SCALE as u32 {
            return Err(ReactionDiffusionRefusal::ConcentrationOutOfRange);
        }
        Ok(())
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionFieldState {
    pub field_id: ReactionDiffusionFieldId,
    pub generation: u64,
    pub width: u16,
    pub height: u16,
    pub parameters: GrayScottParameters,
    cells: Vec<ReactionDiffusionCell>,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq)]
pub struct ReactionDiffusionEvolveRequest {
    pub field_id: ReactionDiffusionFieldId,
    pub expected_generation: u64,
    pub generations: u16,
    pub admitted_cell_generations: u32,
}

impl ReactionDiffusionEvolveRequest {
    pub fn encode(self) -> [u8; REACTION_DIFFUSION_REQUEST_BYTES as usize] {
        let mut encoded = [0; REACTION_DIFFUSION_REQUEST_BYTES as usize];
        encoded[0..8].copy_from_slice(&REQUEST_MAGIC);
        encoded[8..12].copy_from_slice(&NUMERIC_PROFILE_TAG.to_le_bytes());
        encoded[12..28].copy_from_slice(&self.field_id.0);
        encoded[28..36].copy_from_slice(&self.expected_generation.to_le_bytes());
        encoded[36..38].copy_from_slice(&self.generations.to_le_bytes());
        encoded[40..44].copy_from_slice(&self.admitted_cell_generations.to_le_bytes());
        encoded
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ReactionDiffusionRefusal> {
        if encoded.len() != REACTION_DIFFUSION_REQUEST_BYTES as usize {
            return Err(ReactionDiffusionRefusal::WrongLength {
                expected: REACTION_DIFFUSION_REQUEST_BYTES as usize,
                actual: encoded.len(),
            });
        }
        if encoded[0..8] != REQUEST_MAGIC {
            return Err(ReactionDiffusionRefusal::WrongMagic);
        }
        if read_u32(encoded, 8)? != NUMERIC_PROFILE_TAG || encoded[38..40] != [0, 0] {
            return Err(ReactionDiffusionRefusal::WrongNumericProfile);
        }
        let mut field_id = [0; 16];
        field_id.copy_from_slice(&encoded[12..28]);
        Ok(Self {
            field_id: ReactionDiffusionFieldId(field_id),
            expected_generation: read_u64(encoded, 28)?,
            generations: read_u16(encoded, 36)?,
            admitted_cell_generations: read_u32(encoded, 40)?,
        })
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ReactionDiffusionRefusal {
    WrongLength { expected: usize, actual: usize },
    WrongMagic,
    WrongNumericProfile,
    InvalidDimensions,
    CellCountMismatch,
    ConcentrationOutOfRange,
    InvalidParameters,
    WrongFieldIdentity,
    StaleGeneration { expected: u64, actual: u64 },
    InvalidGenerationCount,
    GenerationOverflow,
    WorkLimitExceeded { required: u32, admitted: u32 },
    ArithmeticOverflow,
}

impl ReactionDiffusionFieldState {
    pub fn initialized(
        field_id: ReactionDiffusionFieldId,
        width: u16,
        height: u16,
        parameters: GrayScottParameters,
        seed: u64,
    ) -> Result<Self, ReactionDiffusionRefusal> {
        let count = validate_dimensions(width, height)?;
        parameters.validate()?;
        let mut cells = vec![ReactionDiffusionCell::REST; count];
        let center_x = usize::from(width / 2);
        let center_y = usize::from(height / 2);
        let radius = 1 + usize::try_from(seed % 3)
            .map_err(|_| ReactionDiffusionRefusal::ArithmeticOverflow)?;
        for y in center_y.saturating_sub(radius)..=(center_y + radius).min(usize::from(height) - 1)
        {
            for x in
                center_x.saturating_sub(radius)..=(center_x + radius).min(usize::from(width) - 1)
            {
                let variation = ((seed ^ ((x as u64) << 32) ^ y as u64)
                    .wrapping_mul(0x9e37_79b9_7f4a_7c15)
                    >> 56) as u32;
                cells[y * usize::from(width) + x] = ReactionDiffusionCell {
                    u_ppm: 480_000 + variation * 1_000,
                    v_ppm: 240_000 + variation * 500,
                };
            }
        }
        let state = Self {
            field_id,
            generation: 0,
            width,
            height,
            parameters,
            cells,
        };
        state.validate()?;
        Ok(state)
    }

    pub fn from_cells(
        field_id: ReactionDiffusionFieldId,
        generation: u64,
        width: u16,
        height: u16,
        parameters: GrayScottParameters,
        cells: Vec<ReactionDiffusionCell>,
    ) -> Result<Self, ReactionDiffusionRefusal> {
        let state = Self {
            field_id,
            generation,
            width,
            height,
            parameters,
            cells,
        };
        state.validate()?;
        Ok(state)
    }

    pub fn cells(&self) -> &[ReactionDiffusionCell] {
        &self.cells
    }

    pub fn validate(&self) -> Result<(), ReactionDiffusionRefusal> {
        let count = validate_dimensions(self.width, self.height)?;
        self.parameters.validate()?;
        if self.cells.len() != count {
            return Err(ReactionDiffusionRefusal::CellCountMismatch);
        }
        self.cells.iter().try_for_each(|cell| cell.validate())
    }

    pub fn evolve_reference(
        &self,
        request: ReactionDiffusionEvolveRequest,
    ) -> Result<Self, ReactionDiffusionRefusal> {
        self.validate()?;
        if request.field_id != self.field_id {
            return Err(ReactionDiffusionRefusal::WrongFieldIdentity);
        }
        if request.expected_generation != self.generation {
            return Err(ReactionDiffusionRefusal::StaleGeneration {
                expected: self.generation,
                actual: request.expected_generation,
            });
        }
        if request.generations == 0 || request.generations > REACTION_DIFFUSION_MAXIMUM_GENERATIONS
        {
            return Err(ReactionDiffusionRefusal::InvalidGenerationCount);
        }
        self.generation
            .checked_add(u64::from(request.generations))
            .ok_or(ReactionDiffusionRefusal::GenerationOverflow)?;
        let required = u32::from(self.width)
            .checked_mul(u32::from(self.height))
            .and_then(|cells| cells.checked_mul(u32::from(request.generations)))
            .ok_or(ReactionDiffusionRefusal::ArithmeticOverflow)?;
        if required > REACTION_DIFFUSION_MAXIMUM_WORK
            || required > request.admitted_cell_generations
        {
            return Err(ReactionDiffusionRefusal::WorkLimitExceeded {
                required,
                admitted: request.admitted_cell_generations,
            });
        }

        let mut current = self.cells.clone();
        let mut next = vec![ReactionDiffusionCell::REST; current.len()];
        for _ in 0..request.generations {
            crate::reaction_diffusion_evolution::evolve_generation(
                &current,
                &mut next,
                usize::from(self.width),
                usize::from(self.height),
                self.parameters,
            )?;
            core::mem::swap(&mut current, &mut next);
        }
        Self::from_cells(
            self.field_id,
            self.generation + u64::from(request.generations),
            self.width,
            self.height,
            self.parameters,
            current,
        )
    }

    pub fn encode(&self) -> Result<Vec<u8>, ReactionDiffusionRefusal> {
        self.validate()?;
        let mut encoded = Vec::with_capacity(FIELD_HEADER_BYTES + self.cells.len() * 8);
        encoded.extend_from_slice(&FIELD_MAGIC);
        encoded.extend_from_slice(&NUMERIC_PROFILE_TAG.to_le_bytes());
        encoded.extend_from_slice(&self.width.to_le_bytes());
        encoded.extend_from_slice(&self.height.to_le_bytes());
        encoded.extend_from_slice(&self.generation.to_le_bytes());
        encoded.extend_from_slice(&self.field_id.0);
        for parameter in [
            self.parameters.diffusion_u_ppm,
            self.parameters.diffusion_v_ppm,
            self.parameters.feed_ppm,
            self.parameters.kill_ppm,
            self.parameters.time_step_ppm,
            self.cells.len() as u32,
        ] {
            encoded.extend_from_slice(&parameter.to_le_bytes());
        }
        debug_assert_eq!(encoded.len(), FIELD_HEADER_BYTES);
        for cell in &self.cells {
            encoded.extend_from_slice(&cell.u_ppm.to_le_bytes());
            encoded.extend_from_slice(&cell.v_ppm.to_le_bytes());
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ReactionDiffusionRefusal> {
        if encoded.len() < FIELD_HEADER_BYTES {
            return Err(ReactionDiffusionRefusal::WrongLength {
                expected: FIELD_HEADER_BYTES,
                actual: encoded.len(),
            });
        }
        if encoded[0..8] != FIELD_MAGIC {
            return Err(ReactionDiffusionRefusal::WrongMagic);
        }
        if read_u32(encoded, 8)? != NUMERIC_PROFILE_TAG {
            return Err(ReactionDiffusionRefusal::WrongNumericProfile);
        }
        let width = read_u16(encoded, 12)?;
        let height = read_u16(encoded, 14)?;
        let count = validate_dimensions(width, height)?;
        let encoded_count = usize::try_from(read_u32(encoded, 60)?)
            .map_err(|_| ReactionDiffusionRefusal::CellCountMismatch)?;
        if encoded_count != count {
            return Err(ReactionDiffusionRefusal::CellCountMismatch);
        }
        let expected = FIELD_HEADER_BYTES + count * 8;
        if encoded.len() != expected {
            return Err(ReactionDiffusionRefusal::WrongLength {
                expected,
                actual: encoded.len(),
            });
        }
        let mut field_id = [0; 16];
        field_id.copy_from_slice(&encoded[24..40]);
        let parameters = GrayScottParameters {
            diffusion_u_ppm: read_u32(encoded, 40)?,
            diffusion_v_ppm: read_u32(encoded, 44)?,
            feed_ppm: read_u32(encoded, 48)?,
            kill_ppm: read_u32(encoded, 52)?,
            time_step_ppm: read_u32(encoded, 56)?,
        };
        let mut cells = Vec::with_capacity(count);
        for offset in (FIELD_HEADER_BYTES..expected).step_by(8) {
            cells.push(ReactionDiffusionCell::new(
                read_u32(encoded, offset)?,
                read_u32(encoded, offset + 4)?,
            )?);
        }
        Self::from_cells(
            ReactionDiffusionFieldId(field_id),
            read_u64(encoded, 16)?,
            width,
            height,
            parameters,
            cells,
        )
    }

    pub fn semantic_digest(&self) -> Result<[u8; 32], ReactionDiffusionRefusal> {
        let encoded = self.encode()?;
        let mut digest = Sha256::new();
        digest.update(FIELD_DIGEST_DOMAIN);
        digest.update(REACTION_DIFFUSION_STATE_INFO_ID.as_bytes());
        digest.update(encoded);
        Ok(digest.finalize().into())
    }
}

fn validate_dimensions(width: u16, height: u16) -> Result<usize, ReactionDiffusionRefusal> {
    if !(REACTION_DIFFUSION_MINIMUM_EXTENT..=REACTION_DIFFUSION_MAXIMUM_EXTENT).contains(&width)
        || !(REACTION_DIFFUSION_MINIMUM_EXTENT..=REACTION_DIFFUSION_MAXIMUM_EXTENT)
            .contains(&height)
    {
        return Err(ReactionDiffusionRefusal::InvalidDimensions);
    }
    let count = usize::from(width) * usize::from(height);
    if count > REACTION_DIFFUSION_MAXIMUM_CELLS as usize {
        return Err(ReactionDiffusionRefusal::InvalidDimensions);
    }
    Ok(count)
}

fn read_u16(encoded: &[u8], offset: usize) -> Result<u16, ReactionDiffusionRefusal> {
    encoded
        .get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ReactionDiffusionRefusal::WrongLength {
            expected: offset + 2,
            actual: encoded.len(),
        })
}

fn read_u32(encoded: &[u8], offset: usize) -> Result<u32, ReactionDiffusionRefusal> {
    encoded
        .get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ReactionDiffusionRefusal::WrongLength {
            expected: offset + 4,
            actual: encoded.len(),
        })
}

fn read_u64(encoded: &[u8], offset: usize) -> Result<u64, ReactionDiffusionRefusal> {
    encoded
        .get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ReactionDiffusionRefusal::WrongLength {
            expected: offset + 8,
            actual: encoded.len(),
        })
}
