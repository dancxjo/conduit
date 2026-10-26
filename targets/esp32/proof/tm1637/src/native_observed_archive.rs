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
const WORK_BYTES: usize = 536;
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<State>>> =
    Mutex::new(RefCell::new(None));
struct Cold {
    region: Erased<Region>,
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
}
enum State {
    Cold(Cold),
    Active(Active),
    Retained { _region: Region },
    Failed,
}
#[inline(never)]
pub fn initialize(region: Region) -> Result<(), &'static str> {
    let initial = match Erased::prepare_retained(region) {
        Ok(region) => {
            let contract =
                ObservationContract::prepare().map_err(|_| "observed-history-contract")?;
            let basis =
                observed_record::descriptor(&contract).map_err(|_| "observed-history-forms")?;
            State::Cold(Cold {
                region,
                basis: Box::new(basis),
                work: Box::new([0; WORK_BYTES]),
            })
        }
        Err((region, _)) => State::Retained { _region: region },
    };
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
        reservation.body.as_str(),
        reservation.active.boot_id.as_str(),
        reservation.active.active_play_id.as_str(),
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
        if active.failed {
            return Err("observed-history-failed");
        }
        let outcome = if active.last_sequence.checked_add(1) != Some(sequence) {
            Err("observed-history-sequence")
        } else {
            active
                .journal
                .append(&record, &mut active.work[..])
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
}
pub fn status() -> Status {
    STATE.lock(|cell| match cell.borrow().as_ref() {
        Some(State::Active(active)) => Status {
            committed: active.journal.committed_count(),
            retained_bytes: active.journal.retained_extent(),
            last_sequence: active.last_sequence,
            scope_ready: true,
            failed: active.failed,
        },
        Some(State::Cold(_)) => Status {
            committed: 0,
            retained_bytes: 0,
            last_sequence: 0,
            scope_ready: false,
            failed: false,
        },
        _ => Status {
            committed: 0,
            retained_bytes: 0,
            last_sequence: 0,
            scope_ready: false,
            failed: true,
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
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        let Some(State::Active(active)) = state.as_mut() else {
            return Err("observed-history-unavailable");
        };
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
        for output in batch.records.chunks_exact_mut(observed_record::BYTES) {
            match active
                .journal
                .replay(&mut cursor, output, &mut active.work[..])
                .map_err(|_| "observed-history-read")?
            {
                None => break,
                Some(n) if n == observed_record::BYTES => batch.used += n,
                _ => return Err("observed-history-record"),
            }
        }
        Ok(batch)
    })
}
