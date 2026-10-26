//! Actual append-only custody for retired network control evidence.
use super::RetiredNetworkControl;
use crate::native_evidence_region::NetworkRegion;
use conduit_network_join_call::recovery_evidence as wire;
use core::cell::RefCell;
use embassy_sync::blocking_mutex::{raw::CriticalSectionRawMutex, Mutex};
struct Archive {
    journal: wire::Journal<NetworkRegion>,
    work: alloc::boxed::Box<[u8; wire::MAXIMUM_BYTES]>,
}
enum State {
    Ready(Archive),
    Retained { _region: NetworkRegion },
}
static STATE: Mutex<CriticalSectionRawMutex, RefCell<Option<State>>> =
    Mutex::new(RefCell::new(None));
pub fn initialize(region: NetworkRegion) -> Result<(), &'static str> {
    let mut work = alloc::boxed::Box::new([0; wire::MAXIMUM_BYTES]);
    let (state, result) = match wire::Journal::open(region, &mut work) {
        Ok(journal) => (State::Ready(Archive { journal, work }), Ok(())),
        Err((region, _)) => (
            State::Retained { _region: region },
            Err("network-evidence-retained-refusal"),
        ),
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
pub(super) fn append(retired: &RetiredNetworkControl) -> Result<(), &'static str> {
    STATE.lock(|current| {
        let mut current = current.borrow_mut();
        let Some(State::Ready(archive)) = current.as_mut() else {
            return Err("network-evidence-unavailable");
        };
        let signs = retired.evidence.kernel.signs();
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
            &mut archive.work,
        )
        .map_err(|_| "network-evidence-encoding")?;
        archive
            .journal
            .append(&archive.work)
            .map_err(|_| "network-evidence-commit")?;
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
