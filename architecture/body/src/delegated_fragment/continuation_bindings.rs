//! Link semantic recipe substitutions to the SAME candidate projection facts.
use super::*;
use crate::ContinuationBundleEntry;
use c::{FragmentScopeRole as R, FragmentSealValue as V};
pub(super) fn check(
    pin: &ContinuationBundleEntry<'_>,
    entry: &ContinuationLocalEntry<'_>,
    metadata: &BodyFragmentApprovalMetadata<'_>,
    actual: &c::HostAdvertisement,
) -> Result<(), ContinuationScopeRefusal> {
    let fail = ContinuationScopeRefusal::Scope;
    for b in entry.bindings {
        let text = |expected: &str| matches!(b.value,V::Text(s) if s==expected);
        let number = |expected: u64| matches!(b.value,V::U64(n) if n==expected);
        let Some(placement) = b.field.placement else {
            if b.field.entry != 0
                || !match b.field.role {
                    R::Host => text(actual.host_id.as_str()),
                    R::Boot => text(actual.boot_id.as_str()),
                    R::OfferGeneration => number(actual.offer_generation.0),
                    _ => false,
                }
            {
                return Err(fail);
            }
            continue;
        };
        let p = usize::from(placement);
        let node = *pin.placement_nodes.get(p).ok_or(fail.clone())?;
        let placement_id = *pin.placement_ids.get(p).ok_or(fail.clone())?;
        let mut native = entry.native.iter().filter(|n| n.node == node);
        let n = native.next();
        // Initial primitive profile has one selected native authority per node.
        // Refuse ambiguity instead of guessing canonical grant sorting.
        if native.next().is_some() {
            return Err(fail);
        }
        let native_id = |select: fn(c::AssignedNativeBinding) -> c::AssignedIdentity| {
            n.is_some_and(|n|matches!(b.value,V::Text(s) if c::AssignedIdentity::from_text(s)==select(n.binding)))
        };
        let valid = match b.field.role {
            R::PlacementId => b.field.entry == 0 && text(placement_id),
            R::Host | R::AuthorityHost => text(actual.host_id.as_str()),
            R::Boot | R::AuthorityBoot => text(actual.boot_id.as_str()),
            R::OfferGeneration => b.field.entry == 0 && number(actual.offer_generation.0),
            R::ProviderInstance => b.field.entry == 0 && native_id(|n| n.instance),
            R::ProviderGeneration => {
                b.field.entry == 0 && n.is_some_and(|n| number(n.binding.generation))
            }
            R::AuthorityGrant => b.field.entry == 0 && native_id(|n| n.grant),
            R::ResourcePool => {
                let mut resources = crate::FragmentResources::new(metadata.resource_bindings)
                    .filter_map(Result::ok)
                    .filter(|r| r.node == node && r.resource == b.field.entry);
                resources.next().is_some_and(|r| {
                    r.content.is_none() && r.placement == placement_id && text(r.pool)
                }) && resources.next().is_none()
            }
            // Authored references require their own immutable semantic pin;
            // no unchecked text may masquerade as a fresh selected resource.
            R::PoolReference => false,
        };
        if !valid {
            return Err(fail);
        }
    }
    for resource in crate::FragmentResources::new(metadata.resource_bindings) {
        let r = resource.map_err(|_| fail.clone())?;
        if r.content.is_some()
            || !actual.resources.iter().any(|p| {
                p.pool_id.as_str() == r.pool
                    && p.class_id.as_str() == r.class
                    && p.capacity_units >= r.units
                    && p.compute.is_none()
                    && p.content.is_none()
            })
        {
            return Err(fail.clone());
        }
    }
    for n in entry.native {
        if n.binding.host != c::AssignedIdentity::from_text(actual.host_id.as_str())
            || n.binding.boot != c::AssignedIdentity::from_text(actual.boot_id.as_str())
        {
            return Err(fail.clone());
        }
        let mut resource = crate::FragmentResources::new(metadata.resource_bindings)
            .filter_map(Result::ok)
            .filter(|r| r.node == n.node && r.resource == n.binding.resource);
        if !resource.next().is_some_and(|r| {
            r.content.is_none()
                && c::AssignedIdentity::from_text(r.pool) == n.binding.pool
                && c::AssignedIdentity::from_text(r.class) == n.binding.resource_class
                && r.units == n.binding.units
        }) || resource.next().is_some()
        {
            return Err(fail.clone());
        }
    }
    for fore in entry.fore {
        let ordinal = pin
            .placement_nodes
            .iter()
            .position(|n| *n == fore.local_node)
            .ok_or(fail.clone())?;
        if pin.placement_ids[ordinal] != fore.placement {
            return Err(fail.clone());
        }
    }
    Ok(())
}
