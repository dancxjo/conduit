//! Allocation-free eligibility and complete native template construction envelope.
use super::*;
pub(super) fn preflight<'a>(
    p: &PlannedMaskPlot,
    target: &'a HostAdvertisement,
    owner: &HostAdvertisement,
    lines: [&'a LineOffer; 3],
) -> Option<Checked<'a>> {
    let source_bytes = conduit_std_host::native_mask_plan_backing_bytes(p)?;
    let [f] = p.plan.fragments.as_slice() else {
        return None;
    };
    if f.placements.len() != 4
        || f.connections.len() != 4
        || f.fore_ports.len() != 3
        || f.startup_order.len() != 4
        || f.plan_fragments.len() != 1
        || target.protocol_version != PROTOCOL_VERSION
        || target.host_id.as_str().is_empty()
        || target.boot_id.as_str().is_empty()
        || target.offer_generation.0 == 0
        || target.host_id == owner.host_id
        || p.mask.plot_name != "native-graphical"
    {
        return None;
    }
    let kinds = [
        conduit_face::SHOW_RESOURCE_SOURCE_KIND,
        conduit_face::FACE_TEE_KIND,
        conduit_face::RESOURCE_RENDERER_KIND,
        conduit_face::FACE_INTERACTION_KIND,
    ];
    for kind in kinds {
        if f.placements
            .iter()
            .filter(|g| g.kind_id.as_str() == kind)
            .count()
            != 1
        {
            return None;
        }
    }
    let known = |id: &PlacementId| f.placements.iter().any(|g| &g.placement_id == id);
    if f.connections.iter().any(|c| !known(&c.source_placement_id) || !known(&c.sink_placement_id)
        || c.resource.as_ref().is_some_and(|r| !known(&r.owner_placement_id)))
        || f.fore_ports.iter().any(|v| !known(&v.placement_id))
        || f.startup_dependencies.iter().any(|d| !known(&d.prerequisite_placement_id) || !known(&d.dependent_placement_id))
        || f.startup_order.iter().any(|v| !known(v))
        || f.expected_terminals.iter().any(|v| matches!(v,ExpectedTerminal::PlacementCompleted(id) if !known(id)))
        || f.expected_sign.iter().any(|v| matches!(v,ExpectedSign::PlacementPrepared(id)|ExpectedSign::PlacementTerminal(id) if !known(id))) { return None; }
    let mut bases = [None; 4];
    let mut replacement = 0usize;
    for (i, g) in f.placements.iter().enumerate() {
        let next = unique(
            target
                .capabilities
                .iter()
                .filter(|c| c.capability_id == g.capability_id),
        )?;
        if !cap_matches(g, next)
            || !g.realization_characteristics.is_empty()
            || !g.realization_properties.is_empty()
            || !g.pool_references.is_empty()
            || !g.terminal_transductions.is_empty()
        {
            return None;
        }
        let to_base = optional_unique(
            target
                .bases
                .iter()
                .filter(|b| b.capability_ids.contains(&g.capability_id)),
        )?;
        match (g.base.as_ref(), to_base) {
            (None, None) => {}
            (Some(_), Some(b))
                if b.lifecycle == BaseLifecycle::Ready && b.provider_generation > 0 =>
            {
                for value in [
                    &b.base_id.0,
                    &b.provider_instance_id.0,
                    &b.implementation_id.0,
                    &b.mechanism_family.0,
                ] {
                    replacement = replacement.checked_add(value.len())?;
                }
                if b.base_id.as_str().is_empty()
                    || b.provider_instance_id.as_str().is_empty()
                    || b.resource_pool_ids.iter().enumerate().any(|(i, id)| {
                        b.resource_pool_ids[..i].contains(id)
                            || target.resources.iter().filter(|r| &r.pool_id == id).count() != 1
                    })
                {
                    return None;
                }
                bases[i] = Some(b);
            }
            _ => return None,
        }
        for r in &g.resources {
            if r.protected.is_some() || r.compute.is_some() || r.content.is_some() {
                return None;
            }
            let pool = unique(target.resources.iter().filter(|v| v.class_id == r.class_id))?;
            let demand = f
                .placements
                .iter()
                .flat_map(|v| &v.resources)
                .filter(|v| v.class_id == r.class_id)
                .try_fold(0u32, |n, v| n.checked_add(v.units))?;
            if pool.capacity_units < demand
                || pool.compute.is_some()
                || pool.content.is_some()
                || bases[i].is_some_and(|b| {
                    !b.resource_pool_ids.contains(&pool.pool_id)
                        || target
                            .bases
                            .iter()
                            .filter(|owner| owner.resource_pool_ids.contains(&pool.pool_id))
                            .count()
                            != 1
                })
            {
                return None;
            }
            // A resource connection can own another copy of this exact pool ID.
            replacement = replacement
                .checked_add(pool.pool_id.0.len().checked_mul(1 + f.connections.len())?)?;
        }
        if g.capability_id.as_str() == "renderer-conduitos" {
            let [a] = g.authority.as_slice() else {
                return None;
            };
            if a.grant_id.as_str() != "conduitos/native-mask/present"
                || a.contract_id.as_str() != PRESENT_AUTHORITY_CONTRACT
                || a.host_call_contract_id.as_str() != PRESENT_HOST_CALL_CONTRACT
                || a.subject_kind.as_str() != "face/base/conduitos-surface@1"
                || a.host_id != f.host_id
                || a.boot_id != f.boot_id
                || a.capability_id != g.capability_id
            {
                return None;
            }
            // The exact standard policy is reconstructed before replacing its
            // old binding, so count every new policy string during overlap.
            for value in [
                &a.grant_id.0,
                &a.contract_id.0,
                &a.host_call_contract_id.0,
                &a.subject_kind.0,
                &a.capability_id.0,
            ] {
                replacement = replacement.checked_add(value.len())?;
            }
            replacement = replacement
                .checked_add(target.host_id.0.len())?
                .checked_add(target.boot_id.0.len())?;
        } else if !g.authority.is_empty() {
            return None;
        }
        replacement = replacement
            .checked_add(target.host_id.0.len())?
            .checked_add(target.boot_id.0.len())?;
    }
    for (i, line) in lines.iter().enumerate() {
        if lines[..i]
            .iter()
            .any(|v| v.line_id == line.line_id || v.binding.binding_id == line.binding.binding_id)
        {
            return None;
        }
        let fore = unique(
            f.fore_ports
                .iter()
                .filter(|fore| line_index(&p.mask, fore) == Some(i)),
        )?;
        validate_line(line, fore, target, owner)?;
        replacement = replacement.checked_add(line_backing(line)?)?;
    }
    // Every changed owner is additive to the original visitor (old backing is
    // deliberately not subtracted). Placement identities are fixed 64-byte hex.
    let references = 4usize
        .checked_add(f.connections.len().checked_mul(3)?)?
        .checked_add(f.fore_ports.len())?
        .checked_add(f.startup_dependencies.len().checked_mul(2)?)?
        .checked_add(f.startup_order.len())?
        .checked_add(f.expected_terminals.len())?
        .checked_add(f.expected_sign.len())?;
    replacement = replacement
        .checked_add(references.checked_mul(64)?)?
        .checked_add(target.host_id.0.len().checked_mul(2)?)?
        .checked_add(target.boot_id.0.len())?
        .checked_add(64usize.checked_mul(4)?)?;
    let retained = size_of::<PlannedMaskPlot>()
        .checked_add(source_bytes)?
        .checked_add(replacement)?;
    // Working clone, final ordinary-admitted clone, and conservative ordinary
    // verifier scratch coexist. Source is an independently retained immutable
    // typed artifact; no RemoteSeal/deflate/JSON codec is called in this path.
    let working_plan = retained;
    let ordinarily_admitted_result = retained;
    let remapping_ids = 4usize.checked_mul(size_of::<PlacementId>().checked_add(64)?)?;
    let digest_temporaries = size_of::<PlanId>().checked_add(64)?.checked_add(64)?;
    let verifier = super::super::backing::native_verification_workspace(p)?;
    let peak = working_plan
        .checked_add(ordinarily_admitted_result)?
        .checked_add(remapping_ids)?
        .checked_add(digest_temporaries)?
        .checked_add(verifier)?
        .checked_add(size_of::<Checked<'_>>())?;
    Some(Checked {
        bases,
        lines,
        retained,
        peak,
    })
}

