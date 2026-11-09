//! Cold, no-erase prefix transaction beneath the original physical FLASH owner.
use super::*;
use conduit_resource_snapshot::nor::NorStorage;
const PAGE: usize = 256;
impl<const N: usize> Root<'_, N> {
    /// Commit only a permission selected by this original Root, then verify all
    /// bytes and both signatures. No resource admission, provider bind or Start.
    /// All uncertain effects remain explicit; this never retries or erases.
    ///
    /// # Safety
    /// The caller owns the sole actual checked FLASH owner and this exact prefix,
    /// before actors start or while separately admitted cold maintenance excludes
    /// all timing/flash activity. Root must pre-fund both8192B workspaces, the
    /// adapter's256B bounce, bounded decode allocations and up to32 page programs
    /// plus readback. NorStorage/geometry and this method create no flash permit.
    /// The storage prefix must be the target-reserved continuation prefix, not a
    /// capability/descriptor reconstructed from incoming metadata.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn commit_continuation_cold<S: NorStorage>(
        &self,
        prepared: PreparedContinuationInstall,
        storage: &mut S,
        candidate: &[u8; PREFIX_BYTES],
        readback: &mut [u8; PREFIX_BYTES],
        write_image: &mut [u8; PREFIX_BYTES],
    ) -> Result<InstalledContinuationAcceptance, E> {
        self.check_anchor(&prepared.anchor)?;
        if self.receipt.as_ref() != Some(&prepared.anchor.credential)
            || self.input.is_retained()
            || !self.resources.admissions().is_empty()
            || !self.resources.assignments().is_empty()
            || storage.capacity() < PREFIX_BYTES as u32
            || storage.program_unit() != PAGE as u32
            || Sha256::digest(&candidate[..ARTIFACT_BYTES]).as_slice()
                != prepared.anchor.artifact_digest
        {
            return Err(E::Scope);
        }
        write_image[..ARTIFACT_BYTES].copy_from_slice(&candidate[..ARTIFACT_BYTES]);
        encode_anchor(
            &prepared.anchor,
            (&mut write_image[ARTIFACT_BYTES..])
                .try_into()
                .map_err(|_| E::Bounds)?,
        )?;
        storage.read(0, readback).map_err(|_| E::Media)?;
        let original_matches = match prepared.previous {
            Some(digest) => Sha256::digest(&readback[..]).as_slice() == digest,
            None => readback.iter().all(|b| *b == 0xff),
        };
        if !original_matches {
            return Err(E::Uncertain);
        }
        if readback
            .iter()
            .zip(write_image.iter())
            .any(|(old, new)| old & new != *new)
        {
            return Err(E::RequiresErase);
        }
        // Artifact first, acceptance last. The committed footer is in the final
        // actual256B page; partial marker/JSON/signature can never activate it.
        for (index, page) in write_image.chunks_exact(PAGE).enumerate() {
            let at = index * PAGE;
            if readback[at..at + PAGE] != *page {
                storage.program(at as u32, page).map_err(|_| E::Uncertain)?;
            }
        }
        storage.read(0, readback).map_err(|_| E::Uncertain)?;
        if readback.as_slice() != write_image.as_slice() {
            return Err(E::Uncertain);
        }
        self.read_continuation_acceptance(readback, write_image)
    }
}
