#![no_std]

//! Fixed-storage, offline emergency-keyword spotting over validated PCM frames.
//!
//! This mechanism retains compact integer features, never PCM. It recognizes
//! only three admitted templates and emits inert word identities; sequencing
//! and emergency authority remain outside this crate.

pub const PROFILE_ID: &str = "conduit.reference/emergency-keyword-spotter-s16le-16k@1";
pub const SAMPLE_RATE_HZ: u32 = 16_000;
pub const SAMPLES_PER_FRAME: usize = 160;
pub const FEATURE_SEGMENTS: usize = 8;
pub const FEATURE_DIMENSIONS: usize = FEATURE_SEGMENTS + 1;
pub const TEMPLATE_FRAMES: usize = 8;
pub const TARGET_WORDS: usize = 3;
pub const MAXIMUM_WORK_UNITS_PER_FRAME: u32 = (SAMPLES_PER_FRAME - 1) as u32
    + SAMPLES_PER_FRAME as u32
    + (TARGET_WORDS * TEMPLATE_FRAMES * FEATURE_DIMENSIONS) as u32;

pub type FeatureVector = [u16; FEATURE_DIMENSIONS];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct KeywordTemplate {
    pub word_id: u8,
    pub frames: [FeatureVector; TEMPLATE_FRAMES],
    pub maximum_distance: u32,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct ValidatedPcmFrame<'a> {
    pub microphone_generation: u64,
    pub sequence: u64,
    pub samples: &'a [i16; SAMPLES_PER_FRAME],
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectorDecision {
    Accumulating,
    NoMatch,
    Detected {
        word_id: u8,
        window_end_sequence: u64,
        confidence_millionths: u32,
    },
    Refused(DetectorRefusal),
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum DetectorRefusal {
    DuplicateWord,
    EmptyThreshold,
    AmbiguousMatch,
    StaleGeneration,
    ReplayedFrame,
    SequenceGap,
    MicrophoneLost,
    InputOverflow,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct DetectorBounds {
    pub raw_audio_bytes_retained: usize,
    pub feature_bytes_retained: usize,
    pub template_bytes_retained: usize,
    pub maximum_work_units_per_frame: u32,
}

pub struct EmergencyKeywordSpotter {
    generation: u64,
    templates: [KeywordTemplate; TARGET_WORDS],
    features: [FeatureVector; TEMPLATE_FRAMES],
    retained: u8,
    next_slot: u8,
    last_sequence: Option<u64>,
}

impl EmergencyKeywordSpotter {
    pub fn new(
        generation: u64,
        templates: [KeywordTemplate; TARGET_WORDS],
    ) -> Result<Self, DetectorRefusal> {
        for (index, template) in templates.iter().enumerate() {
            if template.maximum_distance == 0 {
                return Err(DetectorRefusal::EmptyThreshold);
            }
            if templates[..index]
                .iter()
                .any(|prior| prior.word_id == template.word_id)
            {
                return Err(DetectorRefusal::DuplicateWord);
            }
        }
        Ok(Self {
            generation,
            templates,
            features: [[0; FEATURE_DIMENSIONS]; TEMPLATE_FRAMES],
            retained: 0,
            next_slot: 0,
            last_sequence: None,
        })
    }

    pub const fn bounds() -> DetectorBounds {
        DetectorBounds {
            raw_audio_bytes_retained: 0,
            feature_bytes_retained: core::mem::size_of::<[FeatureVector; TEMPLATE_FRAMES]>(),
            template_bytes_retained: core::mem::size_of::<[KeywordTemplate; TARGET_WORDS]>(),
            maximum_work_units_per_frame: MAXIMUM_WORK_UNITS_PER_FRAME,
        }
    }

    pub fn observe(&mut self, frame: ValidatedPcmFrame<'_>) -> DetectorDecision {
        if frame.microphone_generation != self.generation {
            self.clear();
            return DetectorDecision::Refused(DetectorRefusal::StaleGeneration);
        }
        if let Some(last) = self.last_sequence {
            if frame.sequence <= last {
                self.clear();
                return DetectorDecision::Refused(DetectorRefusal::ReplayedFrame);
            }
            if frame.sequence != last.saturating_add(1) {
                self.clear();
                self.last_sequence = Some(frame.sequence);
                return DetectorDecision::Refused(DetectorRefusal::SequenceGap);
            }
        }
        self.last_sequence = Some(frame.sequence);
        self.features[usize::from(self.next_slot)] = extract_features(frame.samples);
        self.next_slot = (self.next_slot + 1) % TEMPLATE_FRAMES as u8;
        self.retained = self.retained.saturating_add(1).min(TEMPLATE_FRAMES as u8);
        if usize::from(self.retained) < TEMPLATE_FRAMES {
            return DetectorDecision::Accumulating;
        }

        let mut accepted: Option<(u8, u32, u32)> = None;
        for template in &self.templates {
            let distance = self.distance(template);
            if distance <= template.maximum_distance {
                let confidence = (1_000_000_u64.saturating_sub(
                    u64::from(distance) * 1_000_000 / u64::from(template.maximum_distance),
                )) as u32;
                if accepted.is_some() {
                    self.clear_window();
                    return DetectorDecision::Refused(DetectorRefusal::AmbiguousMatch);
                }
                accepted = Some((template.word_id, distance, confidence));
            }
        }
        if let Some((word_id, _, confidence_millionths)) = accepted {
            self.clear_window();
            DetectorDecision::Detected {
                word_id,
                window_end_sequence: frame.sequence,
                confidence_millionths,
            }
        } else {
            DetectorDecision::NoMatch
        }
    }

    pub fn microphone_lost(&mut self) -> DetectorDecision {
        self.clear();
        DetectorDecision::Refused(DetectorRefusal::MicrophoneLost)
    }

    pub fn input_overflow(&mut self) -> DetectorDecision {
        self.clear();
        DetectorDecision::Refused(DetectorRefusal::InputOverflow)
    }

    fn distance(&self, template: &KeywordTemplate) -> u32 {
        let mut distance = 0_u32;
        for offset in 0..TEMPLATE_FRAMES {
            let observed = &self.features[(usize::from(self.next_slot) + offset) % TEMPLATE_FRAMES];
            for (actual, expected) in observed.iter().zip(template.frames[offset]) {
                distance = distance.saturating_add(u32::from(actual.abs_diff(expected)));
            }
        }
        distance
    }

    fn clear_window(&mut self) {
        self.retained = 0;
        self.next_slot = 0;
        self.features = [[0; FEATURE_DIMENSIONS]; TEMPLATE_FRAMES];
    }

    fn clear(&mut self) {
        self.clear_window();
        self.last_sequence = None;
    }
}

pub fn extract_features(samples: &[i16; SAMPLES_PER_FRAME]) -> FeatureVector {
    let mut features = [0_u16; FEATURE_DIMENSIONS];
    let segment_samples = SAMPLES_PER_FRAME / FEATURE_SEGMENTS;
    for (segment, output) in features[..FEATURE_SEGMENTS].iter_mut().enumerate() {
        let start = segment * segment_samples;
        let mut magnitude = 0_u32;
        for sample in &samples[start..start + segment_samples] {
            magnitude = magnitude.saturating_add(u32::from(sample.unsigned_abs()));
        }
        *output = (magnitude / segment_samples as u32).min(u32::from(u16::MAX)) as u16;
    }
    let crossings = samples
        .windows(2)
        .filter(|pair| (pair[0] < 0) != (pair[1] < 0))
        .count();
    features[FEATURE_SEGMENTS] =
        ((crossings as u32 * u32::from(u16::MAX)) / (SAMPLES_PER_FRAME - 1) as u32) as u16;
    features
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(amplitude: i16, period: usize) -> [i16; SAMPLES_PER_FRAME] {
        core::array::from_fn(|index| {
            if (index / period).is_multiple_of(2) {
                amplitude
            } else {
                -amplitude
            }
        })
    }

    fn word(amplitude: i16, period: usize) -> [[i16; SAMPLES_PER_FRAME]; TEMPLATE_FRAMES] {
        core::array::from_fn(|index| frame(amplitude + index as i16 * 20, period))
    }

    fn template(
        word_id: u8,
        audio: &[[i16; SAMPLES_PER_FRAME]; TEMPLATE_FRAMES],
    ) -> KeywordTemplate {
        KeywordTemplate {
            word_id,
            frames: core::array::from_fn(|index| extract_features(&audio[index])),
            maximum_distance: 500,
        }
    }

    fn fixtures() -> (
        [KeywordTemplate; TARGET_WORDS],
        [[i16; SAMPLES_PER_FRAME]; TEMPLATE_FRAMES],
    ) {
        let first = word(1_000, 16);
        let second = word(2_000, 10);
        let third = word(3_000, 6);
        (
            [
                template(0, &first),
                template(1, &second),
                template(2, &third),
            ],
            first,
        )
    }

    #[test]
    fn exact_fixed_word_is_detected_once_without_retaining_pcm() {
        let (templates, audio) = fixtures();
        let mut detector = EmergencyKeywordSpotter::new(7, templates).unwrap();
        for (index, samples) in audio.iter().enumerate() {
            let decision = detector.observe(ValidatedPcmFrame {
                microphone_generation: 7,
                sequence: index as u64,
                samples,
            });
            if index + 1 == TEMPLATE_FRAMES {
                assert_eq!(
                    decision,
                    DetectorDecision::Detected {
                        word_id: 0,
                        window_end_sequence: 7,
                        confidence_millionths: 1_000_000,
                    }
                );
            } else {
                assert_eq!(decision, DetectorDecision::Accumulating);
            }
        }
        assert_eq!(
            EmergencyKeywordSpotter::bounds().raw_audio_bytes_retained,
            0
        );
    }

    #[test]
    fn ordinary_silence_tones_and_alternating_noise_do_not_match() {
        let (templates, _) = fixtures();
        for negative in [frame(0, 1), frame(8_000, 40), frame(15_000, 2)] {
            let mut detector = EmergencyKeywordSpotter::new(1, templates).unwrap();
            let mut detected = false;
            for sequence in 0..24 {
                detected |= matches!(
                    detector.observe(ValidatedPcmFrame {
                        microphone_generation: 1,
                        sequence,
                        samples: &negative,
                    }),
                    DetectorDecision::Detected { .. }
                );
            }
            assert!(!detected);
        }
    }

    #[test]
    fn replay_gap_loss_overflow_and_stale_generation_clear_partial_windows() {
        let (templates, audio) = fixtures();
        for refusal in [
            DetectorRefusal::MicrophoneLost,
            DetectorRefusal::InputOverflow,
            DetectorRefusal::StaleGeneration,
            DetectorRefusal::ReplayedFrame,
            DetectorRefusal::SequenceGap,
        ] {
            let mut detector = EmergencyKeywordSpotter::new(7, templates).unwrap();
            for (sequence, samples) in audio.iter().take(4).enumerate() {
                detector.observe(ValidatedPcmFrame {
                    microphone_generation: 7,
                    sequence: sequence as u64,
                    samples,
                });
            }
            let decision = match refusal {
                DetectorRefusal::MicrophoneLost => detector.microphone_lost(),
                DetectorRefusal::InputOverflow => detector.input_overflow(),
                DetectorRefusal::StaleGeneration => detector.observe(ValidatedPcmFrame {
                    microphone_generation: 6,
                    sequence: 4,
                    samples: &audio[4],
                }),
                DetectorRefusal::ReplayedFrame => detector.observe(ValidatedPcmFrame {
                    microphone_generation: 7,
                    sequence: 3,
                    samples: &audio[4],
                }),
                DetectorRefusal::SequenceGap => detector.observe(ValidatedPcmFrame {
                    microphone_generation: 7,
                    sequence: 8,
                    samples: &audio[4],
                }),
                _ => unreachable!(),
            };
            assert_eq!(decision, DetectorDecision::Refused(refusal));
            assert_eq!(detector.retained, 0);
        }
    }

    #[test]
    fn construction_refuses_ambiguous_identity_and_zero_threshold() {
        let (mut templates, _) = fixtures();
        templates[1].word_id = templates[0].word_id;
        assert!(matches!(
            EmergencyKeywordSpotter::new(1, templates),
            Err(DetectorRefusal::DuplicateWord)
        ));
        let (mut templates, _) = fixtures();
        templates[0].maximum_distance = 0;
        assert!(matches!(
            EmergencyKeywordSpotter::new(1, templates),
            Err(DetectorRefusal::EmptyThreshold)
        ));
    }

    #[test]
    fn storage_and_work_bounds_are_exact_and_small() {
        let bounds = EmergencyKeywordSpotter::bounds();
        assert_eq!(bounds.raw_audio_bytes_retained, 0);
        assert_eq!(bounds.feature_bytes_retained, 144);
        assert!(bounds.template_bytes_retained <= 512);
        assert_eq!(bounds.maximum_work_units_per_frame, 535);
        assert!(core::mem::size_of::<EmergencyKeywordSpotter>() <= 704);
    }
}
