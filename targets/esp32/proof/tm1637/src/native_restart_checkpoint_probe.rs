//! Read-only replay dimensions from the actual owner and authenticated source.
//! No time, allocation, scheduling or execution authority is established here.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ProbeReport {
    pub source_generation: u64,
    pub source_bytes: usize,
    pub output_bytes: usize,
}
pub(crate) fn probe(
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
) -> Result<ProbeReport, &'static str> {
    let mut lease = take_store()?;
    let store = lease.0.as_mut().ok_or("restart-not-ready")?;
    if store.failed || store.parked || !store.ready {
        return Err("checkpoint-probe-source");
    }
    let seed = store.seed.as_ref().ok_or("restart-key-missing")?;
    if ed25519_dalek::SigningKey::from_bytes(seed)
        .verifying_key()
        .as_bytes()
        != root
    {
        return Err("restart-signer-mismatch");
    }
    let previous = store.custody.as_ref().ok_or("restart-custody-missing")?;
    #[cfg(test)]
    let original = (
        previous.source.identity(),
        &previous.source as *const _,
        previous.layouts.backing_identity(),
        previous.prefix.as_ref().map(|v| v as *const _),
        previous.proof.as_ref().map(|v| v as *const _),
        store.admissions.as_ref().map(|v| v as *const _),
    );
    let prepared = prepare(previous, owner, store.admissions.as_ref(), root)?;
    // Identical production preparation: complete authenticated source replay,
    // output digest and extent calculation. Never execute this writer.
    let writer = IncrementalRewrite::prepare(
        &mut store.journal,
        &previous.source,
        &store.binding,
        seed,
        |reader, sink| emit(&prepared, reader, sink).map(|_| ()),
    )
    .map_err(checkpoint_commit_refusal)?;
    #[cfg(test)]
    assert!(
        original
            == (
                previous.source.identity(),
                &previous.source as *const _,
                previous.layouts.backing_identity(),
                previous.prefix.as_ref().map(|v| v as *const _),
                previous.proof.as_ref().map(|v| v as *const _),
                store.admissions.as_ref().map(|v| v as *const _),
            )
    );
    Ok(ProbeReport {
        source_generation: previous.source.generation(),
        source_bytes: previous.source.payload_length(),
        output_bytes: writer.output_bytes(),
    })
}

#[cfg(any(test, feature = "checkpoint-preparation-check"))]
#[path = "native_restart_frozen_probe.rs"]
mod frozen;
#[cfg(any(test, feature = "checkpoint-preparation-check"))]
pub(crate) use frozen::{probe_frozen, FrozenProbeReport};
