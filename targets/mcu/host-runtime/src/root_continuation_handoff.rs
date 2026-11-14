//! Existing bounded document carriage for original Root-selected cold installs.
use super::*;
impl<const N: usize> Root<'_, N> {
    /// The owning platform supplies a whole-transaction deadline and funds these
    /// three fixed buffers before calling. This performs NO flash access. An
    /// authenticated current return, signed owner intent, exact installed pins
    /// and monotonic local acceptance are required by the shared selector.
    #[allow(clippy::too_many_arguments)]
    pub async fn receive_continuation_installation<T: DocumentTransport>(
        &mut self,
        link: &mut T,
        source_digest: [u8; 32],
        pins: &[b::ContinuationBundleEntry<'_>],
        previous: Option<&InstalledContinuationAcceptance>,
        candidate: &mut [u8; PREFIX_BYTES],
        selected: &mut [u8; PREFIX_BYTES],
        scratch: &mut [u8; PREFIX_BYTES],
    ) -> Result<(), E> {
        if self.config.execution.and_then(|p| p.continuation).is_none() {
            return Err(E::Scope);
        }
        if N < crate::transport::LIMIT
            || self
                .config
                .preparation
                .is_none_or(|p| p.maximum_assignment_bytes < PREFIX_BYTES as u32)
        {
            return Err(E::Bounds);
        }
        let mut document = conduit_wire::BorrowedBoundedDocumentAssembly::new(candidate, 32);
        link.receive_document(&mut document)
            .await
            .map_err(|_| E::Carrier)?;
        let (bytes, digest) = document.completed().ok_or(E::Encoding)?;
        if bytes.len() != PREFIX_BYTES || Sha256::digest(bytes).as_slice() != digest {
            return Err(E::Encoding);
        }
        let candidate: &[u8; PREFIX_BYTES] = bytes.try_into().map_err(|_| E::Bounds)?;
        let prepared = self.prepare_continuation_install_digest(
            candidate,
            scratch,
            source_digest,
            pins,
            previous,
        )?;
        prepared.encode_selected_prefix(candidate, selected)?;
        let digest: [u8; 32] = Sha256::digest(&selected[..]).into();
        let frame = &mut self.input.writable().map_err(|_| E::Scope)?[..crate::transport::LIMIT];
        let mut offset = 0;
        while offset < PREFIX_BYTES {
            let (n, next) =
                conduit_wire::encode_bounded_document_chunk(frame, selected, offset, &digest)
                    .map_err(|_| E::Bounds)?;
            link.send(&frame[..n]).await.map_err(|_| E::Carrier)?;
            offset = next;
        }
        Ok(())
    }
}