fn unique<T>(mut values: impl Iterator<Item = T>) -> Option<T> {
    let first = values.next()?;
    values.next().is_none().then_some(first)
}
fn optional_unique<T>(mut values: impl Iterator<Item = T>) -> Option<Option<T>> {
    let first = values.next();
    values.next().is_none().then_some(first)
}
pub(super) fn line_index(mask: &conduit_face::MaskPlot, fore: &PlannedForePort) -> Option<usize> {
    if fore.track != ConnectionTrack::Payload {
        return None;
    }
    [
        (PortDirection::Input, &mask.face_input),
        (PortDirection::Output, &mask.show_output),
        (PortDirection::Output, &mask.interaction_output),
    ]
    .iter()
    .position(|(direction, b)| {
        *direction == fore.direction
            && b.front_port_id == fore.front_port_id
            && b.gear_port_id == fore.gear_port_id
    })
}
fn validate_line(
    line: &LineOffer,
    fore: &PlannedForePort,
    target: &HostAdvertisement,
    owner: &HostAdvertisement,
) -> Option<()> {
    let b = &line.binding;
    let (local, peer) = if fore.direction == PortDirection::Input {
        (&b.sink, &b.source)
    } else {
        (&b.source, &b.sink)
    };
    if !line.validate_sign_identity()
        || line.contract.scope != LineScope::RoutedNetwork
        || line.contract.traffic_shape != LineTrafficShape::Message
        || line.contract.ordering != LineOrdering::Ordered
        || line.contract.reliability != LineReliability::Reliable
        || line.contract.security != LineSecurity::AuthenticatedEncrypted
        || line.line_id.as_str().is_empty()
        || b.binding_id.as_str().is_empty()
        || b.base_instance_id.as_str().is_empty()
        || b.base.as_str().is_empty()
        || b.base.as_str() == "conduit.base/local@1"
        || local.host_id != target.host_id
        || local.boot_id != target.boot_id
        || peer.host_id != owner.host_id
        || peer.boot_id != owner.boot_id
        || b.source.endpoint_id.as_str().is_empty()
        || b.sink.endpoint_id.as_str().is_empty()
        || b.source.endpoint_id == b.sink.endpoint_id
        || line.line_id.as_str() == b.binding_id.as_str()
        || line.line_id.as_str() == b.base_instance_id.as_str()
        || line.line_id.as_str() == b.source.endpoint_id.as_str()
        || line.line_id.as_str() == b.sink.endpoint_id.as_str()
        || b.binding_id.as_str() == b.base_instance_id.as_str()
        || line.availability.availability != LineAvailability::Ready
        || b.limits.maximum_in_flight_items == 0
        || b.limits.maximum_payload_bytes == 0
        || b.limits.maximum_buffered_bytes == 0
        || b.limits.maximum_in_flight_items < fore.item_capacity
        || b.limits.maximum_payload_bytes < fore.byte_capacity
        || b.limits.maximum_buffered_bytes < fore.byte_capacity
        || b.limits.maximum_frame_bytes < b.limits.maximum_payload_bytes
        || matches!(&b.credential, LinkCredentialReference::Opaque(v) if v.as_str().is_empty())
        || matches!(&b.authority, LinkAuthorityReference::Grant(v) if v.as_str().is_empty())
    {
        return None;
    }
    Some(())
}
fn line_backing(line: &LineOffer) -> Option<usize> {
    let b = &line.binding;
    let mut n = 0usize;
    for s in [
        &line.line_id.0,
        &b.binding_id.0,
        &b.base.0,
        &b.base_instance_id.0,
        &b.source.host_id.0,
        &b.source.boot_id.0,
        &b.source.endpoint_id.0,
        &b.sink.host_id.0,
        &b.sink.boot_id.0,
        &b.sink.endpoint_id.0,
    ] {
        n = n.checked_add(s.len())?;
    }
    if let LinkCredentialReference::Opaque(v) = &b.credential {
        n = n.checked_add(v.0.len())?;
    }
    if let LinkAuthorityReference::Grant(v) = &b.authority {
        n = n.checked_add(v.0.len())?;
    }
    Some(n)
}
pub(super) fn hex(bytes: [u8; 32]) -> String {
    let mut out = String::with_capacity(64);
    for byte in bytes {
        for digit in [byte >> 4, byte & 15] {
            out.push(b"0123456789abcdef"[usize::from(digit)] as char);
        }
    }
    out
}

fn cap_matches(g: &PlannedGear, c: &CapabilityOffer) -> bool {
    c.kind_id == g.kind_id
        && c.kind_contract_revision == g.kind_contract_revision
        && c.implementation.execution_profile_id == g.execution_profile_id
        && c.implementation.implementation_id == g.implementation_id
        && c.implementation.artifact_id == g.artifact_id
        && c.inputs == g.inputs
        && c.outputs == g.outputs
        && c.semantic_contract == g.semantic_contract
        && c.host_calls == g.host_calls
        && c.limits == g.limits
        && c.realization_properties == g.realization_properties
        && c.state_retention.is_none()
        && c.resource_requirements.len() == g.resources.len()
        && c.resource_requirements.iter().all(|r| {
            r.protected_role.is_none()
                && r.compute.is_none()
                && r.content.is_none()
                && g.resources
                    .iter()
                    .any(|b| b.class_id == r.class_id && b.units == r.units)
        })
        && c.authority_requirements.len() == g.authority.len()
        && c.authority_requirements.iter().all(|r| {
            g.authority.iter().any(|b| {
                b.contract_id == r.contract_id
                    && b.host_call_contract_id == r.host_call_contract_id
                    && b.subject_kind == r.subject_kind
            })
        })
}
