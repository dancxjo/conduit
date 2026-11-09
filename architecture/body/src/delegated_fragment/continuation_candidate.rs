//! Bounded preparation of fresh canonical local documents; never Start authority.
use super::*;
use crate::{
    BodyPlotSeal, CheckedContinuationBundle, ContinuationBundleEntry, ContinuationFullScope,
    FragmentResource, PlainFragmentResource,
};
use alloc::vec::Vec;
use sha2::{Digest, Sha256};

/// Owned preparation output. Root retains these immutable bytes and their
/// references while obtaining/using the ordinary checked FreshContinuationScope.
/// No online owner command, credential, or native grant is produced here.
pub struct ContinuationCandidateDocuments {
    documents: Vec<Vec<u8>>,
}
impl ContinuationCandidateDocuments {
    pub fn document(&self, index: usize) -> Option<&[u8]> {
        self.documents.get(index).map(Vec::as_slice)
    }
    pub fn len(&self) -> usize {
        self.documents.len()
    }
    pub fn is_empty(&self) -> bool {
        self.documents.is_empty()
    }
    /// Vector backing capacities only; Root separately funds inline values,
    /// allocator overhead, installed inputs and the actual admission workspace.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        self.documents.iter().try_fold(
            self.documents
                .capacity()
                .checked_mul(core::mem::size_of::<Vec<u8>>())?,
            |n, d| n.checked_add(d.capacity()),
        )
    }
}
struct Ids {
    fragment: [u8; 64],
    plot: [u8; 64],
}
fn hex(digest: [u8; 32]) -> [u8; 64] {
    let mut out = [0; 64];
    for (i, b) in digest.iter().enumerate() {
        out[i * 2] = b"0123456789abcdef"[usize::from(b >> 4)];
        out[i * 2 + 1] = b"0123456789abcdef"[usize::from(b & 15)];
    }
    out
}
fn text(bytes: &[u8]) -> &str {
    core::str::from_utf8(bytes).expect("canonical ASCII hex")
}
fn lifecycle(prefix: &[u8; 17], digest: [u8; 32]) -> [u8; 81] {
    let mut out = [0; 81];
    out[..17].copy_from_slice(prefix);
    out[17..].copy_from_slice(&hex(digest));
    out
}
fn reserve<T>(
    items: &mut Vec<T>,
    count: usize,
    held: usize,
    budget: usize,
) -> Result<usize, ContinuationScopeRefusal> {
    let required = count
        .checked_mul(core::mem::size_of::<T>())
        .and_then(|n| n.checked_add(held))
        .ok_or(ContinuationScopeRefusal::Bounds)?;
    if required > budget {
        return Err(ContinuationScopeRefusal::Bounds);
    }
    items
        .try_reserve_exact(count)
        .map_err(|_| ContinuationScopeRefusal::Bounds)?;
    let total = items
        .capacity()
        .checked_mul(core::mem::size_of::<T>())
        .and_then(|n| n.checked_add(held))
        .ok_or(ContinuationScopeRefusal::Bounds)?;
    if total > budget {
        return Err(ContinuationScopeRefusal::Bounds);
    }
    Ok(total)
}
/// Assemble exact existing canonical identities/encodings from installed recipes
/// and explicitly typed CURRENT bindings. Root must derive those bindings from
/// its original equipment and Issuers, never a received DTO; this function grants
/// no possession. Later ordinary native issuance/admission checks those owners.
///
/// `maximum_preparation_bytes` limits simultaneous vector backing capacities,
/// including temporary projections/metadata and final document storage; it is
/// capped by the signed runtime ceiling. This is not target RAM qualification.
/// Caller-owned immutable inputs, inline stack and allocator overhead are separate.
#[allow(clippy::too_many_arguments)]
pub fn build_continuation_candidate<const PORTS: usize>(
    intent: &VerifiedContinuationIntent<'_>,
    bundle: &CheckedContinuationBundle,
    full: &ContinuationFullScope<'_>,
    actual: &c::HostAdvertisement,
    pins: &[ContinuationBundleEntry<'_>],
    entries: &[ContinuationLocalEntry<'_>],
    resources: &[&[PlainFragmentResource<'_>]],
    maximum_preparation_bytes: usize,
) -> Result<ContinuationCandidateDocuments, ContinuationScopeRefusal> {
    build_candidate::<PORTS, _>(
        intent,
        bundle,
        full,
        actual,
        pins,
        entries,
        resources,
        maximum_preparation_bytes,
    )
}

/// Canonical content-capable rows; no resource authority is created.
#[allow(clippy::too_many_arguments)]
pub fn build_continuation_candidate_with_resources<const PORTS: usize>(
    intent: &VerifiedContinuationIntent<'_>,
    bundle: &CheckedContinuationBundle,
    full: &ContinuationFullScope<'_>,
    actual: &c::HostAdvertisement,
    pins: &[ContinuationBundleEntry<'_>],
    entries: &[ContinuationLocalEntry<'_>],
    resources: &[&[FragmentResource<'_>]],
    maximum_preparation_bytes: usize,
) -> Result<ContinuationCandidateDocuments, ContinuationScopeRefusal> {
    build_candidate::<PORTS, _>(
        intent,
        bundle,
        full,
        actual,
        pins,
        entries,
        resources,
        maximum_preparation_bytes,
    )
}

trait CandidateResource {
    const PREFIX_BYTES: usize;
    fn encoded_length(&self) -> Result<usize, BodyFragmentDocumentRefusal>;
    fn append_to(&self, output: &mut Vec<u8>) -> Result<(), BodyFragmentDocumentRefusal>;
}
impl CandidateResource for PlainFragmentResource<'_> {
    const PREFIX_BYTES: usize = 0;
    fn encoded_length(&self) -> Result<usize, BodyFragmentDocumentRefusal> {
        self.encoded_length()
    }
    fn append_to(&self, output: &mut Vec<u8>) -> Result<(), BodyFragmentDocumentRefusal> {
        self.append_to(output)
    }
}
impl CandidateResource for FragmentResource<'_> {
    const PREFIX_BYTES: usize = 7;
    fn encoded_length(&self) -> Result<usize, BodyFragmentDocumentRefusal> {
        self.encoded_length()
    }
    fn append_to(&self, output: &mut Vec<u8>) -> Result<(), BodyFragmentDocumentRefusal> {
        self.append_to(output)
    }
}
#[allow(clippy::too_many_arguments)]
fn build_candidate<const PORTS: usize, R: CandidateResource>(
    intent: &VerifiedContinuationIntent<'_>,
    bundle: &CheckedContinuationBundle,
    full: &ContinuationFullScope<'_>,
    actual: &c::HostAdvertisement,
    pins: &[ContinuationBundleEntry<'_>],
    entries: &[ContinuationLocalEntry<'_>],
    resources: &[&[R]],
    maximum_preparation_bytes: usize,
) -> Result<ContinuationCandidateDocuments, ContinuationScopeRefusal> {
    use ContinuationScopeRefusal as E;
    let permission = intent.intent();
    if pins.is_empty()
        || pins.len() > crate::MAX_BODY_PLOTS
        || pins.len() != entries.len()
        || resources.len() != pins.len()
        || full.plots.len() > crate::MAX_BODY_PLOTS
        || crate::continuation_scope_digest(permission.body_id, full).map_err(|_| E::Scope)?
            != permission.scope_digest
        || crate::continuation_bundle_digest(permission.source_digest, pins)
            .map_err(|_| E::Scope)?
            != bundle.digest()
        || bundle.digest() != permission.workset_digest
    {
        return Err(E::Scope);
    }
    let budget = maximum_preparation_bytes
        .min(usize::try_from(permission.ceilings.runtime_bytes).map_err(|_| E::Bounds)?);
    let mut output = ContinuationCandidateDocuments {
        documents: Vec::new(),
    };
    let mut held = reserve(&mut output.documents, pins.len(), 0, budget)?;
    let mut scratch = Vec::new();
    held = reserve(
        &mut scratch,
        entries
            .iter()
            .map(|e| e.template.len())
            .max()
            .ok_or(E::Bounds)?,
        held,
        budget,
    )?;
    scratch.resize(scratch.capacity(), 0);
    {
        let mut ids = Vec::new();
        held = reserve(&mut ids, pins.len(), held, budget)?;
        for (pin, entry) in pins.iter().zip(entries) {
            let fragment = hex(c::fragment_seal_recipe_digest(entry.recipe, entry.bindings)
                .map_err(|_| E::Scope)?);
            let plot = hex(c::PlainPlotPlanSeal {
                source_document_id: pin.source_document,
                checked_plot_id: pin.checked_plot,
                expanded_plot_id: pin.expanded_plot,
            }
            .digest(&[c::PlotFragmentSeal {
                host_id: actual.host_id.as_str(),
                fragment_id: text(&fragment),
            }])
            .map_err(|_| E::Scope)?);
            ids.push(Ids { fragment, plot });
        }
        let mut plots = Vec::new();
        held = reserve(&mut plots, full.plots.len(), held, budget)?;
        for p in full.plots {
            let mut matching = pins.iter().enumerate().filter(|(_, pin)| {
                p.local
                    && pin.source_document == p.source_document
                    && pin.checked_plot == p.checked_plot
            });
            let local = matching.next();
            if matching.next().is_some() || p.local != local.is_some() {
                return Err(E::Scope);
            }
            plots.push(BodyPlotSeal {
                source_document_id: p.source_document,
                checked_plot_id: p.checked_plot,
                plan_id: local.map_or(p.plan_id, |(i, _)| text(&ids[i].plot)),
            });
        }
        let body_plan = lifecycle(
            b"body-plan/sha256:",
            crate::body_plan_commitment_digest(
                permission.body_id,
                full.wake_id,
                full.workload_revision,
                &plots,
                &[],
                None,
            )
            .map_err(|_| E::Scope)?,
        );
        let body_play = lifecycle(
            b"body-play/sha256:",
            crate::body_play_digest(permission.body_id, full.wake_id, text(&body_plan), 1),
        );
        let mut fragments = Vec::new();
        held = reserve(&mut fragments, pins.len(), held, budget)?;
        fragments.extend(ids.iter().map(|id| text(&id.fragment)));
        fragments.sort_unstable();
        let (local_fragment_count, local_fragment_digest) =
            crate::local_fragment_set_commitment(&fragments).map_err(|_| E::Scope)?;
        let mut assignment_bytes = 10usize;
        for (i, ((pin, entry), rows)) in pins.iter().zip(entries).zip(resources).enumerate() {
            let template = c::AssignedLocalProjectionTemplate::check(
                entry.template,
                &pin.projection_digest,
                entry.profile.maxima,
            )
            .map_err(|_| E::Scope)?;
            let length = template
                .instantiate(
                    c::FreshAssignedHeader {
                        plan: c::AssignedIdentity::from_text(text(&ids[i].plot)),
                        fragment: c::AssignedIdentity::from_text(text(&ids[i].fragment)),
                        host: c::AssignedIdentity::from_text(actual.host_id.as_str()),
                        boot: c::AssignedIdentity::from_text(actual.boot_id.as_str()),
                    },
                    entry.native,
                    entry.fore,
                    &mut scratch,
                )
                .map_err(|_| E::Scope)?;
            let projection = &scratch[..length];
            if rows.len() > c::MAXIMUM_BINDINGS_PER_ADMISSION {
                return Err(E::Bounds);
            }
            let resource_bytes = rows
                .iter()
                .try_fold(if rows.is_empty() { 0 } else { R::PREFIX_BYTES }, |n, r| {
                    n.checked_add(r.encoded_length().ok()?)
                })
                .ok_or(E::Bounds)?;
            let mut encoded_resources = Vec::new();
            let with_resources = reserve(&mut encoded_resources, resource_bytes, held, budget)?;
            for row in *rows {
                row.append_to(&mut encoded_resources)
                    .map_err(|_| E::Scope)?;
            }
            let mut encoded_fore = Vec::new();
            let fore_bytes = c::AssignedPlanRecords::new(projection)
                .map_err(|_| E::Scope)?
                .try_fold(0usize, |n, r| {
                    let r = r.map_err(|_| E::Scope)?;
                    n.checked_add(if r.tag == c::ASSIGNED_LOCAL_FORE {
                        r.payload.len()
                    } else {
                        0
                    })
                    .ok_or(E::Bounds)
                })?;
            let temporary = reserve(&mut encoded_fore, fore_bytes, with_resources, budget)?;
            for record in c::AssignedPlanRecords::new(projection).map_err(|_| E::Scope)? {
                let record = record.map_err(|_| E::Scope)?;
                if record.tag == c::ASSIGNED_LOCAL_FORE {
                    encoded_fore.extend_from_slice(record.payload);
                }
            }
            let metadata = BodyFragmentApprovalMetadata {
                schema: 3,
                body: permission.body_id,
                wake: full.wake_id,
                body_plan: text(&body_plan),
                body_play: text(&body_play),
                play_sequence: 1,
                workload_revision: full.workload_revision,
                source: pin.source_document,
                checked_plot: pin.checked_plot,
                expanded_plot: pin.expanded_plot,
                plot_plan: text(&ids[i].plot),
                fragment: text(&ids[i].fragment),
                host: actual.host_id.as_str(),
                boot: actual.boot_id.as_str(),
                offer_generation: actual.offer_generation.0,
                local_fragment_count,
                local_fragment_digest,
                resource_bindings: &encoded_resources,
                fore_ports: &encoded_fore,
                projection_length: u32::try_from(length).map_err(|_| E::Bounds)?,
                projection_digest: Sha256::digest(projection).into(),
            };
            let available = budget.checked_sub(temporary).ok_or(E::Bounds)?;
            let remaining = usize::try_from(permission.ceilings.assignment_bytes)
                .map_err(|_| E::Bounds)?
                .checked_sub(assignment_bytes.checked_add(4).ok_or(E::Bounds)?)
                .ok_or(E::Bounds)?;
            let maximum = u32::try_from(available.min(remaining)).map_err(|_| E::Bounds)?;
            let document = encode_body_fragment_document(&metadata, projection, maximum)
                .map_err(|_| E::Bounds)?;
            if temporary
                .checked_add(document.capacity())
                .is_none_or(|n| n > budget)
            {
                return Err(E::Bounds);
            }
            assignment_bytes = assignment_bytes
                .checked_add(4)
                .and_then(|n| n.checked_add(document.len()))
                .ok_or(E::Bounds)?;
            held = held.checked_add(document.capacity()).ok_or(E::Bounds)?;
            output.documents.push(document);
        }
    }
    let mut documents = Vec::new();
    let retained = output
        .retained_backing_bytes()
        .and_then(|n| n.checked_add(scratch.capacity()))
        .ok_or(E::Bounds)?;
    reserve(&mut documents, output.documents.len(), retained, budget)?;
    documents.extend(output.documents.iter().map(Vec::as_slice));
    // Same complete SAFE verifier as externally authored fresh candidates. This
    // temporary approval cannot escape: Root later borrows its retained bytes.
    FreshContinuationScope::check_local_replacement::<PORTS>(
        intent,
        bundle,
        full,
        actual,
        pins,
        entries,
        &documents,
        &mut scratch,
    )?;
    Ok(output)
}
