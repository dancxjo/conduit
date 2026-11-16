//! First local membership over an existing once-born biography.
use super::{BodyLifecycleSession, BodyLifecycleSessionError};
use crate::{
    AuthenticatedHostObservation, BodyBiographyError, BodyBiographyEvidence, BodyMembership,
    BodyState, PartId,
};
use conduit_core::SignId;

impl BodyLifecycleSession {
    /// Prepare the first Host's local membership without repeating birth.
    ///
    /// The caller must independently own the first-host bootstrap decision and
    /// authenticate the supplied observation. These values are reports, not
    /// proof of authority. This method performs no effect, grants no input or
    /// execution authority, and leaves the Body Lulled with no Plan or Play.
    /// The caller must retain the resulting evidence before exposing effects.
    ///
    /// Only pristine birth evidence is accepted. A returning or additional
    /// member must use the ordinary continuity/admission path instead. Borrowing
    /// the input preserves the caller's exact evidence on every refusal.
    pub fn prepare_first_host(
        birth: &BodyBiographyEvidence,
        part: PartId,
        observation: AuthenticatedHostObservation,
        admitted_sign: SignId,
        attached_sign: SignId,
    ) -> Result<Self, BodyLifecycleSessionError> {
        use BodyLifecycleSessionError as E;
        birth.validate().map_err(E::Biography)?;
        if birth.body.state != BodyState::Lulled
            || birth.body.events.len() != 1
            || birth.records.len() != 1
            || !birth.wakes.is_empty()
            || birth.compaction.is_some()
            || birth.emergency.is_some()
            || birth.graduation.is_some()
            || birth.membership
                != BodyMembership::new(birth.body_id.clone()).map_err(E::Membership)?
        {
            return Err(E::Biography(BodyBiographyError::InvalidEvidence));
        }
        let admitted_sequence = birth
            .body
            .birth_sequence
            .checked_add(1)
            .ok_or(E::SequenceExhausted)?;
        let attached_sequence = admitted_sequence
            .checked_add(1)
            .ok_or(E::SequenceExhausted)?;
        let host = observation.host_id.clone();
        let boot = observation.boot_id.clone();
        let mut membership = birth.membership.clone();
        let admitted = membership
            .admit(
                &birth.body_id,
                membership.revision,
                part.clone(),
                observation.proof_id.clone(),
                admitted_sign,
            )
            .map_err(E::Membership)?;
        let attached = membership
            .observe_present(
                &birth.body_id,
                membership.revision,
                &part,
                observation,
                attached_sign,
            )
            .map_err(E::Membership)?;
        let mut evidence = birth.clone();
        evidence
            .append_membership_events(
                membership,
                &[(admitted, admitted_sequence), (attached, attached_sequence)],
            )
            .map_err(E::Biography)?;
        Self::open_admitted(evidence, &host, &boot)
    }
}
