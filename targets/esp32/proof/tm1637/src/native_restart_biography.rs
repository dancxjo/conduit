//! Source-bound biography export. No decoded DTO or caller-supplied digest
//! constructs an offer, and reading an offer never releases producer custody.
use super::*;
use ed25519_dalek::Signer;
use payload::wire::{Hex, Offer, Summary};

/// Minted only after the current complete affine snapshot is authenticated.
/// Kept opaque so later transport cannot substitute an arbitrary source scope.
pub(crate) struct Export {
    offer: Offer,
}
impl Export {
    pub(crate) fn offer(&self) -> &Offer {
        &self.offer
    }
}
fn cumulative(summary: &payload::archive::Summary) -> Summary {
    Summary {
        count: summary.count,
        head: summary.head.map(Hex),
        last_sequence: summary.last_sequence,
        last_sign: summary.last_sign.as_ref().map(|s| s.as_str().into()),
    }
}
/// Caller owns the actual Store for the whole operation. This reads the full
/// private source but hashes or emits only the previously validated array span.
fn stream(
    store: &mut Store,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<[u8; 32], &'static str> {
    if store.failed {
        return Err("biography-restart-failed");
    }
    let custody = store.custody.as_ref().ok_or("biography-source-missing")?;
    let mut hash = Sha256::new();
    store
        .journal
        .read_snapshot(&custody.source, |reader| {
            let mut read_refusal = None;
            let result = payload::export_archive_array(
                |out| {
                    reader.read(out).map_err(|error| {
                        read_refusal = Some(error);
                    })
                },
                custody.span,
                &mut |bytes| {
                    sink(bytes)?;
                    hash.update(bytes);
                    Ok(())
                },
            );
            result.map_err(|error| read_refusal.unwrap_or_else(|| stream_refusal(error)))
        })
        .map_err(|_| "biography-source-read")?;
    Ok(hash.finalize().into())
}
pub(super) fn prepare_store(store: &mut Store) -> Result<Export, &'static str> {
    let stream_hash = stream(store, &mut |_| Ok(()))?;
    let custody = store.custody.as_ref().ok_or("biography-source-missing")?;
    if custody.local_count == 0 {
        return Err("biography-no-local-archives");
    }
    let body = custody
        .summary
        .body
        .as_ref()
        .ok_or("biography-body-missing")?;
    let body_binding: [u8; 32] = Sha256::digest(body.as_str().as_bytes()).into();
    if body_binding != store.binding {
        return Err("biography-source-binding");
    }
    let key =
        ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("biography-key-missing")?);
    let mut offer = Offer {
        schema: payload::wire::OFFER.into(),
        body: body.clone(),
        generation: custody.source.generation(),
        source: Hex(*custody.source.digest()),
        base: custody
            .prefix
            .as_ref()
            .map_or_else(Summary::default, |p| p.summary.clone()),
        end: cumulative(&custody.summary),
        stream: Hex(stream_hash),
        signature: Hex([0; 64]),
    };
    if offer.end.count.checked_sub(offer.base.count) != Some(custody.local_count) {
        return Err("biography-source-count");
    }
    let root = key.verifying_key();
    if let Some(prefix) = custody.prefix.as_ref() {
        prefix.verify(body, &root)?;
    }
    offer.signature = Hex(key.sign(&offer.transcript(&root.to_bytes())).to_bytes());
    offer.verify(body, &root)?;
    Ok(Export { offer })
}
pub(crate) fn prepare() -> Result<Export, &'static str> {
    with_store(prepare_store)
}
/// A changed source refuses even if its Body and archive head are unchanged.
/// The receiver must verify the offered digest before treating any bytes as
/// durable custody; an interrupted sink leaves producer custody untouched.
pub(crate) fn export(
    offered: &Export,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<(), &'static str> {
    with_store(|store| export_store(store, offered, sink))
}
fn export_store(
    store: &mut Store,
    offered: &Export,
    sink: &mut dyn FnMut(&[u8]) -> Result<(), Refusal>,
) -> Result<(), &'static str> {
    let current = prepare_store(store)?;
    let root =
        ed25519_dalek::SigningKey::from_bytes(store.seed.as_ref().ok_or("biography-key-missing")?)
            .verifying_key();
    if current.offer.transcript(&root.to_bytes()) != offered.offer.transcript(&root.to_bytes()) {
        return Err("biography-source-changed");
    }
    if stream(store, sink)? != offered.offer.stream.0 {
        return Err("biography-stream-changed");
    }
    Ok(())
}
#[cfg(test)]
#[path = "native_restart_biography_tests.rs"]
mod tests;

