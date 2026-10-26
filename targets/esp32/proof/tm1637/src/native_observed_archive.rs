//! Root custody of a finite outgoing observation projection. This does not
//! offer an application store, invent acquisition times, or replace signed
//! canonical publications. No remote reader participates in local append.
use crate::{native_evidence_region::Region, native_owner_reservation::OwnerReservation};
use alloc::boxed::Box;
use conduit_environment_runtime::{
    display_evidence_journal::{Erased, Journal},
    observation::ObservationContract,
    observed_record::{self, ObservedRecord, DESCRIPTOR_BYTES},
    publication::PublicationSnapshot,
};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use sha2::{Digest, Sha256};
#[path = "native_local_history.rs"]
pub(crate) mod history;
#[path = "native_observed_recovery.rs"]
mod recovery;
const WORK_BYTES: usize = 536;
static AUTHORIZED_BODY: Mutex<CriticalSectionRawMutex, RefCell<Option<[u8; 32]>>> =
    Mutex::new(RefCell::new(None));
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<State>>> =
    Mutex::new(RefCell::new(None));
static EPOCHS: Mutex<CriticalSectionRawMutex, RefCell<Option<[State; 5]>>> =
    Mutex::new(RefCell::new(None));
struct Cold {
    region: Erased<Region>,
    signed: bool,
    basis: Box<[u8; DESCRIPTOR_BYTES]>,
    work: Box<[u8; WORK_BYTES]>,
}
struct Active {
    journal: Journal<'static, Region>,
    work: Box<[u8; WORK_BYTES]>,
    scope: [u8; 96],
    forms: [u8; 32],
    last_sequence: u64,
    failed: bool,
    recovered: bool,
    signed: bool,
}
enum State {
    Cold(Cold),
    Active(Active),
    Failed,
}
#[inline(never)]
fn open_epoch(region: Region) -> Result<State, &'static str> {
    let signed = conduit_environment_runtime::display_evidence_journal::Storage::capacity(&region)
        == 0x20000;
    let initial = match Erased::prepare_retained(region) {
        Ok(region) => {
            let contract =
                ObservationContract::prepare().map_err(|_| "observed-history-contract")?;
            let basis =
                observed_record::descriptor(&contract).map_err(|_| "observed-history-forms")?;
            State::Cold(Cold {
                region,
                signed,
                basis: Box::new(basis),
                work: Box::new([0; WORK_BYTES]),
            })
        }
        Err((region, _)) => {
            let contract =
                ObservationContract::prepare().map_err(|_| "observed-history-contract")?;
            let basis: &'static [u8] = Box::leak(Box::new(
                observed_record::descriptor(&contract).map_err(|_| "observed-history-forms")?,
            ));
            let mut work = Box::new([0; WORK_BYTES]);
            match recovery::reopen_profile(region, basis, &mut work[..], signed) {
                Ok((journal, scope, last_sequence)) => State::Active(Active {
                    journal,
                    work,
                    scope,
                    forms: Sha256::digest(basis).into(),
                    last_sequence,
                    failed: false,
                    recovered: true,
                    signed,
                }),
                // Invalid media is preserved physically; its unique handle is
                // retired and cannot be erased or reissued by this adapter.
                Err(_) => State::Failed,
            }
        }
    };
    Ok(initial)
}
/// Five immutable 128 KiB signed epochs admit at most 80 hours at the linked cadence.
/// Reboot consumes a fresh erased epoch; committed or torn media is never erased.
pub fn initialize_epochs(region: Region, additional: [Region; 5]) -> Result<(), &'static str> {
    let mut primary = open_epoch(region)?;
    let epochs = additional.map(open_epoch);
    if epochs.iter().any(Result::is_err) {
        return Err("observed-history-epoch-initialization");
    }
    let mut retained = epochs.map(|epoch| epoch.expect("preflighted epoch"));
    // The original 64 KiB legacy epoch is never used for new signed readings.
    if let Some(blank) = retained
        .iter_mut()
        .find(|epoch| matches!(epoch, State::Cold(_)))
    {
        core::mem::swap(&mut primary, blank);
    }
    if let State::Cold(_) = primary {
        // No signed epoch remains: preserve the legacy region without append.
        if !retained
            .iter()
            .any(|epoch| matches!(epoch, State::Cold(cold) if cold.signed))
        {
            if !matches!(&primary, State::Cold(cold) if cold.signed) {
                primary = State::Failed;
            }
        }
    }
    EPOCHS.lock(|cell| {
        let mut slot = cell.borrow_mut();
        if slot.is_some() {
            return Err("observed-history-already-owned");
        }
        *slot = Some(retained);
        Ok(())
    })?;
    install_epoch(primary)
}
pub fn initialize(region: Region) -> Result<(), &'static str> {
    install_epoch(open_epoch(region)?)
}
fn install_epoch(initial: State) -> Result<(), &'static str> {
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        if state.is_some() {
            return Err("observed-history-already-owned");
        }
        *state = Some(initial);
        Ok(())
    })
}
/// Only the actual authenticated sampler reservation can bind this outbox.
/// All record/work capacity is fixed before the first publication.
#[inline(never)]
pub fn bind(reservation: &OwnerReservation) -> Result<(), &'static str> {
    bind_scope(&reservation.body, &reservation.active)
}
/// Root supplies this only after the installed local Body/fragment scope passes
/// admission. This binds outgoing custody, never membership or native authority.
#[cfg(feature = "body-image-root")]
pub(crate) fn bind_local(
    body: &str,
    active: &conduit_core::ActivePlayIdentity,
) -> Result<(), &'static str> {
    authorize_body(body);
    let retained = STATE.lock(|cell| match cell.borrow().as_ref() {
        Some(State::Active(active)) => active.recovered,
        Some(State::Failed) => true,
        _ => false,
    });
    if retained {
        // Measurement remains available even if old media is unreadable. This
        // Boot has no append/signing authority over the prior-Boot outbox.
        return Ok(());
    }
    match bind_scope(body, active) {
        // Archive media failure cannot suppress independently admitted local
        // measurement. Failed state remains visible and refuses all appends.
        Err("observed-history-header") => Ok(()),
        outcome => outcome,
    }
}
fn bind_scope(body: &str, active: &conduit_core::ActivePlayIdentity) -> Result<(), &'static str> {
    authorize_body(body);
    let (_, cadence) = crate::native_software::source_facts();
    let maximum: u32 = cadence
        .count
        .try_into()
        .map_err(|_| "observed-history-count")?;
    if maximum == 0 || maximum > 960 {
        return Err("observed-history-capacity");
    }
    let mut scope = [0; 96];
    for (out, text) in scope.chunks_exact_mut(32).zip([
        body,
        active.boot_id.as_str(),
        active.active_play_id.as_str(),
    ]) {
        out.copy_from_slice(&Sha256::digest(text.as_bytes()));
    }
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        // A retained previous Boot is never silently replaced or erased.
        if !matches!(state.as_ref(), Some(State::Cold(_))) {
            return Err("observed-history-unavailable");
        }
        let Some(State::Cold(mut cold)) = state.take() else {
            unreachable!()
        };
        *state = Some(State::Failed);
        let signed = cold.signed;
        let basis = &Box::leak(cold.basis)[..];
        let journal =
            Journal::create_prepared(cold.region, &scope, basis, maximum, &mut cold.work[..])
                .map_err(|_| "observed-history-header")?;
        *state = Some(State::Active(Active {
            journal,
            work: cold.work,
            scope,
            forms: Sha256::digest(basis).into(),
            last_sequence: 0,
            failed: false,
            recovered: false,
            signed,
        }));
        Ok(())
    })
}
#[inline(never)]
pub fn append(snapshot: PublicationSnapshot<'_>, observed_ms: u64) -> Result<(), &'static str> {
    let record =
        ObservedRecord::from_publication(snapshot, observed_ms).and_then(|record| record.encode());
    let record = match record {
        Ok(record) => record,
        Err(_) => {
            STATE.lock(|cell| {
                if let Some(State::Active(active)) = cell.borrow_mut().as_mut() {
                    active.failed = true;
                }
            });
            return Err("observed-history-projection");
        }
    };
    let sequence = u64::from_le_bytes(record[..8].try_into().unwrap());
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        let Some(State::Active(active)) = state.as_mut() else {
            return Err("observed-history-unavailable");
        };
        if active.recovered {
            return Err("observed-history-prior-boot-read-only");
        }
        if active.failed {
            return Err("observed-history-failed");
        }
        let mut signed_record = [0u8; history::SIGNED_BYTES];
        let record = if active.signed {
            signed_record[..record.len()].copy_from_slice(&record);
            let signature =
                crate::native_observed_http::sign_record(&active.scope, &active.forms, &record)?;
            signed_record[record.len()..].copy_from_slice(&signature);
            &signed_record[..]
        } else {
            &record[..]
        };
        let outcome = if active.last_sequence.checked_add(1) != Some(sequence) {
            Err("observed-history-sequence")
        } else {
            active
                .journal
                .append(record, &mut active.work[..])
                .map(|_| ())
                .map_err(|_| "observed-history-commit")
        };
        if outcome.is_ok() {
            active.last_sequence = sequence;
        } else {
            active.failed = true;
        }
        outcome
    })
}
#[derive(serde::Serialize)]
pub struct Status {
    committed: u32,
    retained_bytes: u32,
    last_sequence: u64,
    scope_ready: bool,
    failed: bool,
    recovered_read_only: bool,
}
pub fn status() -> Status {
    STATE.lock(|cell| match cell.borrow().as_ref() {
        Some(State::Active(active)) => Status {
            committed: active.journal.committed_count(),
            retained_bytes: active.journal.retained_extent(),
            last_sequence: active.last_sequence,
            scope_ready: !active.recovered,
            failed: active.failed,
            recovered_read_only: active.recovered,
        },
        Some(State::Cold(_)) => Status {
            committed: 0,
            retained_bytes: 0,
            last_sequence: 0,
            scope_ready: false,
            failed: false,
            recovered_read_only: false,
        },
        _ => Status {
            committed: 0,
            retained_bytes: 0,
            last_sequence: 0,
            scope_ready: false,
            failed: true,
            recovered_read_only: false,
        },
    })
}

