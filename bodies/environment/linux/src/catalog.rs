//! Catalogue discovery during preparation, before constructing a recovery Source.
//! Root supplies current independent read authority and actual directory custody.
use conduit_core::{AuthorityBinding, BoundedResourceRef, PlannedGear};
use conduit_environment_runtime::recording::{
    PreparedRecordingCatalogReader, RecordingCatalogRefusal,
};
use conduit_linux_snapshot_store::{FileSnapshotRefusal, PreparedFileSnapshot, SnapshotDirectory};

#[derive(Debug)]
pub enum FileCatalogRefusal {
    Storage(FileSnapshotRefusal),
    Catalog(RecordingCatalogRefusal),
}

/// Retains the exact selected file reader and bounded domain validator. This
/// discovers references, not authority, committed sample history or a Body seed.
/// Root must admit this memory and all later read placements before Play.
pub struct PreparedFileCatalogRecovery<'a> {
    snapshot: PreparedFileSnapshot<'a>,
    catalog: PreparedRecordingCatalogReader,
}
impl<'a> PreparedFileCatalogRecovery<'a> {
    pub fn prepare(
        directory: &'a SnapshotDirectory,
        placement: &PlannedGear,
        reference: &BoundedResourceRef,
    ) -> Result<Self, FileCatalogRefusal> {
        Ok(Self {
            snapshot: PreparedFileSnapshot::prepare(directory, placement, reference)
                .map_err(FileCatalogRefusal::Storage)?,
            catalog: PreparedRecordingCatalogReader::default(),
        })
    }
    /// No heap allocation. No references are returned on file, envelope, UTF-8
    /// or catalogue refusal. Decoding owned references and Source preparation
    /// happen afterwards; this method does not step a second execution kernel.
    pub fn read(
        &mut self,
        authority: &AuthorityBinding,
    ) -> Result<&PreparedRecordingCatalogReader, FileCatalogRefusal> {
        let content = self
            .snapshot
            .read(authority)
            .map_err(FileCatalogRefusal::Storage)?;
        let content = core::str::from_utf8(content)
            .map_err(|_| FileCatalogRefusal::Catalog(RecordingCatalogRefusal::Encoding))?;
        self.catalog
            .read(content)
            .map_err(FileCatalogRefusal::Catalog)?;
        Ok(&self.catalog)
    }
}
