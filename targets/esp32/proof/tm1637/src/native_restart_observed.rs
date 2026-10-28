//! Typed cold history custody signing and durable layout metadata. No execution
//! or membership authority is created by these entrances.
use super::*;
use crate::native_observed_archive::{custody_acceptance as acceptance, custody_journal::layout};
use ed25519_dalek::Signer;
fn signing_key(store: &Store, root: &[u8; 32]) -> Result<ed25519_dalek::SigningKey, &'static str> {
    if store.failed || store.ready || store.parked {
        return Err("observed-restart-failed");
    }
    let key =
        ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("observed-restart-key")?);
    if key.verifying_key().as_bytes() != root {
        return Err("observed-restart-root");
    }
    Ok(key)
}
pub(crate) fn pinned_scope(
) -> Result<(conduit_body::BodyId, ed25519_dalek::VerifyingKey), &'static str> {
    with_store(|store| {
        if store.failed || store.ready || store.parked {
            return Err("observed-requires-cold-custody");
        }
        let body = store
            .evidence
            .as_ref()
            .ok_or("observed-cold-evidence")?
            .body_id
            .clone();
        let key = ed25519_dalek::SigningKey::from_bytes(
            store.seed.as_ref().ok_or("observed-restart-key")?,
        )
        .verifying_key();
        Ok((body, key))
    })
}
pub(crate) fn sign_layout(draft: layout::Draft) -> Result<layout::Receipt, &'static str> {
    with_store(|store| {
        let key = signing_key(store, draft.root())?;
        let signature = key.sign(&draft.transcript()).to_bytes();
        let receipt = draft.signed(signature);
        receipt
            .validate(
                &store
                    .evidence
                    .as_ref()
                    .ok_or("observed-cold-evidence")?
                    .body_id,
                &key.verifying_key(),
            )
            .map_err(|_| "observed-layout-body")?;
        Ok(receipt)
    })
}
pub(crate) fn sign_offer(
    draft: acceptance::ClosedOfferDraft,
) -> Result<acceptance::wire::Offer<'static>, &'static str> {
    with_store(|store| {
        let key = signing_key(store, draft.root())?;
        let signature = key.sign(&draft.transcript()).to_bytes();
        let offer = draft.signed(signature);
        if offer.scope.0[..32] != store.binding {
            return Err("observed-offer-body");
        }
        Ok(offer)
    })
}
pub(crate) fn acknowledge(
    draft: &acceptance::ClosedOfferDraft,
    request: &acceptance::wire::Request<'_>,
) -> Result<[u8; 416], &'static str> {
    with_store(|store| {
        let key = signing_key(store, draft.root())?;
        let body = store
            .evidence
            .as_ref()
            .ok_or("observed-cold-evidence")?
            .body_id
            .clone();
        let mut seed = PrivateSeed([0; 32]);
        let (resident, _) = decode_checkpoint(
            &mut store.journal,
            &store.binding,
            &body,
            &mut seed.0,
            store.custody.as_ref(),
        )?;
        let accepted = acceptance::verify_ack(
            draft,
            resident
                .retained
                .admissions
                .as_ref()
                .ok_or("observed-persisted-admissions")?,
            &resident.retained.evidence.membership,
            request,
        )?;
        if accepted.root() != key.verifying_key().as_bytes() {
            return Err("observed-accepted-root");
        }
        let signature = key.sign(&accepted.transcript()).to_bytes();
        Ok(accepted.signed(signature))
    })
}
/// Rewrite the original authenticated cold envelope without changing its
/// admissions or evidence. Return only bytes physically reread after commit.
pub(crate) fn persist_layout_receipt(
    receipt: layout::Receipt,
) -> Result<layout::Receipt, &'static str> {
    with_store(|store| persist(store, receipt))
}
pub(super) fn persist(
    store: &mut Store,
    receipt: layout::Receipt,
) -> Result<layout::Receipt, &'static str> {
    if store.ready || store.parked || store.failed {
        return Err("layout-requires-cold-custody");
    }
    let body = store
        .evidence
        .as_ref()
        .ok_or("layout-cold-evidence")?
        .body_id
        .clone();
    let mut seed = PrivateSeed(*store.seed.as_ref().ok_or("layout-key")?);
    let root = ed25519_dalek::SigningKey::from_bytes(&seed.0).verifying_key();
    receipt
        .validate(&body, &root)
        .map_err(|_| "layout-receipt")?;
    let region: [u8; 32] = *receipt.region_identity();
    let previous = store.custody.as_ref().ok_or("layout-custody")?;
    let (resident, _) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &body,
        &mut seed.0,
        Some(previous),
    )?;
    let mut layouts = resident.layouts.clone();
    layouts.add(&receipt)?;
    let admissions = resident
        .retained
        .admissions
        .as_ref()
        .ok_or("layout-persisted-admissions")?;
    let source = store
        .journal
        .rewrite_streamed(&previous.source, &store.binding, &seed.0, |io| {
            payload::stream_rewrite_with_layout(
                &resident.retained.evidence,
                &[],
                admissions,
                &previous.summary,
                previous.span,
                previous.prefix.as_ref(),
                previous.local_count,
                &root.to_bytes(),
                &layouts,
                |out| io.read_source(out).map_err(|_| ()),
                &mut |bytes| io.write(bytes),
            )
            .map(|_| ())
            .map_err(stream_refusal)
        })
        .map_err(|_| "layout-rewrite")?;
    drop(resident);
    // Independent authenticated decode of the committed successor, after the
    // journal's physical readback, validates every receipt against this pin.
    let (checked, checked_source) =
        decode_checkpoint(&mut store.journal, &store.binding, &body, &mut seed.0, None)
            .inspect_err(|_| {
                store.failed = true;
            })?;
    if checked.layouts != layouts || checked_source.generation() != source.generation() {
        store.failed = true;
        return Err("layout-readback");
    }
    let durable = checked
        .layouts
        .find(&region)
        .ok_or("layout-readback-missing")?;
    store.custody = Some(ArchiveCustody {
        proof: checked.proof,
        source: checked_source,
        summary: checked.summary,
        span: checked.span,
        prefix: checked.prefix,
        local_count: checked.local_count,
        layouts: checked.layouts,
    });
    Ok(durable)
}
pub(crate) fn read_layout_receipt(
    region: &[u8; 32],
) -> Result<Option<layout::Receipt>, &'static str> {
    with_store(|store| {
        if store.failed {
            return Err("layout-restart-failed");
        }
        let body = store
            .evidence
            .as_ref()
            .ok_or("layout-cold-evidence")?
            .body_id
            .clone();
        let mut seed = PrivateSeed([0; 32]);
        let (resident, _) = decode_checkpoint(
            &mut store.journal,
            &store.binding,
            &body,
            &mut seed.0,
            store.custody.as_ref(),
        )?;
        Ok(resident.layouts.find(region))
    })
}
#[cfg(test)]
#[path = "native_restart_observed_tests.rs"]
pub(super) mod tests;