/// Immutable owned batch before signing or socket I/O; no journal borrow escapes.
pub struct Batch {
    pub scope: [u8; 96],
    pub forms: [u8; 32],
    pub committed: u32,
    pub used: usize,
    pub records: [u8; 4 * observed_record::BYTES],
}
pub fn copy(after: u32) -> Result<Batch, &'static str> {
    copy_checked(after, None)
}
/// Root may read retained bytes only against an independently authenticated
/// exact original scope. This does not restore its original signing key.
#[allow(dead_code)]
pub(crate) fn copy_recovered(scope: &[u8; 96], after: u32) -> Result<Batch, &'static str> {
    copy_checked(after, Some(scope))
}
fn copy_checked(after: u32, expected: Option<&[u8; 96]>) -> Result<Batch, &'static str> {
    let primary = STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        let Some(State::Active(active)) = state.as_mut() else {
            return Err("observed-history-unavailable");
        };
        if active.recovered && expected != Some(&active.scope) {
            return Err("observed-history-original-scope-required");
        }
        if expected.is_some_and(|scope| scope != &active.scope) {
            return Err("observed-history-scope");
        }
        copy_active(active, after)
    });
    if primary.is_ok() || expected.is_none() {
        return primary;
    }
    EPOCHS.lock(|cell| {
        let mut epochs = cell.borrow_mut();
        let Some(epochs) = epochs.as_mut() else {
            return primary;
        };
        for epoch in epochs {
            if let State::Active(active) = epoch {
                if expected == Some(&active.scope) {
                    return copy_active(active, after);
                }
            }
        }
        primary
    })
}
fn copy_active(active: &mut Active, after: u32) -> Result<Batch, &'static str> {
    if active.recovered
        && (!active.signed
            || !AUTHORIZED_BODY
                .lock(|cell| *cell.borrow() == Some(active.scope[..32].try_into().unwrap())))
    {
        return Err("observed-history-original-authority-required");
    }
    let mut cursor = active
        .journal
        .replay_cursor_at(after)
        .map_err(|_| "observed-history-cursor")?;
    let mut batch = Batch {
        scope: active.scope,
        forms: active.forms,
        committed: active.journal.committed_count(),
        used: 0,
        records: [0; 4 * observed_record::BYTES],
    };
    let mut retained = [0u8; history::SIGNED_BYTES];
    for output in batch.records.chunks_exact_mut(observed_record::BYTES) {
        let length = if active.signed {
            retained.len()
        } else {
            observed_record::BYTES
        };
        match active
            .journal
            .replay(&mut cursor, &mut retained[..length], &mut active.work[..])
            .map_err(|_| "observed-history-read")?
        {
            None => break,
            Some(n) if n == length => {
                if active.signed {
                    crate::native_observed_http::verify_record(
                        &active.scope,
                        &active.forms,
                        &retained[..observed_record::BYTES],
                        retained[observed_record::BYTES..].try_into().unwrap(),
                    )?;
                }
                output.copy_from_slice(&retained[..observed_record::BYTES]);
                batch.used += observed_record::BYTES;
            }
            _ => return Err("observed-history-record"),
        }
    }
    Ok(batch)
}

