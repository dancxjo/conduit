//! Actual append-only custody for retired network control evidence.
use super::RetiredNetworkControl;
use crate::native_evidence_region::NetworkRegion;
use conduit_network_join_call::recovery_evidence as wire;
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
// The new immutable diagnostic lives once in flash, outside the CPU stack arena.
#[used]
#[link_section = ".rodata.native_network_status"]
static CAPACITY_REFUSAL: [u8; b"network-evidence-capacity".len()] = *b"network-evidence-capacity";
struct Archive {
    journal: wire::Journal<NetworkRegion>,
}
enum State {
    Ready(Archive),
    Frozen(wire::FrozenJournal<NetworkRegion>),
    // Blank readback plus adopted Clean grants this capacity observation only.
    // Actual append MUST reopen/validate, never reuse the old next-slot cache.
    Reclaimed(wire::FrozenJournal<NetworkRegion>),
    Inspecting { remaining: u8 },
    Retained { _region: NetworkRegion },
}
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<State>>> =
    Mutex::new(RefCell::new(None));
// This is the existing Root-control reserve, not a larger evidence grant.
const ROOT_CONTROL_RESERVE: usize = 8192;
#[used]
#[link_section = ".rodata.native_network_status"]
static WORKSPACE_REFUSAL: [u8; b"network-evidence-workspace".len()] =
    *b"network-evidence-workspace";
#[inline(never)]
fn scratch() -> Result<alloc::boxed::Box<[u8; wire::MAXIMUM_BYTES]>, &'static str> {
    let refusal = || unsafe { core::str::from_utf8_unchecked(&WORKSPACE_REFUSAL) };
    if esp_alloc::HEAP.free() < ROOT_CONTROL_RESERVE + wire::MAXIMUM_BYTES {
        return Err(refusal());
    }
    let layout = core::alloc::Layout::new::<[u8; wire::MAXIMUM_BYTES]>();
    // SAFETY: zeroed bytes initialize this exact array layout. A null allocation
    // is refused; Box uses the same layout on drop. No page-sized stack value.
    let pointer = unsafe { alloc::alloc::alloc(layout) };
    if pointer.is_null() {
        return Err(refusal());
    }
    unsafe { pointer.write_bytes(0, wire::MAXIMUM_BYTES) };
    let work = unsafe { alloc::boxed::Box::from_raw(pointer.cast::<[u8; wire::MAXIMUM_BYTES]>()) };
    if esp_alloc::HEAP.free() < ROOT_CONTROL_RESERVE {
        return Err(refusal());
    }
    Ok(work)
}
pub fn initialize(region: NetworkRegion) -> Result<(), &'static str> {
    let (state, result) = match scratch() {
        Ok(mut work) => match wire::Journal::open(region, &mut work) {
            Ok(journal) => (State::Ready(Archive { journal }), Ok(())),
            Err((region, _)) => (
                State::Retained { _region: region },
                Err("network-evidence-retained-refusal"),
            ),
        },
        Err(error) => (State::Retained { _region: region }, Err(error)),
    };
    STATE.lock(|current| {
        let mut current = current.borrow_mut();
        if current.is_some() {
            return Err("network-evidence-already-owned");
        }
        *current = Some(state);
        result
    })
}
#[allow(unexpected_cfgs)] // Native profile is absent from CPU archive proof.
pub(super) fn append(retired: &RetiredNetworkControl) -> Result<(), &'static str> {
    #[cfg(all(feature = "body-persistence", target_os = "none"))]
    crate::native_restart::network_recovery::ensure_append_allowed()?;
    STATE.lock(|current| {
        let mut current = current.borrow_mut();
        if !matches!(
            current.as_ref(),
            Some(State::Ready(_) | State::Frozen(_) | State::Reclaimed(_))
        ) {
            return Err("network-evidence-unavailable");
        }
        // Admit scratch before taking the frozen owner: pressure cannot lose it.
        let mut work = scratch()?;
        if matches!(
            current.as_ref(),
            Some(State::Frozen(_) | State::Reclaimed(_))
        ) {
            let frozen = match current.take() {
                Some(State::Frozen(frozen) | State::Reclaimed(frozen)) => frozen,
                _ => unreachable!(),
            };
            match frozen.reopen(&mut work) {
                Ok(journal) => *current = Some(State::Ready(Archive { journal })),
                Err((region, _)) => {
                    *current = Some(State::Retained { _region: region });
                    return Err("network-evidence-retained-refusal");
                }
            }
        }
        let Some(State::Ready(archive)) = current.as_mut() else {
            unreachable!()
        };
        let signs = retired.evidence();
        wire::encode(
            wire::Scope {
                host: retired.active.host_id.as_str(),
                boot: retired.active.boot_id.as_str(),
                plan: retired.active.plan_id.as_str(),
                active_play: retired.active.active_play_id.as_str(),
                provider_generation: retired.provider_generation,
                play_sequence: retired.active.play_sequence,
                configuration_revision: retired.configuration_revision,
            },
            signs.events(),
            signs,
            &mut work,
        )
        .map_err(|_| "network-evidence-encoding")?;
        match archive.journal.append(&work) {
            Ok(_) => (),
            Err(error) => {
                return Err(if error == wire::Refusal::Capacity {
                    capacity_refusal()
                } else {
                    "network-evidence-commit"
                });
            }
        }
        Ok(())
    })
}
impl wire::Storage for NetworkRegion {
    fn capacity(&self) -> u32 {
        conduit_environment_runtime::display_evidence_journal::Storage::capacity(self)
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), wire::Refusal> {
        conduit_environment_runtime::display_evidence_journal::Storage::read(self, at, out)
            .map_err(|_| wire::Refusal::Media)
    }
    fn program(&mut self, at: u32, bytes: &[u8]) -> Result<(), wire::Refusal> {
        conduit_environment_runtime::display_evidence_journal::Storage::program(self, at, bytes)
            .map_err(|_| wire::Refusal::Media)
    }
}

