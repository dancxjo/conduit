use conduit_audio::*;
use conduit_plot::rust_binding::{BoundedSequence, NativeRustBinding};
pub fn anchor(timeline: u64, origin: u64) -> AudioTrajectoryAnchor {
    AudioTrajectoryAnchor::new(
        AudioOriginIdentity::new(origin).unwrap(),
        AudioTimelineIdentity::new(timeline).unwrap(),
    )
    .unwrap()
}
pub fn time(n: u64, d: u64) -> AudioExactTimeOffset {
    AudioExactTimeOffset::new(d, n).unwrap()
}
pub fn hz(n: u64, d: u64) -> AudioTrajectoryQuantity {
    AudioTrajectoryQuantity::frequency(d, n).unwrap()
}
pub fn cycle(n: u64, d: u64) -> AudioTrajectoryQuantity {
    AudioTrajectoryQuantity::cycle(d, n).unwrap()
}
pub fn amplitude(n: u64, d: u64) -> AudioTrajectoryQuantity {
    AudioTrajectoryQuantity::amplitude(d, n).unwrap()
}
pub fn segment(
    start: AudioExactTimeOffset,
    end: AudioExactTimeOffset,
    left: AudioTrajectoryQuantity,
    right: AudioTrajectoryQuantity,
    interpolation: AudioTrajectoryInterpolation,
) -> AudioTrajectorySegment {
    AudioTrajectorySegment::new(end, interpolation, left, right, start).unwrap()
}
pub fn trajectory(segments: Vec<AudioTrajectorySegment>) -> AudioQuantityTrajectory {
    AudioQuantityTrajectory::new(
        anchor(1, 1),
        AudioTrajectoryEndpoints::RightContinuousFinalIncluded,
        AudioTrajectoryOutside::Refuse,
        AudioTrajectoryProvenance::new(
            AudioTrajectoryProvenanceKind::Authored,
            "trajectory-independent-reference".into(),
            Some("v1".into()),
        )
        .unwrap(),
        BoundedSequence::try_from_iter(segments).unwrap(),
    )
    .unwrap()
}
pub fn prepare(
    value: &AudioQuantityTrajectory,
) -> Result<PreparedAudioQuantityTrajectory, AudioTrajectoryRefusal> {
    PreparedAudioQuantityTrajectory::new(&value.clone().encode().unwrap())
}
pub fn query(time: AudioExactTimeOffset) -> Vec<u8> {
    AudioTrajectoryQuery::new(anchor(1, 1), time)
        .unwrap()
        .encode()
        .unwrap()
}
pub fn ratio(value: &AudioTrajectoryQuantity) -> (u128, u128) {
    match value {
        AudioTrajectoryQuantity::Frequency(v) => {
            (*v.numerator_hz() as u128, *v.denominator() as u128)
        }
        AudioTrajectoryQuantity::Cycle(v) => {
            (*v.numerator_seconds() as u128, *v.denominator() as u128)
        }
        AudioTrajectoryQuantity::Amplitude(v) => (*v.numerator() as u128, *v.denominator() as u128),
        AudioTrajectoryQuantity::Power(v) => (*v.numerator() as u128, *v.denominator() as u128),
    }
}
