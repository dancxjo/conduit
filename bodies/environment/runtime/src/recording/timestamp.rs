//! Meaning of a producer-Boot timestamp, independently of wall correlation.
//! Observation/publication time must never be promoted to acquisition time.

pub use super::forms::RecordingTimestampBasis;
impl RecordingTimestampBasis {
    /// Exact content profile; the existing acquisition representation is stable.
    pub const fn clock_profile(self) -> &'static str {
        match self {
            Self::Acquisition => "producer-boot-ms",
            Self::ProducerObservation => "producer-observation-boot-ms",
        }
    }
    pub const fn time_property(self) -> &'static str {
        match self {
            Self::Acquisition => "sampled-at-boot-ms",
            Self::ProducerObservation => "observed-at-boot-ms",
        }
    }
}
