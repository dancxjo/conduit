//! Bridge checked compact bindings into the one current Core reservation owner.
use super::*;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum DelegatedResourceRefusal {
    Scope,
    Allocation,
    Projection,
    UnsupportedPool,
    Admission(c::ResourceAdmissionRefusal),
}

impl super::super::DelegatedBodyFragment<'_> {
    /// Prepare only the exact signed plain bindings, then atomically ask Core's
    /// existing owner. No offer catalog, new ledger, native capability or start.
    /// Root still admits complete execution workspace and owns Play retirement.
    pub fn admit_plain_resources<'o>(
        &self,
        owner: &'o mut c::ResourceAdmissionOwner,
        observations: &[c::ResourceObservation],
    ) -> Result<&'o [c::ResourceAdmission], DelegatedResourceRefusal> {
        admit_delegated_plain_resources(&[self], owner, observations)
    }

    fn plain_resource_requests(
        &self,
        actual: &c::HostAdvertisement,
    ) -> Result<Vec<c::ResourceAdmissionRequest>, DelegatedResourceRefusal> {
        use DelegatedResourceRefusal as E;
        let metadata = self.metadata();
        if c::AssignedIdentity::from_text(actual.profile.as_str()) != self.receiver_profile()
            || actual.host_id.as_str() != metadata.host
            || actual.boot_id.as_str() != metadata.boot
            || actual.offer_generation.0 != metadata.offer_generation
        {
            return Err(E::Scope);
        }
        let entries = || PlainFragmentResources::new(metadata.resource_bindings);
        // All signed descriptors and numeric correspondence are checked before
        // allocating requests or changing the existing reservation ledger.
        self.check_plain_resource_projection()?;
        for item in entries() {
            let item = item.map_err(|_| E::Projection)?;
            let pool = actual
                .resources
                .iter()
                .find(|pool| {
                    pool.pool_id.as_str() == item.pool && pool.class_id.as_str() == item.class
                })
                .ok_or(E::UnsupportedPool)?;
            if pool.compute.is_some() || pool.content.is_some() {
                return Err(E::UnsupportedPool);
            }
        }
        let placements = entries()
            .enumerate()
            .filter(|(index, entry)| {
                let node = entry
                    .as_ref()
                    .expect("preflight checked immutable entries")
                    .node;
                !entries()
                    .take(*index)
                    .any(|prior| prior.is_ok_and(|p| p.node == node))
            })
            .count();
        let mut requests: Vec<c::ResourceAdmissionRequest> = Vec::new();
        requests
            .try_reserve_exact(placements)
            .map_err(|_| E::Allocation)?;
        for entry in entries() {
            let entry = entry.map_err(|_| E::Projection)?;
            let index = if let Some(index) = requests
                .iter()
                .position(|r| r.placement_id.as_str() == entry.placement)
            {
                index
            } else {
                let mut items = Vec::new();
                items
                    .try_reserve_exact(
                        entries()
                            .filter(|prior| prior.as_ref().is_ok_and(|p| p.node == entry.node))
                            .count(),
                    )
                    .map_err(|_| E::Allocation)?;
                requests.push(c::ResourceAdmissionRequest {
                    plan_id: copy_id(metadata.body_plan)?.into(),
                    placement_id: copy_id(entry.placement)?.into(),
                    host_id: copy_id(actual.host_id.as_str())?.into(),
                    boot_id: copy_id(actual.boot_id.as_str())?.into(),
                    offer_generation: actual.offer_generation,
                    items,
                });
                requests.len() - 1
            };
            requests[index].items.push(c::ResourceAdmissionItem {
                requirement: c::ResourceRequirement {
                    class_id: copy_id(entry.class)?.into(),
                    units: entry.units,
                    protected_role: None,
                    compute: None,
                    content: None,
                },
                binding: c::ResourceBinding {
                    pool_id: copy_id(entry.pool)?.into(),
                    class_id: copy_id(entry.class)?.into(),
                    units: entry.units,
                    protected: None,
                    compute: None,
                    content: None,
                },
            });
        }
        Ok(requests)
    }

    pub(crate) fn check_plain_resource_projection(&self) -> Result<(), DelegatedResourceRefusal> {
        use DelegatedResourceRefusal::Projection as E;
        let entries = || PlainFragmentResources::new(self.metadata().resource_bindings);
        let mut count = 0usize;
        for (index, entry) in entries().enumerate() {
            if index >= c::MAXIMUM_BINDINGS_PER_ADMISSION {
                return Err(E);
            }
            let entry = entry.map_err(|_| E)?;
            if entry.node >= u16::from(self.view().counts[0]) {
                return Err(E);
            }
            for prior in entries().take(index) {
                let prior = prior.map_err(|_| E)?;
                if (prior.node == entry.node
                    && (prior.placement != entry.placement || prior.resource == entry.resource))
                    || (prior.node != entry.node && prior.placement == entry.placement)
                {
                    return Err(E);
                }
            }
            let mut found = 0usize;
            for record in c::AssignedPlanRecords::new(self.projection()).map_err(|_| E)? {
                let record = record.map_err(|_| E)?;
                if record.tag != c::ASSIGNED_RESOURCE {
                    continue;
                }
                let d = record.payload;
                if d.len() != 8 {
                    return Err(E);
                }
                if d[..2] == entry.node.to_le_bytes() && d[2..4] == entry.resource.to_le_bytes() {
                    if d[4..] != entry.units.to_le_bytes() {
                        return Err(E);
                    }
                    found += 1;
                }
            }
            if found != 1 {
                return Err(E);
            }
            count += 1;
        }
        let actual = c::AssignedPlanRecords::new(self.projection())
            .map_err(|_| E)?
            .try_fold(0usize, |count, r| {
                r.map(|r| count + usize::from(r.tag == c::ASSIGNED_RESOURCE))
            })
            .map_err(|_| E)?;
        if count != actual {
            return Err(E);
        }
        Ok(())
    }
}

