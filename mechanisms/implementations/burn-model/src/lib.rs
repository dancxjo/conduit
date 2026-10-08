#![doc = include_str!("../../../../wiki/Creating-models.md")]
//! Hosted Burn realization of Conduit's existing model/training contracts.
//! Burn tensors are host-local implementation machinery, never portable values.
mod contract;
mod error;
pub use contract::*;
pub use error::Error;
mod device;
mod model;
mod recipe;
mod record_validation;
pub use device::DeviceRequest;
pub use model::*;
pub use recipe::OptimizerRecipe;
mod prepared_model;
pub use prepared_model::PreparedBurnModel;
mod adapter;
mod training_step;
pub use adapter::{BurnAdapter, TrainingContext};
mod cancellation;
pub use cancellation::Cancellation;
mod checkpoint_manifest;
mod inference;
pub use inference::{InferenceBurnAdapter, InferenceContext};
mod checkpoint;
mod checkpoint_store;
pub use checkpoint_store::DirectoryCheckpointStore;

/// Pinned host-side authoring library; never a portable value representation.
pub use burn;