/// Validate live custody without decoding another Body or creating a receipt.
/// Actual Root session possession remains the crate-private caller's obligation.
pub(crate) fn check_live_owner(
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
) -> Result<(), &'static str> {
    with_store(|store| {
        live_custody(store, owner)?;
        let key =
            ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("observed-live-key")?);
        if key.verifying_key().as_bytes() != root {
            return Err("observed-live-root");
        }
        Ok(())
    })
}
fn live_custody<'a>(
    store: &'a Store,
    owner: &SealedBodyLifecycleSession,
) -> Result<&'a ArchiveCustody, &'static str> {
    if store.failed || store.parked || !store.ready {
        return Err("observed-requires-live-custody");
    }
    let evidence = owner.evidence();
    if <[u8; 32]>::from(Sha256::digest(evidence.body_id.as_str().as_bytes())) != store.binding
        || evidence.body.body_id != evidence.body_id
        || evidence.membership.body_id != evidence.body_id
        || !owner.pending_archives().is_empty()
    {
        return Err("observed-live-body");
    }
    let custody = store.custody.as_ref().ok_or("observed-live-source")?;
    if custody.source.binding() != &store.binding
        || Some(custody.source.generation()) != store.journal.generation()
    {
        return Err("observed-live-source");
    }
    Ok(custody)
}
/// Cached receipts were fully authenticated on Boot and carried through every
/// committed/readback checkpoint. This read grants no formatting, signing,
/// persistence, region possession or erase authority, and performs no I/O.
pub(crate) fn read_live_layout_receipt(
    owner: &SealedBodyLifecycleSession,
    region: &[u8; 32],
) -> Result<layout::Receipt, &'static str> {
    with_store(|store| {
        live_custody(store, owner)?
            .layouts
            .find(region)
            .ok_or("observed-live-layout-missing")
    })
}

#[cfg(test)]
#[path = "native_restart_observed_live_tests.rs"]
mod live_tests;