/// Reserve the complete selected local fragment set with Core's atomic batch law.
/// All fragments must belong to one exact Body Plan/Play. A late refusal leaves
/// every old reservation unchanged. No kernel or provider is started here.
pub fn admit_delegated_plain_resources<'o>(
    fragments: &[&super::super::DelegatedBodyFragment<'_>],
    owner: &'o mut c::ResourceAdmissionOwner,
    observations: &[c::ResourceObservation],
) -> Result<&'o [c::ResourceAdmission], DelegatedResourceRefusal> {
    let requests = resource_requests(fragments, owner.host())?;
    if requests.is_empty() {
        let end = owner.admissions().len();
        return Ok(&owner.admissions()[end..]);
    }
    owner
        .admit_batch(requests, observations)
        .map_err(DelegatedResourceRefusal::Admission)
}

/// Hold the same exact complete-set reservations for an unstarted attempt.
/// Dropping the scope releases only these reservations. No Back/start is granted.
pub fn reserve_delegated_plain_resources<'o>(
    fragments: &[&super::super::DelegatedBodyFragment<'_>],
    owner: &'o mut c::ResourceAdmissionOwner,
    observations: &[c::ResourceObservation],
) -> Result<c::ResourceAdmissionScope<'o>, DelegatedResourceRefusal> {
    let requests = resource_requests(fragments, owner.host())?;
    let mut scope = owner.preparation_scope();
    if !requests.is_empty() {
        scope
            .admit_batch(requests, observations)
            .map_err(DelegatedResourceRefusal::Admission)?;
    }
    Ok(scope)
}

fn resource_requests(
    fragments: &[&super::super::DelegatedBodyFragment<'_>],
    actual: &c::HostAdvertisement,
) -> Result<Vec<c::ResourceAdmissionRequest>, DelegatedResourceRefusal> {
    use DelegatedResourceRefusal as E;
    if fragments.len() > c::MAXIMUM_RESOURCE_ADMISSIONS {
        return Err(E::Scope);
    }
    let first = fragments.first().ok_or(E::Scope)?.metadata();
    if !super::super::workset::matches_local_fragment_set(fragments) {
        return Err(E::Scope);
    }
    let mut requests = Vec::new();
    for fragment in fragments {
        let m = fragment.metadata();
        if (
            m.body,
            m.wake,
            m.body_plan,
            m.body_play,
            m.workload_revision,
        ) != (
            first.body,
            first.wake,
            first.body_plan,
            first.body_play,
            first.workload_revision,
        ) {
            return Err(E::Scope);
        }
        let mut prepared = fragment.plain_resource_requests(actual)?;
        if requests.len() + prepared.len() > c::MAXIMUM_RESOURCE_ADMISSIONS {
            return Err(E::Projection);
        }
        requests
            .try_reserve_exact(prepared.len())
            .map_err(|_| E::Allocation)?;
        requests.append(&mut prepared);
    }
    Ok(requests)
}

// Build original signed IDs fallibly; converting the owned String into its typed
// identity moves the allocation. No richer catalog or contract is reconstructed.
fn copy_id(text: &str) -> Result<alloc::string::String, DelegatedResourceRefusal> {
    let mut owned = alloc::string::String::new();
    owned
        .try_reserve_exact(text.len())
        .map_err(|_| DelegatedResourceRefusal::Allocation)?;
    owned.push_str(text);
    Ok(owned)
}