/// Format-only migration of cold custody, before admitting this hardware Boot.
/// The receipt must already be endorsed by this Root's typed ACK signer. Replay
/// against another generation or source is refused by the recomputed offer ID.
pub(crate) fn retain_receipt(receipt: payload::wire::Receipt) -> Result<(), &'static str> {
    with_store(|store| retain_store_receipt(store, receipt))
}
fn retain_store_receipt(
    store: &mut Store,
    receipt: payload::wire::Receipt,
) -> Result<(), &'static str> {
    if store.ready || store.parked || store.evidence.is_none() {
        return Err("biography-requires-cold-custody");
    }
    let offered = prepare_store(store)?;
    let mut seed = PrivateSeed(*store.seed.as_ref().ok_or("biography-key-missing")?);
    let key = ed25519_dalek::SigningKey::from_bytes(&seed.0).verifying_key();
    receipt.verify(&offered.offer.body, &key)?;
    if receipt.offer.0 != offered.offer.transcript(&key.to_bytes())
        || receipt.summary != offered.offer.end
    {
        return Err("biography-receipt-source");
    }
    let previous = store.custody.as_ref().ok_or("biography-source-missing")?;
    // Decode the ORIGINAL admissions from the affine source, not the in-memory
    // manager whose unfinished challenges normal boot initialization retired.
    let resident = store
        .journal
        .read_snapshot(&previous.source, |reader| {
            payload::decode_resident_with_prefix(
                |out| reader.read(out).map_err(|_| ()),
                &offered.offer.body,
                &key.to_bytes(),
                Some(payload::archive::Custody {
                    summary: &previous.summary,
                    prefix: previous.prefix.as_ref(),
                    local_count: previous.local_count,
                }),
            )
            .map_err(|_| Refusal::Corrupt)
        })
        .map_err(|_| "biography-original-source")?;
    let admissions = resident
        .retained
        .admissions
        .as_ref()
        .ok_or("biography-admissions-missing")?;
    let mut metadata = None;
    let source = store
        .journal
        .rewrite_streamed(&previous.source, &store.binding, &seed.0, |io| {
            let mut read_refusal = None;
            let next = payload::stream_rewrite_with_prefix(
                &resident.retained.evidence,
                &[],
                admissions,
                &previous.summary,
                previous.span,
                Some(&receipt),
                0,
                &key.to_bytes(),
                |out| {
                    io.read_source(out).map_err(|error| {
                        read_refusal = Some(error);
                    })
                },
                &mut |bytes| io.write(bytes),
            )
            .map_err(|error| read_refusal.unwrap_or_else(|| stream_refusal(error)))?;
            if metadata.as_ref().is_some_and(|old| old != &next) {
                return Err(Refusal::Corrupt);
            }
            metadata = Some(next);
            Ok(())
        })
        .map_err(|_| "biography-rewrite")?;
    seed.0.fill(0);
    let (summary, span) = metadata.ok_or("biography-rewrite-metadata")?;
    // rewrite_streamed authenticated the original source on both passes and
    // physically read back the entire committed successor before returning.
    store.custody = Some(ArchiveCustody {
        source,
        summary,
        span,
        prefix: Some(receipt),
        local_count: 0,
    });
    Ok(())
}

/// Seed-derived public pin from initialized restart custody, usable before a
/// new hardware Boot is admitted. This confers no Body membership or execution.
pub(crate) fn root_key() -> Result<[u8; 32], &'static str> {
    with_store(|store| {
        if store.failed {
            return Err("biography-restart-failed");
        }
        Ok(ed25519_dalek::SigningKey::from_bytes(
            store.seed.as_ref().ok_or("biography-key-missing")?,
        )
        .verifying_key()
        .to_bytes())
    })
}
