//! Root integrity custody for kernel-produced display evidence only. This is
//! not an application storage offer or a general flash-writing authority.
use crate::{native_evidence_region::Region, native_owner_control::EvidenceReference};
use alloc::boxed::Box;
use conduit_environment_runtime::{
    display_evidence as wire,
    display_evidence_journal::{Erased, Journal, Replay, WORKSPACE_BYTES},
};
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
use sha2::{Digest, Sha256};
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<State>>> =
    Mutex::new(RefCell::new(None));
struct Cold {
    region: Erased<Region>,
    basis: Box<[u8; wire::MAXIMUM_BYTES]>,
    work: Box<[u8; WORKSPACE_BYTES]>,
    scope: Option<[u8; 96]>,
}
struct Active {
    journal: Journal<'static, Region>,
    work: Box<[u8; WORKSPACE_BYTES]>,
    cursor: Replay,
    offered: Option<(EvidenceReference, Replay)>,
    accepted: u32,
}
enum State {
    Cold(Cold),
    Active(Active),
    Retained { _region: Region },
    Failed,
}
#[inline(never)]
pub fn initialize(flash: esp_hal::peripherals::FLASH<'static>) -> Result<(), &'static str> {
    let state = match Erased::prepare_retained(Region::prepare(flash)?) {
        Ok(region) => State::Cold(Cold {
            region,
            basis: Box::new([0; wire::MAXIMUM_BYTES]),
            work: Box::new([0; WORKSPACE_BYTES]),
            scope: None,
        }),
        Err((region, _)) => State::Retained { _region: region },
    };
    STATE.lock(|s| {
        let mut s = s.borrow_mut();
        if s.is_some() {
            return Err("evidence-region-already-owned");
        }
        *s = Some(state);
        Ok(())
    })
}
pub fn bind_scope(
    reservation: &crate::native_owner_reservation::OwnerReservation,
) -> Result<(), &'static str> {
    let mut scope = [0; 96];
    for (out, text) in scope.chunks_exact_mut(32).zip([
        reservation.body.as_str(),
        reservation.active.boot_id.as_str(),
        reservation.active.active_play_id.as_str(),
    ]) {
        out.copy_from_slice(&Sha256::digest(text.as_bytes()));
    }
    STATE.lock(|s| match s.borrow_mut().as_mut() {
        Some(State::Cold(cold)) if cold.scope.is_none() || cold.scope == Some(scope) => {
            cold.scope = Some(scope);
            Ok(())
        }
        _ => Err("evidence-local-scope"),
    })
}
pub fn append(page: &[u8]) -> Result<(), &'static str> {
    wire::Page::parse(page).map_err(|_| "evidence-local-canonical")?;
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        if matches!(state.as_ref(), Some(State::Cold(_))) {
            let Some(State::Cold(mut cold)) = state.take() else {
                unreachable!()
            };
            *state = Some(State::Failed);
            let scope = cold.scope.ok_or("evidence-local-unreserved")?;
            cold.basis[..page.len()].copy_from_slice(page);
            // All backing was allocated before Play. Leak transfers unique
            // immutable basis ownership to the Boot-lifetime journal, no alloc.
            let basis = &Box::leak(cold.basis)[..page.len()];
            let journal = Journal::create_prepared(
                cold.region,
                &scope,
                basis,
                crate::native_display::CONSTRUCTION_FRAMES + 1,
                &mut cold.work,
            )
            .map_err(|_| "evidence-local-header")?;
            let cursor = journal.replay_cursor();
            *state = Some(State::Active(Active {
                journal,
                work: cold.work,
                cursor,
                offered: None,
                accepted: 0,
            }));
        }
        let Some(State::Active(active)) = state.as_mut() else {
            return Err("evidence-local-unavailable");
        };
        active
            .journal
            .append(page, &mut active.work)
            .map_err(|_| "evidence-local-commit")?;
        Ok(())
    })
}
pub fn copy(output: &mut [u8]) -> Result<Option<(EvidenceReference, usize)>, &'static str> {
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        let Some(State::Active(active)) = state.as_mut() else {
            return Ok(None);
        };
        let mut next = active.cursor;
        let Some(n) = active
            .journal
            .replay(&mut next, output, &mut active.work)
            .map_err(|_| "evidence-local-read")?
        else {
            return Ok(None);
        };
        let page = wire::Page::parse(&output[..n]).map_err(|_| "evidence-local-canonical")?;
        let reference = EvidenceReference {
            invocation: page.invocation(),
            sha256: Sha256::digest(&output[..n]).into(),
        };
        if active
            .offered
            .as_ref()
            .is_some_and(|(prior, _)| *prior != reference)
        {
            return Err("evidence-local-offer-changed");
        }
        active.offered = Some((reference, next));
        Ok(Some((reference, n)))
    })
}
pub fn accept(reference: EvidenceReference) -> Result<(), &'static str> {
    STATE.lock(|cell| {
        let mut state = cell.borrow_mut();
        let Some(State::Active(active)) = state.as_mut() else {
            return Err("evidence-local-unavailable");
        };
        let (offered, next) = active.offered.as_ref().ok_or("evidence-local-unoffered")?;
        if *offered != reference {
            return Err("evidence-local-acceptance");
        }
        active.cursor = *next;
        active.offered = None;
        active.accepted += 1;
        Ok(())
    })
}
pub fn pending() -> bool {
    STATE.lock(|s|matches!(s.borrow().as_ref(),Some(State::Active(active))if active.accepted<active.journal.committed_count()))
}

#[derive(serde::Serialize)]
pub struct Status {
    committed: u32,
    accepted: u32,
    retained_bytes: u32,
    scope_ready: bool,
    failed: bool,
}
pub fn status() -> Status {
    STATE.lock(|s| match s.borrow().as_ref() {
        Some(State::Active(active)) => Status {
            committed: active.journal.committed_count(),
            accepted: active.accepted,
            retained_bytes: active.journal.retained_extent(),
            scope_ready: true,
            failed: false,
        },
        Some(State::Cold(cold)) => Status {
            committed: 0,
            accepted: 0,
            retained_bytes: 0,
            scope_ready: cold.scope.is_some(),
            failed: false,
        },
        _ => Status {
            committed: 0,
            accepted: 0,
            retained_bytes: 0,
            scope_ready: false,
            failed: true,
        },
    })
}

pub fn available() -> bool {
    STATE.lock(|s| matches!(s.borrow().as_ref(), Some(State::Cold(_) | State::Active(_))))
}