fn authorize_body(body: &str) {
    let digest = Sha256::digest(body.as_bytes()).into();
    AUTHORIZED_BODY.lock(|cell| *cell.borrow_mut() = Some(digest));
}

/// An ephemeral slot is only a bounded replay address. Every returned reading
/// still passes its original scope and persisted Root signature fences.
pub(crate) fn copy_epoch(slot: u8, after: u32) -> Result<Batch, &'static str> {
    if slot == 0 {
        return copy(after);
    }
    let index = usize::from(slot.checked_sub(1).ok_or("observed-history-slot")?);
    EPOCHS.lock(|cell| {
        let mut epochs = cell.borrow_mut();
        let Some(State::Active(active)) = epochs.as_mut().and_then(|epochs| epochs.get_mut(index))
        else {
            return Err("observed-history-slot");
        };
        copy_active(active, after)
    })
}
pub(crate) fn directory() -> Result<[u8; crate::native_observed_wire::DIRECTORY_BYTES], &'static str>
{
    let body = AUTHORIZED_BODY
        .lock(|cell| *cell.borrow())
        .ok_or("observed-history-body")?;
    let mut bytes = [0; crate::native_observed_wire::DIRECTORY_BYTES];
    bytes[..32].copy_from_slice(&body);
    for slot in 0..=5 {
        let Ok(batch) = copy_epoch(slot, 0) else {
            continue;
        };
        if batch.used == 0 || batch.scope[..32] != body {
            continue;
        }
        let count = usize::from(bytes[32]);
        if count == 5 {
            return Err("observed-history-directory-capacity");
        }
        let entry = &mut bytes[33 + count * 133..33 + (count + 1) * 133];
        entry[0] = slot;
        entry[1..97].copy_from_slice(&batch.scope);
        entry[97..129].copy_from_slice(&batch.forms);
        entry[129..133].copy_from_slice(&batch.committed.to_le_bytes());
        bytes[32] += 1;
    }
    Ok(bytes)
}
