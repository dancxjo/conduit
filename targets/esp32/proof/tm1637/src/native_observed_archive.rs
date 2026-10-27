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
static BASIS: static_cell::StaticCell<Box<[u8; DESCRIPTOR_BYTES]>> = static_cell::StaticCell::new();
static WORK: Mutex<CriticalSectionRawMutex, RefCell<Option<Box<[u8; WORK_BYTES]>>>> =
    Mutex::new(RefCell::new(None));
fn with_work<R>(
    action: impl FnOnce(&mut [u8]) -> Result<R, &'static str>,
) -> Result<R, &'static str> {
    WORK.lock(|cell| {
        let mut work = cell.borrow_mut();
        action(&mut work.as_mut().ok_or("observed-history-workspace")?[..])
    })
}
fn prepare_shared() -> Result<&'static [u8], &'static str> {
    let contract = ObservationContract::prepare().map_err(|_| "observed-history-contract")?;
    let basis = observed_record::descriptor(&contract).map_err(|_| "observed-history-forms")?;
    let basis: &'static Box<[u8; DESCRIPTOR_BYTES]> = BASIS
        .try_init(Box::new(basis))
        .ok_or("observed-history-already-owned")?;
    WORK.lock(|cell| *cell.borrow_mut() = Some(Box::new([0; WORK_BYTES])));
    Ok(&basis[..])
}
static AUTHORIZED_BODY: Mutex<CriticalSectionRawMutex, RefCell<Option<[u8; 32]>>> =
    Mutex::new(RefCell::new(None));
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<State>>> =
    Mutex::new(RefCell::new(None));
// Awake-only bookkeeping, not a DMA buffer or a cross-reset trust source.
// The ordinary initialized section is cleared before reopening exact flash.
#[cfg_attr(feature = "local-appliance", esp_hal::ram(unstable(rtc_slow)))]
static EPOCHS: Mutex<CriticalSectionRawMutex, RefCell<Option<[State; 5]>>> =
    Mutex::new(RefCell::new(None));
#[path = "native_observed_state.rs"]
mod state;
use state::{Active, Cold};
type State = state::State<Region>;
#[inline(never)]
fn open_epoch(region: Region, basis: &'static [u8]) -> Result<State, &'static str> {
    let mut capacity =
        conduit_environment_runtime::display_evidence_journal::Storage::capacity(&region);
    let signed = capacity == 0x20000;
    match Erased::prepare_retained(region) {
        Ok(region) => Ok(State::Cold(Cold {
            region,
            signed,
            basis,
            scope: None,
            capacity,
            maximum_requested: 0,
        })),
        Err((mut region, _)) => {
            if signed {
                if let Ok((prefix, erased)) =
                    with_work(|work| recovery::retained_header_prefix(&mut region, basis, work))
                {
                    if prefix > 0 {
                        region = match region.seal_prefix(prefix) {
                            Ok(region) => region,
                            Err(_) => return Ok(State::Failed),
                        };
                        capacity -= prefix;
                    }
                    if erased {
                        return Erased::prepare_retained(region)
                            .map(|region| {
                                State::Cold(Cold {
                                    region,
                                    signed,
                                    basis,
                                    scope: None,
                                    capacity,
                                    maximum_requested: 0,
                                })
                            })
                            .map_err(|_| "observed-history-empty-suffix");
                    }
                }
            }
            match with_work(|work| recovery::reopen_profile(region, basis, work, signed)) {
                Ok((journal, scope, last_sequence)) => Ok(State::Active(Active {
                    journal,
                    scope,
                    forms: Sha256::digest(basis).into(),
                    last_sequence,
                    failed: false,
                    recovered: true,
                    signed,
                    maximum_records: ((capacity.saturating_sub(536))
                        / if signed { 104 } else { 40 })
                    .min(960),
                })),
                // Invalid media stays physically intact and is never reissued.
                Err(_) => Ok(State::Failed),
            }
        }
    }
}
/// Five immutable 128 KiB signed epochs admit at most 80 hours at the linked cadence.
/// Binding consumes no media. Empty committed headers can be sealed and their
/// verified erased suffix reused; nonempty or torn media is never erased.
/// Each suffix admits only the readings that fit its remaining physical extent.
pub fn initialize_epochs(
    region: impl Into<Option<Region>>,
    additional: [Region; 5],
) -> Result<(), &'static str> {
    let basis = prepare_shared()?;
    // An absent legacy region supplies no journal and can never be read/signed.
    // The existing slot array retains five real signed regions without adding
    // storage or relabeling any recovered scope.
    let mut primary = match region.into() {
        Some(region) => open_epoch(region, basis)?,
        None => State::Failed,
    };
    let epochs = additional.map(|region| open_epoch(region, basis));
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
    let basis = prepare_shared()?;
    install_epoch(open_epoch(region, basis)?)
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
/// Rotate outgoing storage only after the actual sampler returned its native
/// owners and Root installed a distinct admitted proposal. This does not prove
/// normal finite completion or perform the separate Body Lull/Wake transition.
#[cfg(all(feature = "body-image-root", feature = "native-face-root"))]
pub(crate) fn rotate_local(
    owner: &conduit_body::SealedBodyLifecycleSession,
    fragment: &conduit_core::FragmentId,
    fresh: &conduit_core::ActivePlayIdentity,
    generation: conduit_core::OfferGeneration,
    old: &conduit_core::ActivePlayIdentity,
    retired: &crate::native_task::Retired,
) -> Result<(), &'static str> {
    if !retired.execution.providers.matches_active(old)
        || old.host_id != fresh.host_id
        || old.boot_id != fresh.boot_id
        || fresh.play_sequence <= old.play_sequence
        || old.active_play_id == fresh.active_play_id
    {
        return Err("observed-history-native-retirement-scope");
    }
    let source = crate::native_software::source_identity();
    let (body, _) = local_scope::check_installed(owner, &source, fragment, fresh, generation)?;
    let old_scope = scope(body.as_str(), old);
    let fresh_scope = scope(body.as_str(), fresh);
    let (_, cadence) = crate::native_software::source_facts();
    let maximum = cadence
        .count
        .try_into()
        .map_err(|_| "observed-history-count")?;
    STATE.lock(|primary| {
        let mut primary = primary.borrow_mut();
        let primary = primary.as_mut().ok_or("observed-history-unavailable")?;
        EPOCHS.lock(|epochs| {
            let mut epochs = epochs.borrow_mut();
            state::rotate(
                primary,
                epochs.as_mut().ok_or("observed-history-epochs")?,
                &old_scope,
                fresh_scope,
                maximum,
            )
        })
    })
}
#[cfg(all(feature = "body-image-root", feature = "native-face-root"))]
#[path = "native_local_face_scope.rs"]
mod local_scope;

