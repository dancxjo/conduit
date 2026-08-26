use super::*;
use crate::{
    encode_song, simple_melody_plan, CreateSpeakerObservation, OiMode, OiPitch, OiSongEvent,
};
use conduit_core::{BootId, HostId, OfferGeneration};

#[derive(Default)]
struct FakeSerial {
    writes: Vec<Vec<u8>>,
    fail_at: Option<usize>,
    observed: bool,
}

impl CreateSpeakerSerial for FakeSerial {
    fn write_exact(&mut self, bytes: &[u8]) -> Result<(), SerialFailure> {
        if self.fail_at == Some(self.writes.len()) {
            return Err(SerialFailure::ProviderLost);
        }
        self.writes.push(bytes.to_vec());
        Ok(())
    }

    fn observe_song_playing(&mut self, _: u8) -> Result<bool, SerialFailure> {
        Ok(self.observed)
    }
}

fn observation() -> CreateSpeakerObservation {
    CreateSpeakerObservation {
        host_id: HostId::from("pete-std-live"),
        boot_id: BootId::from("pete-std-live-boot"),
        offer_generation: OfferGeneration(9),
        serial_base_id: "pete/create1/serial/0".into(),
        robot_identity: "pete/create1/observed-robot".into(),
        robot_identity_verified: true,
        speaker_resource_id: "pete/create1/speaker".into(),
        mode: OiMode::Safe,
        currently_usable: true,
    }
}

fn song() -> EncodedSong {
    encode_song(
        2,
        &[
            OiSongEvent {
                pitch: OiPitch::Note(69),
                duration_ticks: 16,
            },
            OiSongEvent {
                pitch: OiPitch::Rest,
                duration_ticks: 4,
            },
            OiSongEvent {
                pitch: OiPitch::Note(72),
                duration_ticks: 16,
            },
        ],
        MAXIMUM_ADMITTED_SERIAL_BYTES,
    )
    .unwrap()
}

fn prepared() -> PreparedSpeakerExecution {
    let plan = simple_melody_plan(&observation(), true).unwrap();
    prepare_speaker_execution(&plan, &song()).unwrap()
}

#[test]
fn production_kernel_dispatches_exact_define_then_play_and_requires_observation() {
    let mut execution = prepared();
    let mut serial = FakeSerial {
        observed: true,
        ..FakeSerial::default()
    };
    let report = run_speaker_execution(&mut execution, &mut serial);
    assert_eq!(report.terminal, SpeakerTerminal::Completed);
    assert_eq!(report.define_bytes, 9);
    assert_eq!(report.play_bytes, 2);
    assert_eq!(
        serial.writes,
        vec![vec![140, 2, 3, 69, 16, 0, 4, 72, 16], vec![141, 2]]
    );
    assert!(report.kernel_decisions > 0);
    assert!(report.kernel_signs > 0);

    let mut unobserved = prepared();
    let report = run_speaker_execution(&mut unobserved, &mut FakeSerial::default());
    assert_eq!(
        report.terminal,
        SpeakerTerminal::Failed(SerialFailure::SongNotObserved)
    );
}

#[test]
fn provider_loss_before_or_after_define_remains_distinct_and_terminal() {
    for (fail_at, define_bytes) in [(0, 0), (1, 9)] {
        let mut execution = prepared();
        let mut serial = FakeSerial {
            fail_at: Some(fail_at),
            observed: true,
            ..FakeSerial::default()
        };
        let report = run_speaker_execution(&mut execution, &mut serial);
        assert_eq!(
            report.terminal,
            SpeakerTerminal::Failed(SerialFailure::ProviderLost)
        );
        assert_eq!(report.define_bytes, define_bytes);
        assert!(report.kernel_signs > 0);
    }
}

#[test]
fn cancellation_before_dispatch_is_silent_and_after_dispatch_retains_exact_bound() {
    let mut before = prepared();
    assert_eq!(
        cancel_speaker_execution(&mut before).terminal,
        SpeakerTerminal::CancelledBeforeDispatch
    );

    let mut after = prepared();
    let mut serial = FakeSerial {
        observed: true,
        ..FakeSerial::default()
    };
    dispatch_speaker_execution(&mut after, &mut serial).unwrap();
    assert_eq!(
        cancel_speaker_execution(&mut after).terminal,
        SpeakerTerminal::CancelledAfterDispatch {
            maximum_remaining_ticks: 36
        }
    );
}

#[test]
fn stale_plan_identity_and_unsealed_operation_are_refused_before_kernel_start() {
    let mut plan = simple_melody_plan(&observation(), true).unwrap();
    let placement = &mut plan.fragments[0].placements[0];
    placement.implementation_id = conduit_core::ImplementationId::from("wrong/implementation");
    assert!(prepare_speaker_execution(&plan, &song()).is_err());

    let valid = simple_melody_plan(&observation(), true).unwrap();
    let mut wrong_song = song();
    wrong_song.play[1] = 7;
    assert!(prepare_speaker_execution(&valid, &wrong_song).is_err());
}
