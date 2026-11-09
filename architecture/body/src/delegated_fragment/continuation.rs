//! Separate owner-intent origin. No online command or membership is manufactured.
//! Construction belongs to canonical bundle checking PLUS committed fresh-scope
//! allocation. There is deliberately no public raw-field approval constructor.
use super::*;
use crate::{ApprovedBodyForePorts, AssignedBodyPreparationProfile, VerifiedContinuationIntent};
use conduit_core as c;

/// A complete freshly sealed local set. Neither a signed intent alone nor a
/// public assigned header can construct this value. Root must obtain it through
/// checked installed recipes and committed unique lifecycle scope allocation.
/// ```compile_fail
/// use conduit_body::FreshContinuationScope;
/// let forged = FreshContinuationScope { documents: &[], ..todo!() };
/// ```
pub struct FreshContinuationScope<'a> {
    pub(crate) intent: &'a VerifiedContinuationIntent<'a>,
    pub(crate) documents: &'a [&'a [u8]],
    pub(crate) expected: &'a [BodyFragmentApprovalMetadata<'a>],
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
            || self.documents.len() != self.expected.len()
            || self.documents.len() > crate::MAX_BODY_PLOTS
            || self
                .documents
                .iter()
                .try_fold(0usize, |n, d| n.checked_add(d.len()))
                .is_none_or(|n| n as u64 > u64::from(intent.ceilings.assignment_bytes))
        {
            return Err(E::Bounds);
        }
        if !workset::matches_local_metadata(self.expected.len(), |i| self.expected[i])
            || self.expected.iter().any(|m| {
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
        if self.expected.get(index) != Some(&metadata) {
            return Err(E::Scope);
        }
        let fore =
            ApprovedBodyForePorts::copy_pinned(profile.fore.unwrap_or(&[])).ok_or(E::Bounds)?;
        let ceilings = current.intent().ceilings;
        let fragment = receiving::finish_checked(
            metadata,
            projection,
            actual,
            current.intent().body_id,
            ceilings.assignment_bytes,
            ceilings.runtime_bytes,
            profile,
        )
        .map_err(E::Fragment)?;
        Ok((fragment, fore))
    }
    pub fn matches_fragment(&self, fragment: &crate::DelegatedBodyFragment<'_>) -> bool {
        self.expected.contains(fragment.metadata())
    }
}
