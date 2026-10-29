//! Phased checked installation with optional original preallocated storage.
use super::*;
impl NativePlay {
    /// Storage conveys no authority; all scope and admission checks remain shared.
    pub(crate) fn install_linked_phased_held(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        generation: c::OfferGeneration,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
        admit: impl FnOnce() -> Result<(), ()>,
    ) -> Result<(), ()> {
        self.install_linked_phased_mode(
            image, expected, generation, source, join, issuer, None, admit,
        )
    }
    /// Storage conveys no authority; all scope and admission checks remain shared.
    pub(crate) fn install_linked_phased_held_with_storage(
        &mut self,
        image: &conduit_plot::flat_image::indexed::IndexedSourceImage<'_>,
        expected: &c::PlotIdentity,
        generation: c::OfferGeneration,
        source: &mut Option<ReadyCredentialBase<Source>>,
        join: &mut Option<ReadyNetworkJoinBase<Provider>>,
        issuer: &mut RootIssuer,
        storage: n::PreparedNetworkKernelStorage,
        admit: impl FnOnce() -> Result<(), ()>,
    ) -> Result<(), ()> {
        self.install_linked_phased_mode(
            image,
            expected,
            generation,
            source,
            join,
            issuer,
            Some(storage),
            admit,
        )
    }
}
