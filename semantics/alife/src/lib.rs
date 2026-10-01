#![no_std]

extern crate alloc;

#[allow(clippy::clone_on_copy, dead_code)]
mod generated {
    include!(concat!(env!("OUT_DIR"), "/semantic_types.rs"));

    impl ReactionDiffusionFieldId {
        pub const fn from_bytes(value: [u8; 16]) -> Self {
            Self(value)
        }
    }

    impl LeniaFieldId {
        pub const fn from_bytes(value: [u8; 16]) -> Self {
            Self(value)
        }
    }
}
pub use generated::{
    FieldBitmapRefusal, GardenClockObservation, GardenContactObservation,
    GardenEnrichedObservation, GardenEvolutionRefusal, GardenState, GrayScottParameters,
    LeniaBoundary, LeniaFieldId, LeniaParameters, LeniaRegionChunkKind, LeniaRegionChunkKindCode,
    LeniaValueRefusal, ReactionDiffusionBoundaryEdge, ReactionDiffusionBoundaryEdgeCode,
    ReactionDiffusionCell, ReactionDiffusionEvolveRequest, ReactionDiffusionFieldId,
    ReactionDiffusionPartition, ReactionDiffusionRegion, ReactionDiffusionRegionId,
    ReactionDiffusionValueRefusal, ReactionDiffusionValueRefusalStaleGeneration,
    ReactionDiffusionValueRefusalWorkLimitExceeded,
};

mod distributed_catalog;
mod distributed_expansion;
mod field_bitmap;
mod lenia;
mod lenia_catalog;
mod lenia_evolution;
mod lenia_line_frame;
mod lenia_orbium;
mod lenia_partition;
mod lenia_region_wire;
mod lenia_region_worker;
mod reaction_diffusion;
mod reaction_diffusion_boundary_codec;
mod reaction_diffusion_catalog;
mod reaction_diffusion_evolution;
mod reaction_diffusion_partition;
mod reaction_diffusion_partition_bounds;
mod reaction_diffusion_partition_join;
mod reaction_diffusion_region_work;

pub use distributed_catalog::*;
pub use distributed_expansion::*;
pub use field_bitmap::*;
pub use lenia::*;
pub use lenia_catalog::*;
pub use lenia_line_frame::*;
pub use lenia_orbium::*;
pub use lenia_partition::*;
pub use lenia_region_wire::*;
pub use lenia_region_worker::*;
pub use reaction_diffusion::*;
pub use reaction_diffusion_catalog::*;
pub use reaction_diffusion_partition::*;
pub use reaction_diffusion_partition_bounds::*;
pub use reaction_diffusion_region_work::*;
