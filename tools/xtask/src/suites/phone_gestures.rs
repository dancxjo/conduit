//! Source-linked overlapping phone gestures through the retained contextual owner.
use crate::process::Step;
pub const STEPS: &[Step] = &[
    Step::new(
        "phone-gestures.owner",
        "Check finite selected control profiles, exact overlapping trajectories and refusals",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-speech",
            "--features",
            "semantic-bindings,kernel",
            "--test",
            "phone_gesture_source",
            "--test",
            "phone_gesture_owner",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
    Step::new(
        "phone-gestures.kernel",
        "Replay all four exact Source gesture programs through admitted Std Host Calls",
        "cargo",
        &[
            "test",
            "--locked",
            "-p",
            "conduit-speech",
            "--features",
            "semantic-bindings,kernel",
            "--test",
            "phone_gesture_kernel",
            "--",
            "--nocapture",
            "--test-threads=1",
        ],
    ),
];
