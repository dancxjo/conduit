//! Reopen exact parked custody only after complete validation and allocation.
use super::*;

/// Reopen the already admitted SAME Boot; no interrupted-Wake reconciliation,
/// membership mutation, challenge retirement, new key or journal write occurs.
pub fn restore_parked(
    mut token: ParkedBody,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
    key: &[u8; 32],
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    with_store(|store| restore_store(store, &mut token, host, boot, key))
}
/// Retain the actual parked scope and opaque proposal context on every refusal.
/// Only a successfully published reopened Store consumes this token. Reserved
/// CPU scratch is disposable and may be released before decoding on a retry.
pub fn restore_parked_retained(
    token: &mut Option<ParkedBody>,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
    key: &[u8; 32],
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    with_store(|store| restore_store_retained(store, token, host, boot, key))
}
pub(super) fn restore_store_retained(
    store: &mut Store,
    token: &mut Option<ParkedBody>,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
    key: &[u8; 32],
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    let body = restore_store(
        store,
        token.as_mut().ok_or("restart-parked-missing")?,
        host,
        boot,
        key,
    )?;
    drop(token.take());
    Ok(body)
}
pub(super) fn restore_store(
    store: &mut Store,
    token: &mut ParkedBody,
    host: &conduit_core::HostId,
    boot: &conduit_core::BootId,
    key: &[u8; 32],
) -> Result<Box<SealedBodyLifecycleSession>, &'static str> {
    if !store.parked
        || store.failed
        || store.ready
        || token.generation != store.journal.generation().ok_or("restart-generation")?
        || token.binding != store.binding
        || &token.key != key
        || &token.host != host
        || &token.boot != boot
    {
        return Err("restart-restore-scope");
    }
    let length = store
        .journal
        .payload_length(&store.binding)
        .map_err(|_| "restart-restore-binding")?
        .ok_or("restart-missing")?;
    #[cfg(all(target_os = "none", feature = "local-appliance"))]
    crate::native_root::memory_check::mark("cold-body-restore-payload", length);
    let mut seed = PrivateSeed([0; 32]);
    #[cfg(not(target_os = "none"))]
    let _ = length;
    #[cfg(not(target_os = "none"))]
    drop(token.workspace.take());
    let (resident, source) = decode_checkpoint(
        &mut store.journal,
        &store.binding,
        &token.body_id,
        &mut seed.0,
        store.custody.as_ref(),
    )?;
    let retained = resident.retained;
    if store.seed.as_ref() != Some(&seed.0)
        || ed25519_dalek::SigningKey::from_bytes(&seed.0)
            .verifying_key()
            .as_bytes()
            != key
    {
        return Err("restart-restore-key");
    }
    drop(seed);
    #[cfg(all(target_os = "none", feature = "local-appliance"))]
    crate::native_root::memory_check::mark("cold-body-restore-decoded", length);
    if <[u8; 32]>::from(Sha256::digest(
        retained.evidence.body_id.as_str().as_bytes(),
    )) != store.binding
    {
        return Err("restart-restore-binding");
    }
    let admissions = retained.admissions.ok_or("restart-admissions-missing")?;
    let mut body = SealedBodyLifecycleSession::open_admitted(retained.evidence, host, boot)
        .map_err(|_| "restart-restore-admitted")?;
    if let Some(plot) = token.foreground.as_ref() {
        body.select_plot(plot)
            .map_err(|_| "restart-restore-foreground")?;
    }
    if body.evidence().body.state != conduit_body::BodyState::Lulled || body.realization().is_some()
    {
        return Err("restart-restore-state");
    }
    let layout = core::alloc::Layout::new::<SealedBodyLifecycleSession>();
    // SAFETY: reserve the exact nonzero Box layout before publishing any Store
    // mutation. Allocation refusal leaves the parked scope available to retry.
    let pointer = unsafe { alloc::alloc::alloc(layout) }
        .cast::<core::mem::MaybeUninit<SealedBodyLifecycleSession>>();
    if pointer.is_null() {
        return Err("restart-body-backing-capacity");
    }
    // SAFETY: the unique non-null allocation has this Box's exact layout.
    let body = Box::write(unsafe { Box::from_raw(pointer) }, body);
    store.archives = Vec::new();
    store.custody = Some(ArchiveCustody {
        source,
        summary: resident.summary,
        span: resident.span,
    });
    store.admissions = Some(admissions);
    store.parked = false;
    store.ready = true;
    Ok(body)
}
