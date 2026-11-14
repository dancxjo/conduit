//! Conservative requested-byte funding for the existing plain admission bridge.
use super::*;
use core::mem::size_of;

/// Bounds the bridge/Core bookkeeping allocations before any request is built.
/// Sum every requested buffer, including the exact aggregate request table and
/// full new owner backing, rather than assuming disjoint lifetimes. Existing
/// owner inventory, caller observations, allocator overhead and execution/kernel
/// storage are separate charges. No resource reservation or authority is issued.
pub fn delegated_plain_resource_preparation_bytes(
    fragments: &[&super::super::DelegatedBodyFragment<'_>],
    owner: &c::ResourceAdmissionOwner,
    observations: &[c::ResourceObservation],
) -> Result<usize, DelegatedResourceRefusal> {
    preparation_bytes(fragments, owner, observations, false)
}
/// Fund signed content-contract conversion in addition to the complete existing
/// Core reservation bookkeeping. Caller inventory/observations remain separate.
pub fn delegated_resource_preparation_bytes(
    fragments: &[&super::super::DelegatedBodyFragment<'_>],
    owner: &c::ResourceAdmissionOwner,
    observations: &[c::ResourceObservation],
) -> Result<usize, DelegatedResourceRefusal> {
    preparation_bytes(fragments, owner, observations, true)
}
fn preparation_bytes(
    fragments: &[&super::super::DelegatedBodyFragment<'_>],
    owner: &c::ResourceAdmissionOwner,
    observations: &[c::ResourceObservation],
    allow_content: bool,
) -> Result<usize, DelegatedResourceRefusal> {
    use DelegatedResourceRefusal as E;
    if fragments.len() > c::MAXIMUM_RESOURCE_ADMISSIONS
        || !super::super::workset::matches_local_fragment_set(fragments)
    {
        return Err(E::Scope);
    }
    let actual = owner.host();
    let mut bound = Bound(0);
    let mut placements = 0usize;
    for fragment in fragments {
        let m = fragment.metadata();
        if actual.host_id.as_str() != m.host
            || actual.boot_id.as_str() != m.boot
            || actual.offer_generation.0 != m.offer_generation
            || c::AssignedIdentity::from_text(actual.profile.as_str())
                != fragment.receiver_profile()
        {
            return Err(E::Scope);
        }
        fragment.check_plain_resource_projection()?;
        let entries = || FragmentResources::new(m.resource_bindings);
        let mut local = 0usize;
        for (index, entry) in entries().enumerate() {
            let entry = entry.map_err(|_| E::Projection)?;
            if let Some(content) = entry.content {
                if !allow_content {
                    return Err(E::UnsupportedPool);
                }
                bound.add(2, content.contract.content_profile.len())?;
                for text in [m.host, m.boot, content.base, content.residence_profile] {
                    bound.add(1, text.len())?;
                }
            }
            let observed = observations
                .iter()
                .find(|o| o.pool_id.as_str() == entry.pool && o.class_id.as_str() == entry.class)
                .ok_or(E::Admission(
                    c::ResourceAdmissionRefusal::MissingObservation,
                ))?;
            // The item moves into Core; only its IDs and observation Signs copy.
            bound.add(1, size_of::<c::ResourceAdmissionItem>())?;
            bound.add(1, entry.pool.len())?;
            bound.add(2, entry.class.len())?;
            bound.add(1, size_of::<c::SignId>())?;
            bound.add(1, observed.sign_id.as_str().len())?;
            if !entries()
                .take(index)
                .any(|prior| prior.is_ok_and(|p| p.placement == entry.placement))
            {
                local += 1;
                for text in [m.body_plan, entry.placement, m.host, m.boot] {
                    bound.add(1, text.len())?;
                }
            }
        }
        placements = placements
            .checked_add(local)
            .ok_or(E::PreparationCapacity)?;
        if placements > c::MAXIMUM_RESOURCE_ADMISSIONS {
            return Err(E::PreparationCapacity);
        }
    }
    // The bridge reserves one exact aggregate table before constructing rows.
    bound.add(placements, size_of::<c::ResourceAdmissionRequest>())?;
    if placements != 0 {
        bound.add(placements, size_of::<c::ResourceAdmission>())?;
        // An empty owner adopts the fully validated pending table directly.
        // Existing reservations still require a separate expanded owner table.
        if !owner.admissions().is_empty() {
            bound.add(
                owner
                    .admissions()
                    .len()
                    .checked_add(placements)
                    .ok_or(E::PreparationCapacity)?,
                size_of::<c::ResourceAdmission>(),
            )?;
        }
    }
    Ok(bound.0)
}

struct Bound(usize);
impl Bound {
    fn add(&mut self, count: usize, bytes: usize) -> Result<(), DelegatedResourceRefusal> {
        self.0 = count
            .checked_mul(bytes)
            .and_then(|n| self.0.checked_add(n))
            .ok_or(DelegatedResourceRefusal::PreparationCapacity)?;
        Ok(())
    }
}