/// Capacity observation only; no permission to overwrite retained evidence.
pub fn exhausted() -> bool {
    STATE.lock(|current| match &*current.borrow() {
        Some(State::Ready(archive)) => archive.journal.remaining_slots() == 0,
        Some(State::Frozen(frozen)) => frozen.remaining_slots() == 0,
        Some(State::Inspecting { remaining }) => *remaining == 0,
        _ => false,
    })
}
pub fn capacity_refusal() -> &'static str {
    // SAFETY: the immutable literal above is valid ASCII.
    unsafe { core::str::from_utf8_unchecked(&CAPACITY_REFUSAL) }
}

/// Affine custody of the actual physical snapshot. Transport values cannot
/// replace this owner. No programming, erase, or acceptance entrance is exposed.
pub(crate) struct Inspection {
    frozen: Option<wire::FrozenJournal<NetworkRegion>>,
}
impl Inspection {
    pub(crate) fn region_identity(&self) -> [u8; 32] {
        self.frozen
            .as_ref()
            .expect("inspection owns region")
            .inspect_storage(NetworkRegion::identity)
    }
    pub(crate) fn committed_count(&self) -> u8 {
        self.frozen
            .as_ref()
            .expect("inspection owns region")
            .committed_count()
    }
    pub(crate) fn read_chunk(&mut self, at: u32, out: &mut [u8]) -> Result<(), &'static str> {
        self.frozen
            .as_mut()
            .expect("inspection owns region")
            .read_chunk(at, out)
            .map_err(|_| "network-evidence-snapshot-read")
    }
}
impl Drop for Inspection {
    fn drop(&mut self) {
        if let Some(frozen) = self.frozen.take() {
            STATE.lock(|current| {
                let mut current = current.borrow_mut();
                // initialize refuses every occupied state, including Inspecting.
                assert!(matches!(current.as_ref(), Some(State::Inspecting { .. })));
                *current = Some(State::Frozen(frozen));
            });
        }
    }
}
/// Pin once across bounded export turns; no Root signature or member receipt
/// is inferred from inspection. Dropping restores custody without allocation.
pub(crate) fn inspect() -> Result<Inspection, &'static str> {
    STATE.lock(|current| {
        let mut current = current.borrow_mut();
        if !matches!(current.as_ref(), Some(State::Ready(_) | State::Frozen(_))) {
            return Err("network-evidence-unavailable");
        }
        let frozen = match current.take().expect("checked archive") {
            State::Ready(archive) => archive.journal.freeze(),
            State::Frozen(frozen) => frozen,
            _ => unreachable!(),
        };
        *current = Some(State::Inspecting {
            remaining: frozen.remaining_slots(),
        });
        Ok(Inspection {
            frozen: Some(frozen),
        })
    })
}

#[cfg(test)]
pub(crate) fn exercise_reclaimed_refresh(
    record: &RetiredNetworkControl,
    blank_media: impl FnOnce(),
) {
    let mut owner = inspect().unwrap();
    assert_eq!(owner.committed_count(), 8);
    blank_media();
    let frozen = owner.frozen.take().unwrap();
    STATE.lock(|state| *state.borrow_mut() = Some(State::Reclaimed(frozen)));
    drop(owner);
    assert!(!exhausted());
    // The cached eight-record metadata cannot advertise the erased image.
    assert!(inspect().is_err());
    append(record).unwrap();
    let reopened = inspect().unwrap();
    assert_eq!(reopened.committed_count(), 1);
}

#[path = "native_network_snapshot.rs"]
pub(crate) mod snapshot;

#[cfg(all(feature = "body-persistence", target_os = "none"))]
impl crate::native_restart::network_recovery::Storage for Inspection {
    fn identity(&self) -> [u8; 32] {
        self.region_identity()
    }
    fn capacity(&self) -> u32 {
        self.frozen
            .as_ref()
            .expect("owned inspection")
            .inspect_storage(crate::native_restart::network_recovery::Storage::capacity)
    }
    fn read(&mut self, at: u32, out: &mut [u8]) -> Result<(), &'static str> {
        self.read_chunk(at, out)
    }
    fn erase(
        &mut self,
        _permit: &crate::native_restart::network_recovery::RecoveryPermit,
        _sector: u8,
    ) -> Result<(), &'static str> {
        Err("network-live-requires-wait-permit")
    }
}
#[cfg(all(feature = "body-persistence", target_os = "none"))]
impl Inspection {
    pub(crate) fn erase_closed_sector(
        &mut self,
        authority: &crate::native_restart::network_recovery::RecoveryPermit,
        _wait: &crate::native_flash_window::FlashPermit<'_, '_>,
        sector: u8,
    ) -> Result<(), &'static str> {
        self.frozen
            .as_ref()
            .ok_or("network-recovery-owner")?
            .inspect_storage(|region| region.erase_recovery_sector(authority, sector))
    }
}
#[cfg(all(feature = "body-persistence", target_os = "none"))]
pub(crate) fn publish_reclaimed(
    proof: crate::native_restart::network_recovery::CleanBlank<Inspection>,
) {
    let mut inspection = proof.into_storage();
    let frozen = inspection
        .frozen
        .take()
        .expect("Clean proof owns frozen region");
    STATE.lock(|current| {
        let mut current = current.borrow_mut();
        assert!(matches!(current.as_ref(), Some(State::Inspecting { .. })));
        *current = Some(State::Reclaimed(frozen));
    });
}
