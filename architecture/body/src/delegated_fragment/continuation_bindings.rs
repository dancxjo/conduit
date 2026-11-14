//! Link semantic recipe substitutions to the SAME candidate projection facts.
use super::*;
use crate::ContinuationBundleEntry;
use c::{FragmentScopeRole as R, FragmentSealValue as V};
/// Immutable operation identities at one canonical authority entry. This describes
/// the pinned recipe; it neither issues nor proves possession of an original grant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ContinuationRecipeAuthority {
    pub authority: c::AssignedIdentity,
    pub call: c::AssignedIdentity,
    pub subject: c::AssignedIdentity,
    pub capability: c::AssignedIdentity,
}

/// Read the canonical Grant/literal/Host/Boot/literal authority layout without
/// allocating. Entry ordinals belong to the pinned recipe, never to fresh grant
/// sorting or capability requirement ordering.
pub fn continuation_recipe_authority(
    recipe: &[c::FragmentSealPiece<'_>],
    field: c::FragmentSealField,
) -> Result<ContinuationRecipeAuthority, ContinuationScopeRefusal> {
    use c::FragmentSealPiece as P;
    let fail = || ContinuationScopeRefusal::Scope;
    if field.placement.is_none()
        || !matches!(
            field.role,
            R::AuthorityGrant | R::AuthorityHost | R::AuthorityBoot
        )
    {
        return Err(fail());
    }
    let mut ordinal = 0usize;
    let mut anchor = None;
    let mut counts = [0usize; 3];
    for (index, piece) in recipe.iter().enumerate() {
        let P::Field(f, textual) = piece else {
            continue;
        };
        if f.placement != field.placement {
            continue;
        }
        let role = match f.role {
            R::AuthorityGrant => 0,
            R::AuthorityHost => 1,
            R::AuthorityBoot => 2,
            _ => continue,
        };
        if !textual {
            return Err(fail());
        }
        if role == 0 {
            if usize::from(f.entry) != ordinal {
                return Err(fail());
            }
            ordinal += 1;
            if f.entry == field.entry {
                anchor = Some(index);
            }
        }
        if f.entry == field.entry {
            counts[role] += 1;
        }
    }
    if counts != [1, 1, 1] {
        return Err(fail());
    }
    let index = anchor.ok_or_else(fail)?;
    let expected = |role| c::FragmentSealField { role, ..field };
    let Some(
        [P::Field(_, true), P::Literal(triple), P::Field(host, true), P::Field(boot, true), P::Literal(tail)],
    ) = recipe.get(index..index + 5)
    else {
        return Err(fail());
    };
    if *host != expected(R::AuthorityHost) || *boot != expected(R::AuthorityBoot) {
        return Err(fail());
    }
    fn identity(bytes: &mut &[u8]) -> Option<c::AssignedIdentity> {
        let length = usize::try_from(u32::from_le_bytes(bytes.get(..4)?.try_into().ok()?)).ok()?;
        let text = core::str::from_utf8(bytes.get(4..4usize.checked_add(length)?)?).ok()?;
        if text.is_empty() {
            return None;
        }
        *bytes = bytes.get(4 + length..)?;
        Some(c::AssignedIdentity::from_text(text))
    }
    let mut triple = *triple;
    let authority = identity(&mut triple).ok_or_else(fail)?;
    let call = identity(&mut triple).ok_or_else(fail)?;
    let subject = identity(&mut triple).ok_or_else(fail)?;
    if !triple.is_empty() {
        return Err(fail());
    }
    let mut tail = *tail;
    let capability = identity(&mut tail).ok_or_else(fail)?;
    Ok(ContinuationRecipeAuthority {
        authority,
        call,
        subject,
        capability,
    })
}

fn matches_authority(key: ContinuationRecipeAuthority, n: &c::FreshAssignedNative) -> bool {
    key.authority == n.binding.authority
        && key.call == n.binding.call
        && key.subject == n.binding.subject
        && key.capability == n.binding.authority_capability
        && key.capability == n.binding.capability
        && n.contract == n.binding.call
}

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
        // A node has one provider, independently of its number of operations.
        if native.any(|other| {
            n.is_none_or(|first| {
                first.binding.instance != other.binding.instance
                    || first.binding.generation != other.binding.generation
            })
        }) {
            return Err(fail);
        }
        let authority = if matches!(
            b.field.role,
            R::AuthorityGrant | R::AuthorityHost | R::AuthorityBoot
        ) {
            let key = continuation_recipe_authority(entry.recipe, b.field)?;
            let mut rows = entry
                .native
                .iter()
                .filter(|n| n.node == node && matches_authority(key, n));
            let row = rows.next().ok_or(fail.clone())?;
            if rows.next().is_some() {
                return Err(fail);
            }
            Some(row)
        } else {
            None
        };
        let native_id = |select: fn(c::AssignedNativeBinding) -> c::AssignedIdentity| {
            n.is_some_and(|n|matches!(b.value,V::Text(s) if c::AssignedIdentity::from_text(s)==select(n.binding)))
        };
        let valid = match b.field.role {
            R::PlacementId => b.field.entry == 0 && text(placement_id),
            R::Host => b.field.entry == 0 && text(actual.host_id.as_str()),
            R::AuthorityHost => text(actual.host_id.as_str()),
            R::Boot => b.field.entry == 0 && text(actual.boot_id.as_str()),
            R::AuthorityBoot => text(actual.boot_id.as_str()),
            R::OfferGeneration => b.field.entry == 0 && number(actual.offer_generation.0),
            R::ProviderInstance => b.field.entry == 0 && native_id(|n| n.instance),
            R::ProviderGeneration => {
                b.field.entry == 0 && n.is_some_and(|n| number(n.binding.generation))
            }
            R::AuthorityGrant => authority.is_some_and(|n| {
                matches!(b.value, V::Text(s)
                if c::AssignedIdentity::from_text(s) == n.binding.grant)
            }),
            R::ResourcePool => {
                let mut resources = crate::FragmentResources::new(metadata.resource_bindings)
                    .filter_map(Result::ok)
                    .filter(|r| r.node == node && r.resource == b.field.entry);
                resources
                    .next()
                    .is_some_and(|r| r.placement == placement_id && text(r.pool))
                    && resources.next().is_none()
            }
            R::ContentOwnerHost | R::ContentOwnerBoot => {
                let mut resources = crate::FragmentResources::new(metadata.resource_bindings)
                    .filter_map(Result::ok)
                    .filter(|r| r.node == node && r.resource == b.field.entry);
                resources.next().is_some_and(|r| {
                    r.content.is_some()
                        && r.placement == placement_id
                        && text(if b.field.role == R::ContentOwnerHost {
                            actual.host_id.as_str()
                        } else {
                            actual.boot_id.as_str()
                        })
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
        let ordinal = pin
            .placement_nodes
            .iter()
            .position(|node| *node == r.node)
            .ok_or(fail.clone())?;
        if pin.placement_ids[ordinal] != r.placement {
            return Err(fail.clone());
        }
        let mut pools = actual
            .resources
            .iter()
            .filter(|p| p.pool_id.as_str() == r.pool);
        let pool = pools.next().ok_or(fail.clone())?;
        if pools.next().is_some()
            || pool.class_id.as_str() != r.class
            || pool.capacity_units < r.units
            || pool.compute.is_some()
        {
            return Err(fail.clone());
        }
        match (r.content, &pool.content) {
            (None, None) => {}
            (Some(expected), Some(held))
                if expected.contract
                    == crate::FragmentContentContract::from_core(&held.contract)
                    && expected.base == held.base_id.as_str()
                    && expected.residence_profile == held.residence_profile.as_str()
                    && held.owner_host == actual.host_id
                    && held.owner_boot == actual.boot_id => {}
            _ => return Err(fail.clone()),
        }
        c::fragment_seal_resource_matches(
            entry.recipe,
            u16::try_from(ordinal).map_err(|_| fail.clone())?,
            r.resource,
            pool,
            r.units,
        )
        .map_err(|_| fail.clone())?;
    }

    for (index, n) in entry.native.iter().enumerate() {
        let ordinal = pin
            .placement_nodes
            .iter()
            .position(|node| *node == n.node)
            .ok_or(fail.clone())?;
        let placement = u16::try_from(ordinal).map_err(|_| fail.clone())?;
        let mut matches = 0usize;
        for piece in entry.recipe {
            if let c::FragmentSealPiece::Field(field, _) = piece {
                if field.placement == Some(placement)
                    && field.role == R::AuthorityGrant
                    && matches_authority(continuation_recipe_authority(entry.recipe, *field)?, n)
                {
                    matches += 1;
                }
            }
        }
        if matches != 1
            || n.binding.generation == 0
            || entry.native[..index].iter().any(|other| {
                other.node == n.node
                    && (other.call == n.call || other.binding.grant == n.binding.grant)
            })
        {
            return Err(fail.clone());
        }
        if n.binding.host != c::AssignedIdentity::from_text(actual.host_id.as_str())
            || n.binding.boot != c::AssignedIdentity::from_text(actual.boot_id.as_str())
        {
            return Err(fail.clone());
        }
        let mut resource = crate::FragmentResources::new(metadata.resource_bindings)
            .filter_map(Result::ok)
            .filter(|r| r.node == n.node && r.resource == n.binding.resource);
        if !resource.next().is_some_and(|r| {
            c::AssignedIdentity::from_text(r.pool) == n.binding.pool
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
