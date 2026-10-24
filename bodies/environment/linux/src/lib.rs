//! Linux recorder realization beneath the ordinary kernel and retained Root
//! owner. Preparation and matching identity data create no start or authority.
mod recovery;
pub use recovery::{FileRecoveryCompletion, FileRecoveryRefusal, PreparedFileRecovery};

mod batch;
pub use batch::{FileBatchSoftware, PreparedFileBatchRecovery};

mod catalog;
pub use catalog::{FileCatalogRefusal, PreparedFileCatalogRecovery};

#[cfg(feature = "installed-seed")]
mod seed;
#[cfg(feature = "installed-seed")]
pub use seed::{recover_durable_catalog_seed, DurableSeedRecoveryRefusal};

mod publication;
pub use publication::{
    FilePublicationCompletion, FilePublicationRefusal, PreparedFileRecordingPublication,
};

mod publication_offer;
pub use publication_offer::{
    recording_publication_offer, RECORDING_PUBLICATION_ARTIFACT,
    RECORDING_PUBLICATION_IMPLEMENTATION, RECORDING_PUBLICATION_PROFILE,
};

mod publication_face;
pub use publication_face::{FilePublicationFaceRefusal, PreparedFilePublicationFace};
