//! Public SAFE checker for a local candidate, never an owner-global transition.
use super::*;
use crate::{
    body_plan_commitment_digest, body_play_identity_matches, continuation_bundle_digest,
    continuation_scope_digest, BodyPlotSeal, CheckedContinuationBundle, ContinuationBundleEntry,
    ContinuationFullScope, MAX_BODY_PLOTS,
};
use sha2::{Digest, Sha256};
#[path = "continuation_bindings.rs"]
mod bindings;

/// Candidate bytes and typed fresh facts, not approval. Native possession stays
/// with the original Root and must still pass ordinary issuance/admission.
pub struct ContinuationLocalEntry<'a> {
    pub recipe: &'a [c::FragmentSealPiece<'a>],
    pub bindings: &'a [c::FragmentSealBinding<'a>],
    pub template: &'a [u8],
    pub native: &'a [c::FreshAssignedNative],
    pub fore: &'a [c::FreshAssignedFore<'a>],
    pub profile: AssignedBodyPreparationProfile<'a>,
}
impl<'a> FreshContinuationScope<'a> {
    /// Check a complete LOCAL replacement inside the exact authorized FULL
    /// workset. Nonlocal Plot commitments remain unchanged; none is Started or
    /// declared current here. The signed retained Wake survives this Boot.
    ///
    /// Scratch is caller-funded temporary preparation, not an enlarged grant.
    /// No reservation, provider transfer, membership or global Playing occurs.
    /// Root must consume the returned origin through its one-use local Start.
    #[allow(clippy::too_many_arguments)]
    pub fn check_local_replacement<const PORTS: usize>(
        intent: &'a VerifiedContinuationIntent<'a>,
        bundle: &CheckedContinuationBundle,
        full: &ContinuationFullScope<'_>,
        actual: &c::HostAdvertisement,
        pins: &[ContinuationBundleEntry<'_>],
        entries: &[ContinuationLocalEntry<'_>],
        documents: &'a [&'a [u8]],
        scratch: &mut [u8],
    ) -> Result<Self, ContinuationScopeRefusal> {
        use ContinuationScopeRefusal as E;
        let permission = intent.intent();
        if actual.protocol_version != c::PROTOCOL_VERSION
            || actual.offer_generation.0 == 0
            || actual.host_id.as_str() != permission.host_id
            || actual.boot_id.as_str() != intent.actual_boot()
            || actual.profile.as_str() != permission.configuration_id
            || documents
                .iter()
                .try_fold(10usize, |n, d| n.checked_add(4)?.checked_add(d.len()))
                .is_none_or(|n| n as u64 > u64::from(permission.ceilings.assignment_bytes))
        {
            return Err(E::Bounds);
        }
        if continuation_scope_digest(permission.body_id, full).map_err(|_| E::Scope)?
            != permission.scope_digest
            || continuation_bundle_digest(permission.source_digest, pins).map_err(|_| E::Scope)?
                != bundle.digest()
            || bundle.digest() != permission.workset_digest
            || entries.is_empty()
            || entries.len() != pins.len()
            || documents.len() != pins.len()
            || full.plots.iter().filter(|p| p.local).count() != pins.len()
        {
            return Err(E::Scope);
        }
        let empty = BodyPlotSeal {
            source_document_id: "",
            checked_plot_id: "",
            plan_id: "",
        };
        let mut proposed = [empty; MAX_BODY_PLOTS];
        for (out, pin) in proposed.iter_mut().zip(full.plots) {
            *out = BodyPlotSeal {
                source_document_id: pin.source_document,
                checked_plot_id: pin.checked_plot,
                plan_id: pin.plan_id,
            };
        }
        let mut declared_runtime = 0u32;
        for (i, (pin, entry)) in pins.iter().zip(entries).enumerate() {
            let (m, projection) = decode_body_fragment_document(documents[i])
                .map_err(|e| E::Fragment(crate::DelegatedFragmentRefusal::Document(e)))?;
            let mut slots = full.plots.iter().enumerate().filter(|(_, p)| {
                p.local
                    && p.source_document == pin.source_document
                    && p.checked_plot == pin.checked_plot
            });
            let (slot, _) = slots.next().ok_or(E::Scope)?;
            if slots.next().is_some()
                || pins[..i].iter().any(|p| {
                    p.source_document == pin.source_document && p.checked_plot == pin.checked_plot
                })
                || m.body != permission.body_id
                || m.host != permission.host_id
                || m.boot != intent.actual_boot()
                || m.boot != actual.boot_id.as_str()
                || m.host != actual.host_id.as_str()
                || m.offer_generation != actual.offer_generation.0
                || m.wake != full.wake_id
                || m.workload_revision != full.workload_revision
                || m.play_sequence != 1
                || m.source != pin.source_document
                || m.checked_plot != pin.checked_plot
                || m.expanded_plot != pin.expanded_plot
                || c::fragment_seal_recipe_identity(entry.recipe.iter().copied())
                    != pin.recipe_digest
            {
                return Err(E::Scope);
            }
            bindings::check(pin, entry, &m, actual)?;
            let fragment = c::fragment_seal_recipe_digest(entry.recipe, entry.bindings)
                .map_err(|_| E::Scope)?;
            if !crate::continuation_intent::scope::digest_matches(m.fragment, "", fragment) {
                return Err(E::Scope);
            }
            let plot = c::PlainPlotPlanSeal {
                source_document_id: pin.source_document,
                checked_plot_id: pin.checked_plot,
                expanded_plot_id: pin.expanded_plot,
            }
            .digest(&[c::PlotFragmentSeal {
                host_id: m.host,
                fragment_id: m.fragment,
            }])
            .map_err(|_| E::Scope)?;
            if !crate::continuation_intent::scope::digest_matches(m.plot_plan, "", plot) {
                return Err(E::Scope);
            }
            let template = c::AssignedLocalProjectionTemplate::check(
                entry.template,
                &pin.projection_digest,
                entry.profile.maxima,
            )
            .map_err(|_| E::Scope)?;
            let n = template
                .instantiate(
                    c::FreshAssignedHeader {
                        plan: c::AssignedIdentity::from_text(m.plot_plan),
                        fragment: c::AssignedIdentity::from_text(m.fragment),
                        host: c::AssignedIdentity::from_text(m.host),
                        boot: c::AssignedIdentity::from_text(m.boot),
                    },
                    entry.native,
                    entry.fore,
                    scratch,
                )
                .map_err(|_| E::Scope)?;
            if scratch.get(..n) != Some(projection) {
                return Err(E::Scope);
            }
            let profile = AssignedBodyPreparationProfile {
                maxima: entry.profile.maxima,
                requirements: entry.profile.requirements,
                computations: entry.profile.computations,
                fore: entry.profile.fore,
            };
            let (approved, _) = approve_checked::<PORTS>(
                m,
                projection,
                actual,
                permission.body_id,
                permission.ceilings,
                profile,
            )
            .map_err(E::Fragment)?;
            declared_runtime = declared_runtime
                .checked_add(u32::from(approved.view().runtime_state_bytes))
                .ok_or(E::Bounds)?;
            if declared_runtime > permission.ceilings.runtime_bytes {
                return Err(E::Bounds);
            }
            proposed[slot].plan_id = m.plot_plan;
        }
        let candidate = body_plan_commitment_digest(
            permission.body_id,
            full.wake_id,
            full.workload_revision,
            &proposed[..full.plots.len()],
            &[],
            None,
        )
        .map_err(|_| E::Scope)?;
        for document in documents {
            let (m, _) = decode_body_fragment_document(document).map_err(|_| E::Scope)?;
            if !crate::continuation_intent::scope::digest_matches(
                m.body_plan,
                "body-plan/sha256:",
                candidate,
            ) || !body_play_identity_matches(
                permission.body_id,
                full.wake_id,
                m.body_plan,
                1,
                m.body_play,
            ) {
                return Err(E::Scope);
            }
        }
        let mut hash = Sha256::new();
        hash.update(b"conduit.body/local-continuation-custody@1\0");
        hash.update(permission.signing_transcript());
        hash.update(candidate);
        for document in documents {
            hash.update((document.len() as u64).to_le_bytes());
            hash.update(document);
        }
        let result = Self {
            intent,
            documents,
            custody_identity: hash.finalize().into(),
        };
        result.check_actual(intent, actual)?;
        Ok(result)
    }
}
