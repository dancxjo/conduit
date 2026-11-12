//! Direct native USB selection from original carrier, member and installed Host.
//! Authored Source and incoming frame metadata cannot initialize this provider.
use super::*;
use conduit_body::{BodyPlotPlan, ResidentPlot};
use conduit_core as c;
use conduit_embedded_build::SelectedForeSession;
use conduit_wire::PHYSICAL_FORE_SESSION_CONTRACT as SESSION_CONTRACT;
use std::collections::BTreeMap;
type Sessions = BTreeMap<ResidentPlot, Vec<SelectedForeSession>>;
type Choices = BTreeMap<ResidentPlot, owner::OwnedForeLineSelection>;
#[derive(Clone, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(in crate::durable_host::source_revision) struct DirectForeCarrierSelection {
    host_definition: std::path::PathBuf,
    usb_serial: String,
    plot: ResidentPlot,
    ports: Vec<super::super::super::local_fore_policy::LocalForePortIntent>,
}
/// Called once before Wake, with original sockets held through preparation.
/// Empty policy offers no Lines. Direct policy cannot fall back to a forwarded
/// TTY, stale member, declaration-only availability, or an unverified reopen.
pub(in crate::durable_host::source_revision) fn select_direct_fore_lines(
    plays: &[UsbPlaySelection],
    carriers: &[Result<SelectedPlayCarrier, String>],
    peer: &conduit_body::FederationPeer,
    partitions: &[BodyPlotPlan],
) -> Result<(Choices, Vec<Sessions>), String> {
    if plays.len() != carriers.len() {
        return Err("native carrier selection count differs".into());
    }
    let mut choices = Choices::new();
    let mut sessions = Vec::with_capacity(plays.len());
    for (play, carrier) in plays.iter().zip(carriers) {
        let mut selected = Sessions::new();
        if let Some(policy) = &play.direct_fore_carrier {
            let carrier = carrier.as_ref().map_err(Clone::clone)?;
            if !carrier.authenticated_return() {
                return Err("direct Fore requires the original completed member return".into());
            }
            carrier
                .socket
                .verify_local_usb_serial(&policy.usb_serial)
                .map_err(|e| format!("direct Fore physical attachment: {e:?}"))?;
            let (rows, picked) = describe(policy, &carrier.member, peer, partitions)?;
            let entry = choices.entry(policy.plot.clone()).or_insert_with(|| {
                owner::OwnedForeLineSelection {
                    choices: Vec::new(),
                    offers: Vec::new(),
                    allowed_bases: Vec::new(),
                }
            });
            entry.choices.extend(rows.choices);
            entry.offers.extend(rows.offers);
            for base in rows.allowed_bases {
                if !entry.allowed_bases.contains(&base) {
                    entry.allowed_bases.push(base);
                }
            }
            selected.insert(policy.plot.clone(), picked);
        }
        sessions.push(selected);
    }
    Ok((choices, sessions))
}
// This descriptive projection is reached only after actual descriptor/return
// checks above. It creates no native custody ticket or additional resource grant.
fn describe(
    policy: &DirectForeCarrierSelection,
    member: &owner::MemberPresentationBasis,
    peer: &conduit_body::FederationPeer,
    partitions: &[BodyPlotPlan],
) -> Result<(owner::OwnedForeLineSelection, Vec<SelectedForeSession>), String> {
    use conduit_host_make as h;
    if policy.ports.len() != 2
        || policy
            .ports
            .iter()
            .filter(|p| p.direction == c::PortDirection::Input)
            .count()
            != 1
    {
        return Err("direct duplex Fore requires exactly one request and one result".into());
    }
    let text = String::from_utf8(bounded_read(&policy.host_definition, 65536)?)
        .map_err(|e| e.to_string())?;
    let config = h::parse_host_configuration_conduit(&text)
        .map_err(|e| format!("installed carrier Host: {e:?}"))?;
    let configuration = h::host_configuration_identity(&config)
        .map_err(|e| format!("installed carrier identity: {e:?}"))?;
    if member.host_advertisement.profile.as_str() != configuration {
        return Err(
            "installed carrier Host differs from authenticated member configuration".into(),
        );
    }
    let bounds = h::fore_carrier::DeclaredForeCarrierBounds::from_configuration(&config)?
        .ok_or("installed Host declares no Fore carrier")?;
    let base = config
        .bases
        .iter()
        .find(|b| b.kind == h::fore_carrier::BASE_KIND)
        .unwrap();
    let implementation = base
        .implementation
        .as_deref()
        .filter(|s| !s.is_empty())
        .ok_or("direct carrier requires an independently pinned implementation")?;
    if !base.implementations.is_empty() || bounds.maximum_lines < 2 {
        return Err("direct carrier implementation or finite Line capacity is ambiguous".into());
    }
    let partition = partitions
        .iter()
        .find(|p| p.plot == policy.plot)
        .ok_or("direct carrier Plot absent from complete workset")?;
    let actual = &member.host_advertisement;
    let fragment = partition
        .plan
        .fragments
        .iter()
        .find(|f| f.host_id == actual.host_id && f.boot_id == actual.boot_id)
        .ok_or("direct carrier member has no exact current Plan fragment")?;
    let (lowered, _) = conduit_plan_lowering::lowering::lower_plan_fragment_from_plan(
        &partition.plan,
        &fragment.fragment_id,
    )
    .map_err(|e| format!("direct carrier Fore lowering: {e:?}"))?;
    let pins =
        conduit_embedded_build::assigned_fore_inventory(&partition.plan, &fragment.fragment_id, 16)
            .map_err(|e| format!("direct carrier Fore pins: {e:?}"))?;
    let limits = c::LinkLimits {
        maximum_in_flight_items: bounds.maximum_in_flight_items,
        maximum_frame_bytes: bounds.maximum_frame_bytes,
        maximum_payload_bytes: bounds.maximum_payload_bytes,
        maximum_buffered_bytes: bounds.maximum_buffered_bytes as u32,
    };
    let contract = c::LineContract {
        scope: c::LineScope::PointToPoint,
        traffic_shape: c::LineTrafficShape::Message,
        duplex: c::LineDuplex::FullDuplex,
        ordering: c::LineOrdering::Ordered,
        reliability: c::LineReliability::Reliable,
        continuation: c::LineContinuation::None,
        security: c::LineSecurity::PhysicalPossession,
    };
    let mut rows = owner::OwnedForeLineSelection {
        choices: Vec::new(),
        offers: Vec::new(),
        allowed_bases: vec![implementation.into()],
    };
    let mut sessions = Vec::new();
    for wanted in &policy.ports {
        let matching: Vec<_> = lowered
            .fore_ports
            .iter()
            .enumerate()
            .filter(|(_, p)| {
                p.front_port_id == wanted.front_port_id
                    && p.direction == wanted.direction
                    && p.track == wanted.track
            })
            .collect();
        let [(index, port)] = matching.as_slice() else {
            return Err("direct carrier requires one exact Fore selector".into());
        };
        if port.value_kind != wanted.value_kind
            || port.item_capacity != wanted.item_capacity
            || port.byte_capacity != wanted.byte_capacity
        {
            return Err(
                "direct carrier Fore differs from independently selected ABI limits".into(),
            );
        }
        let described = conduit_wire::describe_physical_fore_line(
            conduit_wire::PhysicalForeInstallation {
                body_id: member.credential.body_id.as_str(),
                configuration_id: &configuration,
                implementation_id: implementation,
                session_contract: SESSION_CONTRACT,
            },
            conduit_wire::PhysicalForePeerFacts {
                host_id: actual.host_id.as_str(),
                boot_id: actual.boot_id.as_str(),
                offer_generation: actual.offer_generation.0,
            },
            conduit_wire::PhysicalForePeerFacts {
                host_id: peer.host_id.as_str(),
                boot_id: peer.boot_id.as_str(),
                offer_generation: peer.offer_generation.0,
            },
            &pins[*index],
            limits,
            contract,
        )
        .map_err(|e| format!("direct physical Fore: {e:?}"))?;
        let line = described.line;
        let binding = line.binding;
        rows.choices.push(conduit_planner::ExternalForeLineChoice {
            host_id: actual.host_id.clone(),
            boot_id: actual.boot_id.clone(),
            peer_host_id: peer.host_id.clone(),
            peer_boot_id: peer.boot_id.clone(),
            direction: port.direction,
            front_port_id: port.front_port_id.clone(),
            track: port.track,
            line_id: line.line_id.clone(),
        });
        rows.offers.push(c::LineOffer {
            line_id: line.line_id.clone(),
            contract: line.contract,
            availability: c::LineAvailabilitySign {
                line_id: line.line_id,
                binding_id: binding.binding_id.clone(),
                availability: c::LineAvailability::Ready,
                sign_id: format!("native/{}", described.session_id).into(),
            },
            binding: c::LinkBinding {
                binding_id: binding.binding_id,
                source: binding.source,
                sink: binding.sink,
                base: binding.base,
                base_instance_id: binding.base_instance_id,
                credential: binding.credential,
                authority: binding.authority,
                limits: binding.limits,
            },
        });
        sessions.push(SelectedForeSession {
            front_port_id: port.front_port_id.clone(),
            direction: port.direction,
            track: port.track,
            session: c::AssignedIdentity::from_text(&described.session_id),
            session_contract: c::AssignedIdentity::from_text(SESSION_CONTRACT),
        });
    }
    Ok((rows, sessions))
}

#[cfg(test)]
#[path = "source_revision_fore_provider_tests.rs"]
mod tests;
