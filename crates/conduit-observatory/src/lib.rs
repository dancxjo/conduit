#![no_std]

extern crate alloc;

use alloc::collections::{BTreeMap, BTreeSet};
use alloc::format;
use alloc::string::String;
use alloc::vec::Vec;
use conduit_core::{
    AuthorityBinding, AuthorityRequirement, BootId, CapabilityId, CapabilityLimits, CheckedFormId,
    ConnectionId, ConnectionProvider, ExecutionProfileId, ExpandedFormId, HostAdvertisement,
    HostId, HostOperationRequirement, HostProfileId, ImplementationId, KindContractRevision,
    KindId, Observation, ObservationKind, OfferGeneration, PlacementId, Plan, PlanId,
    PortDescriptor, ResourceBinding, ResourceOffer, ResourceRequirement, SourceDocumentId,
    TerminalDisposition,
};
use conduit_realm::{LinkId, LinkState, MembershipState, RealmId, RealmView};
use core::fmt::Write;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
pub struct EvidenceId(String);

impl EvidenceId {
    pub fn as_str(&self) -> &str {
        &self.0
    }
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OperationalState {
    Available,
    Stale,
    Unreachable,
    Failed,
    Unsupported,
    Denied,
    Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum OfferFreshness {
    Fresh,
    Stale,
    Unknown,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilitySupport {
    Supported,
    Unsupported,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum CapabilityAvailability {
    Available,
    Unavailable,
}

#[derive(Debug, Copy, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub enum PlanLifecycle {
    Unknown,
    Prepared,
    Active,
    Completed,
    Failed,
    Cancelled,
    Released,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HostRow {
    pub realm_id: Option<RealmId>,
    pub host_id: HostId,
    pub boot_id: Option<BootId>,
    pub profile: Option<HostProfileId>,
    pub offer_generation: Option<OfferGeneration>,
    pub membership: Option<MembershipState>,
    pub state: OperationalState,
    pub capability_count: usize,
    pub resources: Vec<ResourceOffer>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct CapabilityRow {
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub inputs: Vec<PortDescriptor>,
    pub outputs: Vec<PortDescriptor>,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resource_requirements: Vec<ResourceRequirement>,
    pub authority_requirements: Vec<AuthorityRequirement>,
    pub limits: CapabilityLimits,
    pub freshness: OfferFreshness,
    pub support: CapabilitySupport,
    pub availability: CapabilityAvailability,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct LinkRow {
    pub realm_id: RealmId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub link_id: LinkId,
    pub remote_host_id: HostId,
    pub state: LinkState,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlanRow {
    pub plan_id: PlanId,
    pub source_document_id: SourceDocumentId,
    pub checked_form_id: CheckedFormId,
    pub expanded_form_id: ExpandedFormId,
    pub lifecycle: PlanLifecycle,
    pub terminal_disposition: Option<TerminalDisposition>,
    pub placement_count: usize,
    pub connection_count: usize,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementRow {
    pub plan_id: PlanId,
    pub placement_id: PlacementId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub capability_id: CapabilityId,
    pub kind_id: KindId,
    pub kind_contract_revision: KindContractRevision,
    pub execution_profile_id: ExecutionProfileId,
    pub implementation_id: ImplementationId,
    pub host_operations: Vec<HostOperationRequirement>,
    pub resources: Vec<ResourceBinding>,
    pub authority: Vec<AuthorityBinding>,
    pub lifecycle: PlanLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ConnectionRow {
    pub plan_id: PlanId,
    pub connection_id: ConnectionId,
    pub source_placement_id: PlacementId,
    pub sink_placement_id: PlacementId,
    pub value_kind: KindId,
    pub provider: ConnectionProvider,
    pub item_capacity: u16,
    pub byte_capacity: u32,
    pub lifecycle: PlanLifecycle,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct EvidenceRow {
    pub evidence_id: EvidenceId,
    pub host_id: HostId,
    pub boot_id: BootId,
    pub plan_id: Option<PlanId>,
    pub placement_id: Option<PlacementId>,
    pub connection_id: Option<ConnectionId>,
    pub kind: ObservationKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct RetentionRow {
    pub bounded: bool,
    pub visible_gap_count: u64,
    pub explanation: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObservatoryReport {
    pub hosts: Vec<HostRow>,
    pub capabilities: Vec<CapabilityRow>,
    pub links: Vec<LinkRow>,
    pub plans: Vec<PlanRow>,
    pub placements: Vec<PlacementRow>,
    pub connections: Vec<ConnectionRow>,
    pub evidence: Vec<EvidenceRow>,
    pub retention: RetentionRow,
}

pub fn build_report(
    advertisements: &[HostAdvertisement],
    realm_view: Option<&RealmView>,
    plans: &[Plan],
    observations: &[Observation],
) -> ObservatoryReport {
    let advertisement_by_host = advertisements
        .iter()
        .map(|advertisement| (advertisement.host_id.clone(), advertisement))
        .collect::<BTreeMap<_, _>>();
    let mut host_ids = advertisement_by_host
        .keys()
        .cloned()
        .collect::<BTreeSet<_>>();
    if let Some(view) = realm_view {
        host_ids.extend(view.members.iter().map(|member| member.host_id.clone()));
    }
    host_ids.extend(
        observations
            .iter()
            .map(|observation| observation.host_id.clone()),
    );

    let hosts = host_ids
        .into_iter()
        .map(|host_id| {
            let advertisement = advertisement_by_host.get(&host_id).copied();
            let member = realm_view
                .and_then(|view| view.members.iter().find(|member| member.host_id == host_id));
            let boot_id = advertisement
                .map(|advertisement| advertisement.boot_id.clone())
                .or_else(|| member.map(|member| member.boot_id.clone()));
            let state = host_state(advertisement, member, observations);
            HostRow {
                realm_id: realm_view.map(|view| view.realm_id.clone()),
                host_id,
                boot_id,
                profile: advertisement.map(|advertisement| advertisement.profile.clone()),
                offer_generation: advertisement.map(|advertisement| advertisement.offer_generation),
                membership: member.map(|member| member.state),
                state,
                capability_count: advertisement
                    .map(|advertisement| advertisement.capabilities.len())
                    .unwrap_or(0),
                resources: advertisement
                    .map(|advertisement| advertisement.resources.clone())
                    .unwrap_or_default(),
            }
        })
        .collect::<Vec<_>>();

    let capabilities = advertisements
        .iter()
        .flat_map(|advertisement| {
            let member = realm_view.and_then(|view| {
                view.members
                    .iter()
                    .find(|member| member.host_id == advertisement.host_id)
            });
            let freshness = match member {
                Some(member) if member.boot_id == advertisement.boot_id => OfferFreshness::Fresh,
                Some(_) => OfferFreshness::Stale,
                None => OfferFreshness::Unknown,
            };
            let available = matches!(
                host_state(Some(advertisement), member, observations),
                OperationalState::Available
            );
            advertisement
                .capabilities
                .iter()
                .map(move |capability| CapabilityRow {
                    host_id: advertisement.host_id.clone(),
                    boot_id: advertisement.boot_id.clone(),
                    capability_id: capability.capability_id.clone(),
                    kind_id: capability.kind_id.clone(),
                    kind_contract_revision: capability.kind_contract_revision.clone(),
                    execution_profile_id: capability.execution_profile_id.clone(),
                    implementation_id: capability.implementation_id.clone(),
                    inputs: capability.inputs.clone(),
                    outputs: capability.outputs.clone(),
                    host_operations: capability.host_operations.clone(),
                    resource_requirements: capability.resource_requirements.clone(),
                    authority_requirements: capability.authority_requirements.clone(),
                    limits: capability.limits.clone(),
                    freshness,
                    support: CapabilitySupport::Supported,
                    availability: if available {
                        CapabilityAvailability::Available
                    } else {
                        CapabilityAvailability::Unavailable
                    },
                })
        })
        .collect::<Vec<_>>();

    let links = realm_view
        .into_iter()
        .flat_map(|view| {
            view.members.iter().flat_map(move |member| {
                member.links.iter().map(move |link| LinkRow {
                    realm_id: view.realm_id.clone(),
                    host_id: member.host_id.clone(),
                    boot_id: member.boot_id.clone(),
                    link_id: link.link_id.clone(),
                    remote_host_id: link.remote_host_id.clone(),
                    state: link.state,
                })
            })
        })
        .collect::<Vec<_>>();

    let plan_rows = plans
        .iter()
        .map(|plan| {
            let (lifecycle, terminal_disposition) = plan_lifecycle(&plan.plan_id, observations);
            PlanRow {
                plan_id: plan.plan_id.clone(),
                source_document_id: plan.source_document_id.clone(),
                checked_form_id: plan.checked_form_id.clone(),
                expanded_form_id: plan.expanded_form_id.clone(),
                lifecycle,
                terminal_disposition,
                placement_count: plan
                    .fragments
                    .iter()
                    .flat_map(|fragment| &fragment.placements)
                    .map(|placement| placement.placement_id.clone())
                    .collect::<BTreeSet<_>>()
                    .len(),
                connection_count: plan
                    .fragments
                    .iter()
                    .flat_map(|fragment| &fragment.connections)
                    .map(|connection| connection.connection_id.clone())
                    .collect::<BTreeSet<_>>()
                    .len(),
            }
        })
        .collect::<Vec<_>>();

    let placements = plans
        .iter()
        .flat_map(|plan| {
            plan.fragments.iter().flat_map(move |fragment| {
                fragment
                    .placements
                    .iter()
                    .map(move |placement| PlacementRow {
                        plan_id: plan.plan_id.clone(),
                        placement_id: placement.placement_id.clone(),
                        host_id: placement.host_id.clone(),
                        boot_id: placement.boot_id.clone(),
                        capability_id: placement.capability_id.clone(),
                        kind_id: placement.kind_id.clone(),
                        kind_contract_revision: placement.kind_contract_revision.clone(),
                        execution_profile_id: placement.execution_profile_id.clone(),
                        implementation_id: placement.implementation_id.clone(),
                        host_operations: placement.host_operations.clone(),
                        resources: placement.resources.clone(),
                        authority: placement.authority.clone(),
                        lifecycle: placement_lifecycle(
                            &plan.plan_id,
                            &placement.placement_id,
                            observations,
                        ),
                    })
            })
        })
        .collect::<Vec<_>>();

    let connections = plans
        .iter()
        .flat_map(|plan| {
            plan.fragments.iter().flat_map(move |fragment| {
                fragment
                    .connections
                    .iter()
                    .map(move |connection| ConnectionRow {
                        plan_id: plan.plan_id.clone(),
                        connection_id: connection.connection_id.clone(),
                        source_placement_id: connection.source_placement_id.clone(),
                        sink_placement_id: connection.sink_placement_id.clone(),
                        value_kind: connection.value_kind.clone(),
                        provider: connection.provider,
                        item_capacity: connection.item_capacity,
                        byte_capacity: connection.byte_capacity,
                        lifecycle: connection_lifecycle(
                            &plan.plan_id,
                            &connection.connection_id,
                            observations,
                        ),
                    })
            })
        })
        .fold(
            BTreeMap::<(PlanId, ConnectionId), ConnectionRow>::new(),
            |mut rows, row| {
                rows.entry((row.plan_id.clone(), row.connection_id.clone()))
                    .or_insert(row);
                rows
            },
        )
        .into_values()
        .collect::<Vec<_>>();

    let evidence = observations
        .iter()
        .enumerate()
        .map(|(index, observation)| EvidenceRow {
            evidence_id: EvidenceId(format!("evidence/{index}")),
            host_id: observation.host_id.clone(),
            boot_id: observation.boot_id.clone(),
            plan_id: observation.plan_id.clone(),
            placement_id: observation.placement_id.clone(),
            connection_id: observation.connection_id.clone(),
            kind: observation.kind.clone(),
        })
        .collect::<Vec<_>>();
    let visible_gap_count = observations
        .iter()
        .filter_map(|observation| match observation.kind {
            ObservationKind::EvidenceGap { dropped } => Some(dropped),
            _ => None,
        })
        .sum();

    ObservatoryReport {
        hosts,
        capabilities,
        links,
        plans: plan_rows,
        placements,
        connections,
        evidence,
        retention: RetentionRow {
            bounded: true,
            visible_gap_count,
            explanation: String::from(
                "Observatory history is projected from bounded host evidence; EvidenceGap rows report omitted records.",
            ),
        },
    }
}

fn host_state(
    advertisement: Option<&HostAdvertisement>,
    member: Option<&conduit_realm::RealmMember>,
    observations: &[Observation],
) -> OperationalState {
    if observations.iter().any(|observation| {
        advertisement
            .map(|advertisement| observation.host_id == advertisement.host_id)
            .or_else(|| member.map(|member| observation.host_id == member.host_id))
            .unwrap_or(false)
            && matches!(observation.kind, ObservationKind::Failure { .. })
    }) {
        return OperationalState::Failed;
    }
    match (advertisement, member) {
        (_, Some(member)) if member.state == MembershipState::Denied => OperationalState::Denied,
        (_, Some(member)) if member.state == MembershipState::Departed => {
            OperationalState::Unreachable
        }
        (Some(advertisement), Some(member)) if advertisement.boot_id != member.boot_id => {
            OperationalState::Stale
        }
        (Some(_), _) => OperationalState::Available,
        (None, Some(_)) => OperationalState::Unknown,
        (None, None) => OperationalState::Unknown,
    }
}

fn plan_lifecycle(
    plan_id: &PlanId,
    observations: &[Observation],
) -> (PlanLifecycle, Option<TerminalDisposition>) {
    let mut lifecycle = PlanLifecycle::Unknown;
    let mut terminal = None;
    for observation in observations
        .iter()
        .filter(|observation| observation.plan_id.as_ref() == Some(plan_id))
    {
        match &observation.kind {
            ObservationKind::PlanTerminal { disposition } => {
                terminal = Some(*disposition);
                lifecycle = terminal_lifecycle(disposition);
            }
            ObservationKind::PlanCompleted => lifecycle = PlanLifecycle::Completed,
            ObservationKind::PlanActivated => lifecycle = PlanLifecycle::Active,
            ObservationKind::PlanFragmentReceived => {
                if lifecycle == PlanLifecycle::Unknown {
                    lifecycle = PlanLifecycle::Prepared;
                }
            }
            ObservationKind::Released => lifecycle = PlanLifecycle::Released,
            _ => {}
        }
    }
    (lifecycle, terminal)
}

fn placement_lifecycle(
    plan_id: &PlanId,
    placement_id: &PlacementId,
    observations: &[Observation],
) -> PlanLifecycle {
    let mut lifecycle = PlanLifecycle::Unknown;
    for observation in observations.iter().filter(|observation| {
        observation.plan_id.as_ref() == Some(plan_id)
            && observation.placement_id.as_ref() == Some(placement_id)
    }) {
        match &observation.kind {
            ObservationKind::PlacementTerminal { disposition } => {
                lifecycle = terminal_lifecycle(disposition);
            }
            ObservationKind::PlacementCompleted => lifecycle = PlanLifecycle::Completed,
            ObservationKind::PlanActivated => lifecycle = PlanLifecycle::Active,
            ObservationKind::PlacementPrepared => lifecycle = PlanLifecycle::Prepared,
            ObservationKind::Released => lifecycle = PlanLifecycle::Released,
            _ => {}
        }
    }
    lifecycle
}

fn connection_lifecycle(
    plan_id: &PlanId,
    connection_id: &ConnectionId,
    observations: &[Observation],
) -> PlanLifecycle {
    let mut lifecycle = PlanLifecycle::Unknown;
    for observation in observations.iter().filter(|observation| {
        observation.plan_id.as_ref() == Some(plan_id)
            && observation.connection_id.as_ref() == Some(connection_id)
    }) {
        if let ObservationKind::ConnectionTerminal { disposition } = &observation.kind {
            lifecycle = terminal_lifecycle(&disposition.disposition);
        }
    }
    lifecycle
}

fn terminal_lifecycle(disposition: &TerminalDisposition) -> PlanLifecycle {
    match disposition {
        TerminalDisposition::Completed => PlanLifecycle::Completed,
        TerminalDisposition::Failed { .. } => PlanLifecycle::Failed,
        TerminalDisposition::Cancelled { .. } => PlanLifecycle::Cancelled,
    }
}

pub fn unsupported_state() -> OperationalState {
    OperationalState::Unsupported
}

pub fn render_text_report(report: &ObservatoryReport) -> String {
    let mut output = String::new();
    let _ = writeln!(output, "host observatory report");
    let _ = writeln!(output, "hosts {}", report.hosts.len());
    for host in &report.hosts {
        let _ = writeln!(
            output,
            "host id={} boot={} realm={} state={:?} membership={:?} capabilities={} resources={:?}",
            host.host_id.as_str(),
            host.boot_id
                .as_ref()
                .map(BootId::as_str)
                .unwrap_or("unknown"),
            host.realm_id
                .as_ref()
                .map(RealmId::as_str)
                .unwrap_or("unknown"),
            host.state,
            host.membership,
            host.capability_count,
            host.resources
        );
    }
    let _ = writeln!(output, "capabilities {}", report.capabilities.len());
    for capability in &report.capabilities {
        let _ = writeln!(
            output,
            "capability host={} boot={} capability={} kind={} contract={} execution_profile={} implementation={} input_ports={} output_ports={} host_operations={:?} resource_requirements={:?} authority_requirements={:?} active_limit={} queue_items={} queue_bytes={} freshness={:?} support={:?} availability={:?}",
            capability.host_id.as_str(),
            capability.boot_id.as_str(),
            capability.capability_id.as_str(),
            capability.kind_id.as_str(),
            capability.kind_contract_revision.as_str(),
            capability.execution_profile_id.as_str(),
            capability.implementation_id.as_str(),
            capability.inputs.len(),
            capability.outputs.len(),
            capability.host_operations,
            capability.resource_requirements,
            capability.authority_requirements,
            capability.limits.max_active_instances,
            capability.limits.max_queue_items,
            capability.limits.max_queue_bytes,
            capability.freshness,
            capability.support,
            capability.availability
        );
    }
    let _ = writeln!(output, "links {}", report.links.len());
    for link in &report.links {
        let _ = writeln!(
            output,
            "link realm={} host={} boot={} link={} remote={} state={:?}",
            link.realm_id.as_str(),
            link.host_id.as_str(),
            link.boot_id.as_str(),
            link.link_id.as_str(),
            link.remote_host_id.as_str(),
            link.state
        );
    }
    let _ = writeln!(output, "plans {}", report.plans.len());
    for plan in &report.plans {
        let _ = writeln!(
            output,
            "plan id={} source_document={} checked_form={} expanded_form={} lifecycle={:?} terminal={:?} placements={} connections={}",
            plan.plan_id.as_str(),
            plan.source_document_id.as_str(),
            plan.checked_form_id.as_str(),
            plan.expanded_form_id.as_str(),
            plan.lifecycle,
            plan.terminal_disposition,
            plan.placement_count,
            plan.connection_count
        );
    }
    let _ = writeln!(output, "placements {}", report.placements.len());
    for placement in &report.placements {
        let _ = writeln!(
            output,
            "placement plan={} placement={} host={} boot={} capability={} kind={} contract={} execution_profile={} implementation={} host_operations={:?} resources={:?} authority={:?} lifecycle={:?}",
            placement.plan_id.as_str(),
            placement.placement_id.as_str(),
            placement.host_id.as_str(),
            placement.boot_id.as_str(),
            placement.capability_id.as_str(),
            placement.kind_id.as_str(),
            placement.kind_contract_revision.as_str(),
            placement.execution_profile_id.as_str(),
            placement.implementation_id.as_str(),
            placement.host_operations,
            placement.resources,
            placement.authority,
            placement.lifecycle
        );
    }
    let _ = writeln!(output, "connections {}", report.connections.len());
    for connection in &report.connections {
        let _ = writeln!(
            output,
            "connection plan={} connection={} source={} sink={} value_kind={} provider={:?} queue_items={} queue_bytes={} lifecycle={:?}",
            connection.plan_id.as_str(),
            connection.connection_id.as_str(),
            connection.source_placement_id.as_str(),
            connection.sink_placement_id.as_str(),
            connection.value_kind.as_str(),
            connection.provider,
            connection.item_capacity,
            connection.byte_capacity,
            connection.lifecycle
        );
    }
    let _ = writeln!(output, "evidence {}", report.evidence.len());
    for evidence in &report.evidence {
        let _ = writeln!(
            output,
            "evidence id={} host={} boot={} plan={} placement={} connection={} kind={:?}",
            evidence.evidence_id.as_str(),
            evidence.host_id.as_str(),
            evidence.boot_id.as_str(),
            evidence
                .plan_id
                .as_ref()
                .map(PlanId::as_str)
                .unwrap_or("none"),
            evidence
                .placement_id
                .as_ref()
                .map(PlacementId::as_str)
                .unwrap_or("none"),
            evidence
                .connection_id
                .as_ref()
                .map(ConnectionId::as_str)
                .unwrap_or("none"),
            evidence.kind
        );
    }
    let _ = writeln!(
        output,
        "retention bounded={} visible_gaps={} explanation={}",
        report.retention.bounded, report.retention.visible_gap_count, report.retention.explanation
    );
    output
}

#[cfg(test)]
mod tests {
    use alloc::collections::BTreeMap;
    use alloc::vec;

    use super::{
        build_report, render_text_report, unsupported_state, CapabilityAvailability,
        CapabilitySupport, OfferFreshness, OperationalState, PlanLifecycle,
    };
    use conduit_browser_sim::{BrowserSimConfig, BrowserSimPage};
    use conduit_core::{
        authority_grant, present_authority_requirement, BootId, CapabilityId, ConnectionProvider,
        HostCommand, HostId, ObservationKind, OfferGeneration, OperationId, TerminalDisposition,
    };
    use conduit_form::parse;
    use conduit_pico_sim::{pico_advertisement, PicoSimConfig};
    use conduit_planner::{plan_with_options, PlacementChoice, PlacementChoices, PlanningOptions};
    use conduit_realm::{AdmissionRequest, LinkId, Realm, RealmId};
    use conduit_signal::signal_profile_catalog;
    use conduit_std_host::{StdHost, StdHostConfig};

    #[test]
    fn report_separates_identity_capability_plan_connection_and_evidence_tables() {
        let mut std_host = StdHost::new_with_config(StdHostConfig {
            host_id: HostId::from("std-host-triple"),
            boot_id: BootId::from("std-boot-triple"),
            offer_generation: OfferGeneration(1),
        });
        let page = BrowserSimPage::with_hosts([BrowserSimConfig {
            host_id: HostId::from("browser-sim-triple"),
            boot_id: BootId::from("browser-sim-boot-triple"),
            offer_generation: OfferGeneration(1),
        }]);
        let pico_ad = pico_advertisement(PicoSimConfig {
            host_id: HostId::from("pico-sim-triple"),
            boot_id: BootId::from("pico-sim-boot-triple"),
            offer_generation: OfferGeneration(1),
        });
        let mut browser_ad = page
            .advertisements()
            .into_iter()
            .next()
            .expect("browser advertisement exists");
        let browser_host_id = browser_ad.host_id.clone();
        let browser_boot_id = browser_ad.boot_id.clone();
        let browser_capability = browser_ad
            .capabilities
            .iter_mut()
            .find(|capability| capability.capability_id == CapabilityId::from("dom-show"))
            .expect("browser presentation capability exists");
        let presentation_subject = browser_capability
            .host_operations
            .iter()
            .find_map(|requirement| requirement.target_kind.clone())
            .expect("browser presentation declares a target subject");
        let authority_requirement = present_authority_requirement(presentation_subject);
        browser_capability
            .authority_requirements
            .push(authority_requirement.clone());
        let browser_authority_grant = authority_grant(
            "grant/browser-presentation",
            &authority_requirement,
            browser_host_id,
            browser_boot_id,
            browser_capability.capability_id.clone(),
        );
        let advertisements = vec![
            std_host.advertisement().clone(),
            browser_ad.clone(),
            pico_ad.clone(),
        ];

        let mut realm = Realm::found(
            RealmId::from("realm-m3"),
            advertisements[0].clone(),
            LinkId::from("link-std"),
            16,
        );
        realm
            .admit(AdmissionRequest {
                advertisement: browser_ad.clone(),
                link_id: LinkId::from("link-browser"),
                allow: true,
            })
            .expect("browser joins realm");
        realm
            .admit(AdmissionRequest {
                advertisement: pico_ad.clone(),
                link_id: LinkId::from("link-pico"),
                allow: true,
            })
            .expect("pico joins realm");
        let realm_view = realm
            .view_for(&HostId::from("std-host-triple"))
            .expect("std host observes realm");

        let form = parse(
            include_str!("../../../examples/triple-signal.form"),
            &signal_profile_catalog(),
        )
        .expect("triple form parses");
        let placements = PlacementChoices {
            by_operation: BTreeMap::from([
                (
                    OperationId::from("pulse"),
                    PlacementChoice {
                        host_id: HostId::from("std-host-triple"),
                        capability_id: CapabilityId::from("pulse-1"),
                    },
                ),
                (
                    OperationId::from("local"),
                    PlacementChoice {
                        host_id: HostId::from("std-host-triple"),
                        capability_id: CapabilityId::from("stdout-show-1"),
                    },
                ),
                (
                    OperationId::from("web"),
                    PlacementChoice {
                        host_id: HostId::from("browser-sim-triple"),
                        capability_id: CapabilityId::from("dom-show"),
                    },
                ),
                (
                    OperationId::from("light"),
                    PlacementChoice {
                        host_id: HostId::from("pico-sim-triple"),
                        capability_id: CapabilityId::from("onboard-led"),
                    },
                ),
            ]),
        };
        let connection_providers = BTreeMap::from([
            (
                (OperationId::from("pulse"), OperationId::from("local")),
                ConnectionProvider::Local,
            ),
            (
                (OperationId::from("pulse"), OperationId::from("web")),
                ConnectionProvider::FixtureFrame,
            ),
            (
                (OperationId::from("pulse"), OperationId::from("light")),
                ConnectionProvider::FixtureDatagram,
            ),
        ]);
        let plan = plan_with_options(
            &form,
            &advertisements,
            &placements,
            &[
                ConnectionProvider::Local,
                ConnectionProvider::FixtureFrame,
                ConnectionProvider::FixtureDatagram,
            ],
            PlanningOptions {
                connection_providers: &connection_providers,
                connection_item_capacity: 4,
                connection_byte_capacity: 64,
                authority_grants: core::slice::from_ref(&browser_authority_grant),
            },
        )
        .expect("M1 triple-simulation plan resolves");
        let fragment = plan
            .fragments
            .iter()
            .find(|fragment| fragment.host_id == HostId::from("std-host-triple"))
            .expect("std fragment exists")
            .clone();
        let _ = std_host.handle(HostCommand::Prepare(fragment.clone()));
        let _ = std_host.handle(HostCommand::Activate(fragment.plan_id.clone()));
        let observations = std_host
            .handle(HostCommand::Inspect)
            .events
            .into_iter()
            .find_map(|event| match event {
                conduit_core::HostEvent::Observations { items } => Some(items),
                _ => None,
            })
            .expect("inspect returns observations");

        let report = build_report(&advertisements, Some(&realm_view), &[plan], &observations);
        assert_eq!(report.hosts.len(), 3);
        assert_eq!(report.links.len(), 3);
        assert!(report
            .hosts
            .iter()
            .all(|host| host.realm_id == Some(RealmId::from("realm-m3"))));
        assert!(report.hosts.iter().all(|host| host.boot_id.is_some()));
        assert!(report.capabilities.iter().all(|capability| {
            capability.support == CapabilitySupport::Supported
                && capability.availability == CapabilityAvailability::Available
                && capability.freshness == OfferFreshness::Fresh
        }));
        assert_eq!(report.plans.len(), 1);
        assert_eq!(report.plans[0].placement_count, 4);
        assert_eq!(report.plans[0].connection_count, 3);
        assert_eq!(report.placements.len(), 4);
        assert!(report.capabilities.iter().any(|capability| {
            capability.capability_id == CapabilityId::from("dom-show")
                && capability.authority_requirements == vec![authority_requirement.clone()]
        }));
        assert!(report.placements.iter().any(|placement| {
            placement.capability_id == CapabilityId::from("dom-show")
                && placement.authority.len() == 1
                && placement.authority[0].grant_id == browser_authority_grant.grant_id
        }));
        assert_eq!(report.connections.len(), 3);
        assert!(report
            .connections
            .iter()
            .any(|connection| connection.provider == ConnectionProvider::FixtureFrame));
        assert!(report
            .connections
            .iter()
            .any(|connection| connection.provider == ConnectionProvider::FixtureDatagram));
        assert!(report
            .evidence
            .iter()
            .all(|row| !row.evidence_id.as_str().is_empty()));
        assert!(report.evidence.iter().any(|row| {
            row.plan_id == Some(report.plans[0].plan_id.clone())
                && matches!(row.kind, ObservationKind::PlanActivated)
        }));
        assert!(report.retention.bounded);

        let rendered = render_text_report(&report);
        assert!(rendered.contains("host observatory report"));
        assert!(rendered.contains("host id=std-host-triple boot=std-boot-triple"));
        assert!(rendered.contains("capability host=browser-sim-triple"));
        assert!(rendered.contains(conduit_core::PRESENT_HOST_OPERATION_CONTRACT));
        assert!(rendered.contains("presentation/signal"));
        assert!(rendered.contains(conduit_core::PRESENTATION_RESOURCE_CLASS));
        assert!(rendered.contains("browser/presentation"));
        assert!(rendered.contains(conduit_core::PRESENT_AUTHORITY_CONTRACT));
        assert!(rendered.contains("grant/browser-presentation"));
        assert!(rendered.contains("provider=FixtureFrame"));
        assert!(rendered.contains("provider=FixtureDatagram"));
        assert!(rendered.contains("evidence id=evidence/"));
    }

    #[test]
    fn status_vocabulary_keeps_failure_modes_distinct() {
        assert_ne!(OperationalState::Stale, OperationalState::Unreachable);
        assert_ne!(OperationalState::Failed, OperationalState::Denied);
        assert_ne!(OperationalState::Unknown, unsupported_state());
        assert_ne!(PlanLifecycle::Failed, PlanLifecycle::Cancelled);
        assert_ne!(
            TerminalDisposition::Completed,
            TerminalDisposition::Failed {
                reason: conduit_core::FailureReason::UnsupportedKind,
            }
        );
    }
}
