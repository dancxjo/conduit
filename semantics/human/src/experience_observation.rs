//! Shared provenance and freshness envelope for factual observations.

use alloc::string::String;
use conduit_core::{SignId, TemporalInstant};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SourceAvailability {
    Present,
    Missing,
    Unavailable,
}

/// One typed observation with exact source, time, and evidence truth.
///
/// The payload remains owned by its semantic domain. Vision, body state, and
/// future observation families can share this envelope without sharing a
/// domain ontology.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceObservation<T> {
    pub source_identity: String,
    pub availability: SourceAvailability,
    pub value: Option<T>,
    pub observation_sign_id: Option<SignId>,
    pub observed_at: Option<TemporalInstant>,
    pub freshness_limit_ticks: u64,
    pub uncertainty_permille: u16,
    pub calibration_profile_identity: Option<String>,
}
