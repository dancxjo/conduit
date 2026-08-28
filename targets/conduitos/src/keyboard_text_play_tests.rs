use conduit_core::{KeyEvent, KeyModifiers, KeyTransition};
use conduit_kernel::scheduler::SchedulerStatus;

use crate::{
    identity::BootIdentities,
    keyboard_offer::KeyboardRealization,
    keyboard_text_plan,
    keyboard_text_play::{KeyboardTextKernel, run},
    offer::{CpuFeatures, HostOffer},
};

fn prepared() -> keyboard_text_plan::PreparedKeyboardTextPlay {
    let identities = BootIdentities {
        host: [1; 32],
        boot: [2; 32],
    };
    let offer = HostOffer::new(
        &identities,
        "build",
        CpuFeatures {
            sse2: true,
            rdrand: true,
            invariant_tsc: true,
        },
        1_048_576,
    )
    .with_keyboard(
        KeyboardRealization {
            controller_id: [3; 32],
            device_id: [4; 32],
            interface_id: [5; 32],
            endpoint_id: [6; 32],
            report_buffers: 2,
            transition_slots: 8,
            operation_slots: 2,
        },
        "build",
    )
    .unwrap();
    keyboard_text_plan::prepare(&identities, &offer, "build").unwrap()
}

fn press(usage: u8, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Pressed, modifiers).unwrap()
}

fn release(usage: u8, modifiers: KeyModifiers) -> KeyEvent {
    KeyEvent::new(usage, KeyTransition::Released, modifiers).unwrap()
}

#[test]
fn hello_and_unicode_cross_the_exact_four_gear_production_play() {
    let events = [
        press(0x0b, KeyModifiers::NONE),
        release(0x0b, KeyModifiers::NONE),
        press(0x08, KeyModifiers::NONE),
        release(0x08, KeyModifiers::NONE),
        press(0x0f, KeyModifiers::NONE),
        release(0x0f, KeyModifiers::NONE),
        press(0x0f, KeyModifiers::NONE),
        release(0x0f, KeyModifiers::NONE),
        press(0x12, KeyModifiers::NONE),
        release(0x12, KeyModifiers::NONE),
        press(0x04, KeyModifiers::RIGHT_ALT),
        release(0x04, KeyModifiers::RIGHT_ALT),
    ];
    let report = run(&prepared(), &events).unwrap();
    let expected = ["H", "E", "L", "L", "O", "Æ"];
    assert_eq!(usize::from(report.presentation_count), expected.len());
    for (actual, expected) in report.presentations.iter().zip(expected) {
        assert_eq!(actual.unwrap().as_bytes(), expected.as_bytes());
    }
    assert!(report.completed);
    assert!(report.decisions > 0);
}

#[test]
fn cancellation_resets_semantic_state_and_rejects_late_work() {
    let mut kernel = KeyboardTextKernel::prepare(&prepared(), 2).unwrap();
    assert!(matches!(
        kernel.step(),
        Ok(SchedulerStatus::Progress { .. })
    ));
    assert!(kernel.next_host_request().is_some());
    kernel.cancel().unwrap();
    assert_eq!(kernel.step(), Ok(SchedulerStatus::Cancelled));
}
