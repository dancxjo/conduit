use super::*;
    #[test]
    fn production_kernel_runs_primary_and_degraded_conditions() {
        let primary = run_speech(OutputCondition::PrimaryPlayback, SpeechFault::None).unwrap();
        assert!(matches!(primary.outcome, SpeechOutcome::Played(_)));
        assert!(primary.sign_count > 0);
        assert!(primary.kernel_event_count > 0);
        let degraded = run_speech(OutputCondition::DegradedWavArtifact, SpeechFault::None).unwrap();
        match degraded.outcome {
            SpeechOutcome::WavArtifact(payload) => assert_eq!(*payload.wav_bytes(), 1_260),
            _ => panic!("wrong outcome"),
        }
    }

    #[test]
    fn failures_remain_distinct_and_machine_readable() {
        let cases = [
            (SpeechFault::FormatMismatch, SpeechOutcome::FormatMismatch),
            (SpeechFault::Pressure, SpeechOutcome::Pressure),
            (SpeechFault::Cancelled, SpeechOutcome::Cancelled),
            (SpeechFault::Underrun, SpeechOutcome::Underrun),
            (
                SpeechFault::ImplementationUnavailable,
                SpeechOutcome::ImplementationUnavailable,
            ),
            (SpeechFault::BaseDenied, SpeechOutcome::BaseDenied),
            (SpeechFault::BaseLost, SpeechOutcome::BaseLost),
            (SpeechFault::DeviceFailure, SpeechOutcome::DeviceFailure),
        ];
        for (fault, expected) in cases {
            let receipt = run_speech(OutputCondition::PrimaryPlayback, fault).unwrap();
            assert_eq!(receipt.outcome, expected);
            assert!(!receipt.signs.is_empty());
            let encoded = serde_json::to_string(&receipt.signs).unwrap();
            assert!(!encoded.contains(SPECIMEN_TEXT));
        }
    }

    #[test]
    fn caller_supplied_bounded_text_changes_exact_plan_and_synthesis_identity() {
        let first = run_speech_text(
            "The upstairs temperature is 21 C.",
            OutputCondition::DegradedWavArtifact,
            SpeechFault::None,
        )
        .unwrap();
        let second = run_speech_text(
            "The upstairs temperature is 22 C.",
            OutputCondition::DegradedWavArtifact,
            SpeechFault::None,
        )
        .unwrap();
        assert_ne!(first.plan_id, second.plan_id);
        assert_ne!(first.text_sha256, second.text_sha256);
        assert_ne!(first.outcome, second.outcome);
        let encoded = serde_json::to_string(&first).unwrap();
        assert!(!encoded.contains("upstairs temperature"));
    }

    #[test]
    fn maximum_admitted_text_stays_within_one_pcm_frame_block() {
        let text = "x".repeat(crate::MAXIMUM_TEXT_BYTES as usize);
        let receipt = run_speech_text(
            &text,
            OutputCondition::DegradedWavArtifact,
            SpeechFault::None,
        )
        .unwrap();
        let SpeechOutcome::WavArtifact(payload) = receipt.outcome else {
            panic!("maximum admitted text must produce the degraded artifact");
        };
        assert!(*payload.wav_bytes() <= crate::MAXIMUM_PCM_BYTES);
    }
