//! Host-side checked construction material; no provider or authority is minted.
extern crate alloc;
mod body_image;
mod environment_body_image;
pub use body_image::{BodyImageRefusal, PreparedBodyImage};
pub use environment_body_image::{EnvironmentBodyImage, EnvironmentBodyImageRefusal};
mod display_output;
pub use display_output::{DisplayOutputSource, DisplayPlanningRefusal, PreparedDisplayOutput};

mod source_recipe;
pub use source_recipe::{NativeSoftwareSourceRecipe, SourceRecipeRefusal};

mod display_material;
pub use display_material::DisplaySourceMaterial;
mod network_source;
mod network_source_material;
pub use conduit_environment_runtime::construction::compile_default as compile_appliance_source;
pub use network_source::{compile_network_scan, compile_network_setup};
pub use network_source_material::NetworkSourceMaterial;

mod mask_source;
pub use mask_source::{SevenSegmentMaskMaterial, SevenSegmentMaskSource};

mod recording_recovery;
pub use recording_recovery::{
    PreparedRecordingRecovery, RecordingPlanningRefusal, RecordingRecoverySource,
};
