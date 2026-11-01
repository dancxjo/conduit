//! One bounded read-only startup measurement. It never stages membership,
//! advances a writer, releases a receipt, or changes the admitted workset.
use super::*;
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FrozenProbeReport {
    pub source_generation: u64,
    pub source_bytes: usize,
    pub output_bytes: usize,
    pub prepare_us: u64,
    pub capture_us: u64,
    pub retained_cache_bytes: usize,
    /// Current Root heap use and Boot-lifetime heap peak, not phase-local peaks
    /// or complete RAM accounting (stacks/static arenas are outside this).
    pub heap_before: [usize; 2],
    pub heap_after_prepare: [usize; 2],
    pub heap_after_capture: [usize; 2],
    pub heap_after_release: [usize; 2],
}
pub(crate) fn probe_frozen(
    owner: &SealedBodyLifecycleSession,
    root: &[u8; 32],
    mut clock_us: impl FnMut() -> u64,
    mut heap: impl FnMut() -> [usize; 2],
) -> Result<FrozenProbeReport, &'static str> {
    let mut lease = take_store()?;
    let store = lease.0.as_mut().ok_or("restart-not-ready")?;
    if store.failed || store.parked || !store.ready {
        return Err("checkpoint-probe-source");
    }
    let previous = store.custody.as_ref().ok_or("restart-custody-missing")?;
    let original = previous.source.identity();
    let generation = previous.source.generation();
    let source_bytes = previous.source.payload_length();
    let seed = store.seed.as_ref().ok_or("restart-key-missing")?;
    if ed25519_dalek::SigningKey::from_bytes(seed)
        .verifying_key()
        .as_bytes()
        != root
    {
        return Err("restart-signer-mismatch");
    }
    let before = heap();
    let started = clock_us();
    let prepared = prepare(previous, owner, store.admissions.as_ref(), root)?;
    let measured = prepared
        .measure_frozen(&mut store.journal, &previous.source)?
        .ok_or("checkpoint-probe-no-framed-source")?;
    let prepare_us = clock_us().saturating_sub(started);
    let after_prepare = heap();
    let started = clock_us();
    let (cache, writer) = prepared
        .capture_frozen(
            &mut store.journal,
            &previous.source,
            &store.binding,
            seed,
            &measured,
        )?
        .ok_or("checkpoint-probe-cache-capacity")?;
    let capture_us = clock_us().saturating_sub(started);
    let after_capture = heap();
    let output_bytes = writer.output_bytes();
    let retained_cache_bytes = cache.retained_bytes();
    // No `advance` or `finish` entrance is called: the writer is still before
    // its first erase. Dropping these private preparations cannot publish it.
    drop(writer);
    drop(cache);
    drop(measured);
    drop(prepared);
    if previous.source.identity() != original || store.journal.generation() != Some(generation) {
        return Err("checkpoint-probe-source-changed");
    }
    Ok(FrozenProbeReport {
        source_generation: generation,
        source_bytes,
        output_bytes,
        prepare_us,
        capture_us,
        retained_cache_bytes,
        heap_before: before,
        heap_after_prepare: after_prepare,
        heap_after_capture: after_capture,
        heap_after_release: heap(),
    })
}
