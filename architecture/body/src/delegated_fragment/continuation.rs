//! Separate owner-intent origin. No online command or membership is manufactured.
//! Construction checks the signed retained Wake/full workset and fresh local
//! canonical realizations. Root separately enforces its one-use Boot attempt. There is deliberately no public raw-field approval constructor.
use super::*;
use crate::{ApprovedBodyForePorts, AssignedBodyPreparationProfile, VerifiedContinuationIntent};
use conduit_core as c;

/// A complete freshly sealed local set. Neither a signed intent alone nor a
/// public assigned header can construct this value. Root must obtain it through
/// checked installed recipes, the signed retained Wake/full workset, and fresh
/// current-Boot canonical realizations. It does not allocate a new Wake.
/// ```compile_fail
/// use conduit_body::FreshContinuationScope;
/// let forged = FreshContinuationScope { documents: &[], ..todo!() };
/// ```
pub struct FreshContinuationScope<'a> {
    pub(crate) intent: &'a VerifiedContinuationIntent<'a>,
    pub(crate) documents: &'a [&'a [u8]],
    pub(crate) custody_identity: [u8; 32],
}
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ContinuationScopeRefusal {
    Scope,
    Bounds,
    Fragment(crate::DelegatedFragmentRefusal),
}
impl<'a> FreshContinuationScope<'a> {
    pub fn custody_identity(&self) -> [u8; 32] {
        self.custody_identity
    }
    pub fn fragment_count(&self) -> usize {
        self.documents.len()
    }
    pub fn document(&self, index: usize) -> Option<&'a [u8]> {
        self.documents.get(index).copied()
    }
    pub fn intent(&self) -> &VerifiedContinuationIntent<'a> {
        self.intent
    }
    /// Recheck actual current receiver facts and unchanged verified permission.
    /// No field here certifies durable media or authorizes a new membership.
    pub fn check_actual(
        &self,
        current: &VerifiedContinuationIntent<'_>,
        actual: &c::HostAdvertisement,
    ) -> Result<(), ContinuationScopeRefusal> {
        use ContinuationScopeRefusal as E;
        let intent = current.intent();
        if current.intent().signing_transcript() != self.intent.intent().signing_transcript()
            || current.actual_boot() != self.intent.actual_boot()
            || actual.protocol_version != c::PROTOCOL_VERSION
            || actual.host_id.as_str() != intent.host_id
            || actual.boot_id.as_str() != current.actual_boot()
            || actual.profile.as_str() != intent.configuration_id
            || actual.offer_generation.0 == 0
            || self.custody_identity == [0; 32]
        {
            return Err(E::Scope);
        }
        if self.documents.is_empty()
            || self
                .documents
                .iter()
                .any(|d| decode_body_fragment_document(d).is_err())
            || self.documents.len() > crate::MAX_BODY_PLOTS
            || self
                .documents
                .iter()
                .try_fold(10usize, |n, d| n.checked_add(4)?.checked_add(d.len()))
                .is_none_or(|n| n as u64 > u64::from(intent.ceilings.assignment_bytes))
        {
            return Err(E::Bounds);
        }
        let metadata = |i| {
            decode_body_fragment_document(self.documents[i])
                .expect("checked immutable document")
                .0
        };
        if !workset::matches_local_metadata(self.documents.len(), metadata)
            || (0..self.documents.len()).any(|i| {
                let m = metadata(i);
                m.body != intent.body_id
                    || m.host != intent.host_id
                    || m.boot != current.actual_boot()
                    || m.offer_generation != actual.offer_generation.0
            })
        {
            return Err(E::Scope);
        }
        Ok(())
    }
    /// Same checked assigned/profile/resource decoder as online preparation,
    /// reached through a different opaque origin. No owner command is invented.
    pub fn approve_fragment<const PORTS: usize>(
        &self,
        index: usize,
        current: &VerifiedContinuationIntent<'_>,
        actual: &c::HostAdvertisement,
        profile: AssignedBodyPreparationProfile<'_>,
    ) -> Result<
        (
            crate::DelegatedBodyFragment<'a>,
            ApprovedBodyForePorts<PORTS>,
        ),
        ContinuationScopeRefusal,
    > {
        use ContinuationScopeRefusal as E;
        self.check_actual(current, actual)?;
        let (metadata, projection) =
            decode_body_fragment_document(self.document(index).ok_or(E::Scope)?)
                .map_err(|e| E::Fragment(crate::DelegatedFragmentRefusal::Document(e)))?;
        approve_checked(
            metadata,
            projection,
            actual,
            current.intent().body_id,
            current.intent().ceilings,
            profile,
        )
        .map_err(E::Fragment)
    }
    pub fn matches_fragment(&self, fragment: &crate::DelegatedBodyFragment<'_>) -> bool {
        self.documents
            .iter()
            .any(|d| decode_body_fragment_document(d).is_ok_and(|(m, _)| &m == fragment.metadata()))
    }
}

#[path = "continuation_verifier.rs"]
mod verifier;
pub use verifier::*;
#[allow(clippy::too_many_arguments)]
fn approve_checked<'a, const PORTS: usize>(
    metadata: BodyFragmentApprovalMetadata<'a>,
    projection: &'a [u8],
    actual: &c::HostAdvertisement,
    body: &str,
    ceilings: crate::ContinuationCeilings,
    profile: AssignedBodyPreparationProfile<'_>,
) -> Result<
    (
        crate::DelegatedBodyFragment<'a>,
        ApprovedBodyForePorts<PORTS>,
    ),
    crate::DelegatedFragmentRefusal,
> {
    if let Some(pins) = profile.fore {
        let fore = ApprovedBodyForePorts::copy_pinned(pins)
            .ok_or(crate::DelegatedFragmentRefusal::ForeApproval)?;
        let fragment = receiving::finish_checked(
            metadata,
            projection,
            actual,
            body,
            ceilings.assignment_bytes,
            ceilings.runtime_bytes,
            profile,
        )?;
        Ok((fragment, fore))
    } else {
        approved_fore::finish_approved_fore_checked(
            metadata,
            projection,
            actual,
            body,
            ceilings.assignment_bytes,
            ceilings.runtime_bytes,
            profile,
        )
    }
}
