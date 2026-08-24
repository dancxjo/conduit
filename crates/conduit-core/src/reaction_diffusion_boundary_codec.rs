//! Canonical codec for one directed reaction-diffusion boundary segment.

use alloc::vec::Vec;

use crate::{
    ReactionDiffusionBoundaryEdge, ReactionDiffusionBoundaryState, ReactionDiffusionCell,
    ReactionDiffusionFieldId, ReactionDiffusionPartitionRefusal, ReactionDiffusionRegionId,
    REACTION_DIFFUSION_BOUNDARY_HEADER_BYTES,
};

const BOUNDARY_MAGIC: [u8; 8] = *b"CNDBND01";
const NUMERIC_PROFILE_TAG: u32 = 0x4753_5031;

impl ReactionDiffusionBoundaryState {
    pub fn encode(&self) -> Result<Vec<u8>, ReactionDiffusionPartitionRefusal> {
        if self.values.len() != 1 {
            return Err(ReactionDiffusionPartitionRefusal::MalformedBoundaryLength);
        }
        let mut encoded =
            Vec::with_capacity(REACTION_DIFFUSION_BOUNDARY_HEADER_BYTES + self.values.len() * 8);
        encoded.extend_from_slice(&BOUNDARY_MAGIC);
        encoded.extend_from_slice(&NUMERIC_PROFILE_TAG.to_le_bytes());
        encoded.extend_from_slice(&self.field_id.0);
        encoded.extend_from_slice(&self.generation.to_le_bytes());
        encoded.extend_from_slice(&self.boundary_id.to_le_bytes());
        encoded.extend_from_slice(&self.source_region.0.to_le_bytes());
        encoded.extend_from_slice(&self.destination_region.0.to_le_bytes());
        encoded.push(edge_tag(self.destination_edge));
        encoded.push(0);
        encoded.extend_from_slice(&self.destination_offset.to_le_bytes());
        encoded.extend_from_slice(&(self.values.len() as u16).to_le_bytes());
        for value in &self.values {
            ReactionDiffusionCell::new(value.u_ppm, value.v_ppm)
                .map_err(ReactionDiffusionPartitionRefusal::Field)?;
            encoded.extend_from_slice(&value.u_ppm.to_le_bytes());
            encoded.extend_from_slice(&value.v_ppm.to_le_bytes());
        }
        Ok(encoded)
    }

    pub fn decode(encoded: &[u8]) -> Result<Self, ReactionDiffusionPartitionRefusal> {
        if encoded.len() < REACTION_DIFFUSION_BOUNDARY_HEADER_BYTES
            || encoded[0..8] != BOUNDARY_MAGIC
        {
            return Err(ReactionDiffusionPartitionRefusal::MalformedBoundaryLength);
        }
        if read_u32(encoded, 8)? != NUMERIC_PROFILE_TAG || encoded[45] != 0 {
            return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryProfile);
        }
        let count = usize::from(read_u16(encoded, 48)?);
        let expected = REACTION_DIFFUSION_BOUNDARY_HEADER_BYTES + count * 8;
        if count != 1 || encoded.len() != expected {
            return Err(ReactionDiffusionPartitionRefusal::MalformedBoundaryLength);
        }
        let mut field_id = [0; 16];
        field_id.copy_from_slice(&encoded[12..28]);
        let edge = match encoded[44] {
            0 => ReactionDiffusionBoundaryEdge::North,
            1 => ReactionDiffusionBoundaryEdge::South,
            2 => ReactionDiffusionBoundaryEdge::West,
            3 => ReactionDiffusionBoundaryEdge::East,
            _ => return Err(ReactionDiffusionPartitionRefusal::WrongBoundaryEdge),
        };
        let mut values = Vec::with_capacity(count);
        for offset in (REACTION_DIFFUSION_BOUNDARY_HEADER_BYTES..expected).step_by(8) {
            values.push(
                ReactionDiffusionCell::new(
                    read_u32(encoded, offset)?,
                    read_u32(encoded, offset + 4)?,
                )
                .map_err(ReactionDiffusionPartitionRefusal::Field)?,
            );
        }
        Ok(Self {
            boundary_id: read_u32(encoded, 36)?,
            field_id: ReactionDiffusionFieldId(field_id),
            generation: read_u64(encoded, 28)?,
            source_region: ReactionDiffusionRegionId(read_u16(encoded, 40)?),
            destination_region: ReactionDiffusionRegionId(read_u16(encoded, 42)?),
            destination_edge: edge,
            destination_offset: read_u16(encoded, 46)?,
            values,
        })
    }
}

pub(crate) fn edge_tag(edge: ReactionDiffusionBoundaryEdge) -> u8 {
    match edge {
        ReactionDiffusionBoundaryEdge::North => 0,
        ReactionDiffusionBoundaryEdge::South => 1,
        ReactionDiffusionBoundaryEdge::West => 2,
        ReactionDiffusionBoundaryEdge::East => 3,
    }
}

fn read_u16(encoded: &[u8], offset: usize) -> Result<u16, ReactionDiffusionPartitionRefusal> {
    encoded
        .get(offset..offset + 2)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u16::from_le_bytes)
        .ok_or(ReactionDiffusionPartitionRefusal::MalformedBoundaryLength)
}

fn read_u32(encoded: &[u8], offset: usize) -> Result<u32, ReactionDiffusionPartitionRefusal> {
    encoded
        .get(offset..offset + 4)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u32::from_le_bytes)
        .ok_or(ReactionDiffusionPartitionRefusal::MalformedBoundaryLength)
}

fn read_u64(encoded: &[u8], offset: usize) -> Result<u64, ReactionDiffusionPartitionRefusal> {
    encoded
        .get(offset..offset + 8)
        .and_then(|bytes| bytes.try_into().ok())
        .map(u64::from_le_bytes)
        .ok_or(ReactionDiffusionPartitionRefusal::MalformedBoundaryLength)
}
