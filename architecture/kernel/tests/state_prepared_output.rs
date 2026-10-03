//! Fixed large State uses the same transactional kernel and finite store.
use conduit_kernel::state_delay::{back::StateBack, StateDelay};
use conduit_kernel::{CordId, PortId, RemoteEndpointId, ValueStorage};
#[path = "common/state_play.rs"]
mod common;
use common::{deliver, idle, with_back};

const BYTES: usize = 256;
fn play() -> common::Play<BYTES> {
    let state = StateDelay::externally_continued(0, BYTES, &[1; BYTES]).unwrap();
    with_back(StateBack::new_prepared(state, PortId(0), PortId(0)).unwrap())
}

#[test]
fn large_initialization_updates_and_pressure_preserve_exact_committed_values() {
    let mut play = play();
    idle(&mut play);
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[2; BYTES])
        .unwrap();
    for _ in 0..1000 {
        idle(&mut play);
    }
    assert_eq!(play.drivers()[0].state().generation(), 0);
    assert_eq!(play.drivers()[0].state().current(), &[1; BYTES]);
    deliver(&mut play, 0, &[1; BYTES]);
    idle(&mut play);
    deliver(&mut play, 1, &[2; BYTES]);
    for sequence in 1..16 {
        let bytes = [sequence as u8; BYTES];
        play.admit_remote_input(RemoteEndpointId(0), CordId(0), sequence, &bytes)
            .unwrap();
        idle(&mut play);
        deliver(&mut play, sequence + 1, &bytes);
        assert_eq!(play.drivers()[0].state().generation(), sequence + 1);
        assert_eq!(play.drivers()[0].state().current(), &bytes);
        assert_eq!(play.values().used_items(), 0);
    }
}

#[test]
fn cancellation_during_pressure_retains_only_the_published_generation() {
    let mut play = play();
    idle(&mut play);
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[2; BYTES])
        .unwrap();
    idle(&mut play);
    play.cancel().unwrap();
    let retired = play
        .try_retire()
        .ok()
        .expect("cancelled finite state retires");
    assert_eq!(retired.values.used_items(), 0);
    let [back] = retired.drivers;
    let state = back.into_state();
    assert_eq!(state.current(), &[1; BYTES]);
    assert_eq!(state.generation(), 0);
}

#[test]
fn prepared_state_preserves_empty_and_actual_short_extent() {
    let state = StateDelay::<BYTES>::externally_continued(0, BYTES, &[]).unwrap();
    let mut play = with_back(StateBack::new_prepared(state, PortId(0), PortId(0)).unwrap());
    idle(&mut play);
    deliver(&mut play, 0, &[]);
    play.admit_remote_input(RemoteEndpointId(0), CordId(0), 0, &[7; 101])
        .unwrap();
    idle(&mut play);
    deliver(&mut play, 1, &[7; 101]);
    assert_eq!(play.drivers()[0].state().current(), &[7; 101]);
}