fn scope(body: &str, active: &conduit_core::ActivePlayIdentity) -> [u8; 96] {
    let mut scope = [0; 96];
    for (out, text) in scope.chunks_exact_mut(32).zip([
        body,
        active.boot_id.as_str(),
        active.active_play_id.as_str(),
    ]) {
        out.copy_from_slice(&Sha256::digest(text.as_bytes()));
    }
    scope
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
    let scope = scope(body, active);
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        // A retained previous Boot is never silently replaced or erased.
        if !matches!(state.as_ref(), Some(State::Cold(_))) {
            return Err("observed-history-unavailable");
        }
        let Some(State::Cold(cold)) = state.as_mut() else {
            unreachable!()
        };
        cold.scope = Some(scope);
        cold.maximum_requested = maximum;
        Ok(())
    })
}
#[inline(never)]
pub fn append(
    snapshot: PublicationSnapshot<'_>,
    observed_ms: u64,
    producer_play: &[u8; 32],
) -> Result<(), &'static str> {
    let record =
        ObservedRecord::from_publication(snapshot, observed_ms).and_then(|record| record.encode());
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        state::check_writer(
            state.as_ref().ok_or("observed-history-unavailable")?,
            producer_play,
        )?;
        let record = match record {
            Ok(record) => record,
            Err(_) => {
                if let Some(State::Active(active)) = state.as_mut() {
                    active.failed = true;
                }
                return Err("observed-history-projection");
            }
        };
        let sequence = u64::from_le_bytes(record[..8].try_into().unwrap());
        if matches!(state.as_ref(), Some(State::Cold(cold)) if cold.scope.is_some()) {
            let Some(State::Cold(cold)) = state.take() else {
                unreachable!()
            };
            *state = Some(State::Failed);
            let scope = cold.scope.ok_or("observed-history-scope")?;
            if sequence != 1 {
                return Err("observed-history-sequence");
            }
            // Signed records use the raw codec: 85 payload + 2 codec bytes,
            // four-byte padding, 8-byte prefix and 8-byte commit = 104 bytes.
            let capacity = cold.capacity;
            let stride = if cold.signed { 104 } else { 40 };
            let maximum = ((capacity.saturating_sub(536)) / stride).min(cold.maximum_requested);
            if maximum == 0 {
                return Err("observed-history-capacity");
            }
            let journal = with_work(|work| {
                Journal::create_prepared(cold.region, &scope, cold.basis, maximum, work)
                    .map_err(|_| "observed-history-header")
            })?;
            *state = Some(State::Active(Active {
                journal,
                scope,
                forms: Sha256::digest(cold.basis).into(),
                last_sequence: 0,
                failed: false,
                recovered: false,
                signed: cold.signed,
                maximum_records: maximum,
            }));
        }
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
            with_work(|work| {
                active
                    .journal
                    .append(record, work)
                    .map(|_| ())
                    .map_err(|_| "observed-history-commit")
            })
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
    maximum_records: u32,
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
            maximum_records: active.maximum_records,
        },
        Some(State::Cold(cold)) => Status {
            committed: 0,
            retained_bytes: 0,
            last_sequence: 0,
            scope_ready: cold.scope.is_some(),
            failed: false,
            recovered_read_only: false,
            maximum_records: ((cold.capacity.saturating_sub(536))
                / if cold.signed { 104 } else { 40 })
            .min(if cold.scope.is_some() {
                cold.maximum_requested
            } else {
                960
            }),
        },
        _ => Status {
            committed: 0,
            retained_bytes: 0,
            last_sequence: 0,
            scope_ready: false,
            failed: true,
            recovered_read_only: false,
            maximum_records: 0,
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
        match with_work(|work| {
            active
                .journal
                .replay(&mut cursor, &mut retained[..length], work)
                .map_err(|_| "observed-history-read")
        })? {
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
