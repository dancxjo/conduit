//! Inspect custody metadata before granting any ordinary erased-data view.
//! A pending erase retains its affine owner; ordinary boot never resumes it.
use super::{custody_acceptance, custody_journal as journal, ArchiveRegion};
use journal::{Custody, Error, Opened, WholeEpoch};

pub(super) fn prepare(
    region: ArchiveRegion,
    work: &mut [u8; journal::WORK],
) -> Result<Opened<ArchiveRegion>, &'static str> {
    // A genuinely fresh Body has no key yet. It may retain blank metadata,
    // but cannot authenticate any existing reclaim transaction.
    let pinned = crate::native_restart::observed::pinned_scope().ok();
    let verify = |bytes: &[u8; journal::ACCEPTANCE]| {
        pinned
            .as_ref()
            .is_some_and(|(body, root)| custody_acceptance::verify_accepted(bytes, body, root))
    };
    let custody = match Custody::open(region, work, verify) {
        Ok(custody) => custody,
        Err((region, Error::UnknownMetadata)) => {
            let (body, root) = pinned.as_ref().ok_or("observed-layout-root")?;
            let receipt = crate::native_restart::observed::read_layout_receipt(&region.identity())?
                .ok_or("observed-layout-missing")?;
            let established = receipt
                .verify(body, root, &region)
                .map_err(|_| "observed-layout-verification")?;
            Custody::open_established(region, established, work, verify)
                .map_err(|_| "observed-custody-format")?
        }
        Err(_) => return Err("observed-custody-metadata"),
    };
    custody.classify().map_err(|_| "observed-custody-readback")
}
