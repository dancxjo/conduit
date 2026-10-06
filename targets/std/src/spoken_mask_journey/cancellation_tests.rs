//! Real provider cancellation after observed PCM, not after a guessed sleep.
use super::*;
use crate::hosted_process::stream::observations;
use std::{cell::RefCell, rc::Rc};

#[test]
#[ignore = "requires installed eSpeak NG; observes actual PCM, cancels and verifies no completed Mask artifact"]
fn streamed_mask_stop_reaps_provider_and_never_acknowledges_partial_audio() {
    let directory = std::env::temp_dir().join(format!(
        "conduit-streamed-mask-cancel-{}",
        std::process::id()
    ));
    std::fs::create_dir(&directory).unwrap();
    let destination = directory.join("cancelled.wav");
    let engine = std::fs::canonicalize("/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1").unwrap();
    let discovery = crate::hosted_speech_synthesis::EspeakDiscovery::inspect(
        std::path::Path::new("/usr/bin/espeak-ng"),
        std::path::Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
        "en-us",
        &[engine],
    )
    .unwrap();
    let coverage = crate::hosted_language::tests::fixture_coverage(
        &discovery.provider_identity(),
        "en-us",
        "language/english",
    );
    let discovery = discovery.declare_language_coverage(coverage).unwrap();
    let words = "This Body keeps the clock you started. Change the interval, then inspect the connections to see how your action reaches the running work. You can pause the Body without erasing its history. When you return, inspect the current host and the new plan before starting again.";
    let face = tests::presentation_with_text(words);
    let candidate = tests::retained(face.clone());
    let control = crate::RunControl::default();
    let stop_id = crate::RunControlRequestId::new("stop/after-real-pcm").unwrap();
    let observed = Rc::new(RefCell::new((None, false, Vec::new())));
    let capture = observed.clone();
    let request_control = control.clone();
    let request_id = stop_id.clone();
    let partial_directory = directory.clone();
    let _observer = observations::observe(move |event| {
        let mut capture = capture.borrow_mut();
        if let Some(reaped) = event.reaped {
            capture.2.push((event.pid, reaped));
        } else if capture.0.is_none() && event.stdout_bytes >= 44 + 64 * 50 {
            capture.0 = Some(event.pid);
            capture.1 = std::fs::read_dir(&partial_directory).unwrap().any(|entry| {
                let entry = entry.unwrap();
                entry.file_name().to_string_lossy().contains(".partial-")
                    && entry.metadata().unwrap().len() > 44
            });
            request_control.request_stop(request_id.clone()).unwrap();
        }
    });
    let run = run::run_mask(
        "stream-cancel",
        "stream-cancel",
        face,
        candidate,
        run::SpeechPreparation {
            real: Some((discovery, &destination)),
            streaming: true,
            language: &crate::hosted_language::tests::request("language/english"),
        },
        &control,
    )
    .unwrap();
    let (pid, partial_written, retired) = &*observed.borrow();
    let pid = pid.expect("the provider actually produced PCM before cancellation");
    assert!(
        *partial_written,
        "a partial WAV received real PCM before stop"
    );
    assert_eq!(
        retired.as_slice(),
        &[(pid, true)],
        "the exact launched provider was reaped once"
    );
    assert!(
        run.deliveries.is_empty(),
        "cancelled synthesis must not emit an acknowledged Show"
    );
    assert!(!destination.exists());
    assert_eq!(
        std::fs::read_dir(&directory).unwrap().count(),
        0,
        "partial artifact retired"
    );
    assert!(matches!(
        run.report.observations.last().map(|entry| &entry.kind),
        Some(conduit_core::ObservationKind::PlanTerminal {
            disposition: conduit_core::TerminalDisposition::Cancelled { .. }
        })
    ));
    assert_eq!(run.report.control_receipts.len(), 1);
    assert_eq!(run.report.control_receipts[0].request_id, stop_id);
    assert_eq!(
        run.report.control_receipts[0].disposition,
        crate::RunControlDisposition::Accepted
    );
    std::fs::remove_dir(directory).unwrap();
}
