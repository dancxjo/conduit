//! Bounded, read-only presentation history over validated Observatory reports.

use conduit_observatory::{build_report, ObservatoryReport, ObservatorySnapshot};
use std::collections::VecDeque;

pub const MAX_TOPOLOGY_LINES: usize = 256;
pub const MAX_RETAINED_REPORT_BYTES: usize = 64 * 1024;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TopologyViewError {
    ZeroHistoryCapacity,
    InvalidReport(String),
    ReportTooLarge,
    PresentationTooLarge,
}

impl std::fmt::Display for TopologyViewError {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::ZeroHistoryCapacity => formatter.write_str("topology history capacity is zero"),
            Self::InvalidReport(message) => {
                write!(formatter, "invalid Observatory report: {message}")
            }
            Self::ReportTooLarge => {
                formatter.write_str("Observatory report exceeds Patchbay retention bounds")
            }
            Self::PresentationTooLarge => {
                formatter.write_str("topology presentation exceeds its finite line bound")
            }
        }
    }
}

impl std::error::Error for TopologyViewError {}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct TopologyDocument {
    lines: Vec<String>,
}

impl TopologyDocument {
    pub fn lines(&self) -> &[String] {
        &self.lines
    }
}

pub struct PatchbayTopology {
    history_capacity: usize,
    history: VecDeque<ObservatoryReport>,
    dropped_reports: u64,
}

impl PatchbayTopology {
    pub fn new(history_capacity: usize) -> Result<Self, TopologyViewError> {
        if history_capacity == 0 {
            return Err(TopologyViewError::ZeroHistoryCapacity);
        }
        Ok(Self {
            history_capacity,
            history: VecDeque::with_capacity(history_capacity),
            dropped_reports: 0,
        })
    }

    /// Validates and projects before changing retained presentation state.
    pub fn ingest(&mut self, snapshot: &ObservatorySnapshot) -> Result<(), TopologyViewError> {
        let report = build_report(snapshot).map_err(TopologyViewError::InvalidReport)?;
        let report_bytes = conduit_observatory::render_text_report(&report).len();
        let presentation_lines = 4usize
            .saturating_add(report.hosts.len())
            .saturating_add(report.capabilities.len())
            .saturating_add(
                report
                    .hosts
                    .iter()
                    .map(|host| host.planner_capabilities.len() + host.resources.len())
                    .sum::<usize>(),
            )
            .saturating_add(report.links.len())
            .saturating_add(report.clues.len());
        if report_bytes > MAX_RETAINED_REPORT_BYTES || presentation_lines > MAX_TOPOLOGY_LINES {
            return Err(TopologyViewError::ReportTooLarge);
        }
        if self.history.len() == self.history_capacity {
            self.history.pop_front();
            self.dropped_reports = self.dropped_reports.saturating_add(1);
        }
        self.history.push_back(report);
        Ok(())
    }

    pub fn retained_reports(&self) -> usize {
        self.history.len()
    }

    pub fn history_capacity(&self) -> usize {
        self.history_capacity
    }

    pub fn dropped_reports(&self) -> u64 {
        self.dropped_reports
    }

    pub fn current_report(&self) -> Option<&ObservatoryReport> {
        self.history.back()
    }

    /// Builds a sorted presentation copy. Filtering and sorting never mutate
    /// either the retained report or its Conduit facts.
    pub fn document(&self, filter: Option<&str>) -> Result<TopologyDocument, TopologyViewError> {
        let report = self.current_report();
        let mut lines = Vec::new();
        push_line(
            &mut lines,
            format!(
                "CURRENT REPORTS retained={} capacity={} dropped={}",
                self.retained_reports(),
                self.history_capacity(),
                self.dropped_reports()
            ),
        )?;
        let Some(report) = report else {
            push_line(&mut lines, "HOSTS none reported".into())?;
            push_line(&mut lines, "LINKS none reported".into())?;
            return Ok(TopologyDocument { lines });
        };

        let mut hosts = report.hosts.iter().collect::<Vec<_>>();
        hosts.sort_by(|left, right| {
            (&left.host_id, &left.boot_id).cmp(&(&right.host_id, &right.boot_id))
        });
        push_line(&mut lines, format!("HOSTS {}", hosts.len()))?;
        for host in hosts {
            push_line(
                &mut lines,
                format!(
                    "  host={} boot={} state={:?} profile={} generation={}",
                    host.host_id.as_str(),
                    host.boot_id.as_str(),
                    host.state,
                    host.profile.as_str(),
                    host.offer_generation.0
                ),
            )?;
            let mut capabilities = report
                .capabilities
                .iter()
                .filter(|row| row.host_id == host.host_id && row.boot_id == host.boot_id)
                .collect::<Vec<_>>();
            capabilities.sort_by(|left, right| left.capability_id.cmp(&right.capability_id));
            for capability in capabilities {
                push_line(
                    &mut lines,
                    format!(
                        "    operation={} kind={} contract={} support={:?} availability={:?} freshness={:?}",
                        capability.capability_id.as_str(),
                        capability.kind_id.as_str(),
                        capability.kind_contract_revision.as_str(),
                        capability.support,
                        capability.availability,
                        capability.freshness
                    ),
                )?;
            }
            let mut planners = host.planner_capabilities.iter().collect::<Vec<_>>();
            planners.sort_by(|left, right| left.profile_id.cmp(&right.profile_id));
            for planner in planners {
                push_line(
                    &mut lines,
                    format!(
                        "    planner={} hosts={} operations={} connections={}",
                        planner.profile_id.as_str(),
                        planner.limits.maximum_host_advertisements,
                        planner.limits.maximum_gears,
                        planner.limits.maximum_connections
                    ),
                )?;
            }
            let mut resources = host.resources.iter().collect::<Vec<_>>();
            resources.sort_by(|left, right| left.pool_id.cmp(&right.pool_id));
            for resource in resources {
                push_line(
                    &mut lines,
                    format!(
                        "    resource={} class={} capacity={}",
                        resource.pool_id.as_str(),
                        resource.class_id.as_str(),
                        resource.capacity_units
                    ),
                )?;
            }
        }

        let mut links = report.links.iter().collect::<Vec<_>>();
        links.sort_by(|left, right| left.binding.binding_id.cmp(&right.binding.binding_id));
        push_line(&mut lines, format!("LINKS {}", links.len()))?;
        for link in links {
            push_line(
                &mut lines,
                format!(
                    "  link={} {}@{} -> {}@{} base={:?} instance={} report={:?} availability={:?}",
                    link.binding.binding_id.as_str(),
                    link.binding.source.host_id.as_str(),
                    link.binding.source.boot_id.as_str(),
                    link.binding.sink.host_id.as_str(),
                    link.binding.sink.boot_id.as_str(),
                    link.binding.base,
                    link.binding.base_instance_id.as_str(),
                    link.state,
                    link.binding.availability
                ),
            )?;
        }

        push_line(
            &mut lines,
            format!(
                "OBSERVATIONS {} retained={} capacity={} visible_gaps={}",
                report.clues.len(),
                report.retention.retained_items,
                report.retention.item_capacity,
                report.retention.visible_gap_count
            ),
        )?;
        for clue in &report.clues {
            push_line(
                &mut lines,
                format!(
                    "  clue={} host={} boot={} kind={:?}",
                    clue.clue_id.as_str(),
                    clue.host_id.as_str(),
                    clue.boot_id.as_str(),
                    clue.kind
                ),
            )?;
        }

        if let Some(filter) = filter {
            let header = lines.remove(0);
            lines.retain(|line| line.contains(filter));
            lines.insert(0, header);
        }
        Ok(TopologyDocument { lines })
    }
}

fn push_line(lines: &mut Vec<String>, line: String) -> Result<(), TopologyViewError> {
    if lines.len() == MAX_TOPOLOGY_LINES {
        return Err(TopologyViewError::PresentationTooLarge);
    }
    lines.push(line);
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::{BootId, ClueId, HostId, LinkAvailability, Observation, ObservationKind};
    use conduit_observatory::{
        CapabilityAvailability, CapabilityStatusReport, CapabilitySupport, HostReport, LinkReport,
        ObservatorySnapshot, OfferFreshness, OperationalState, RetentionReport, SNAPSHOT_SCHEMA,
    };

    fn host_report(
        advertisement: conduit_core::HostAdvertisement,
        state: OperationalState,
    ) -> HostReport {
        let capabilities = advertisement
            .capabilities
            .iter()
            .map(|offer| CapabilityStatusReport {
                capability_id: offer.capability_id.clone(),
                freshness: if state == OperationalState::Stale {
                    OfferFreshness::Stale
                } else {
                    OfferFreshness::Fresh
                },
                support: CapabilitySupport::Supported,
                availability: if state == OperationalState::Available {
                    CapabilityAvailability::Available
                } else {
                    CapabilityAvailability::Unavailable
                },
            })
            .collect();
        HostReport {
            advertisement,
            state,
            capabilities,
        }
    }

    fn fleet_snapshot(link_available: bool) -> ObservatorySnapshot {
        let exact = conduit_signal::triple::exact_plan().unwrap();
        let laptop = exact.source_advertisement;
        let browser_new = exact.browser_advertisement;
        let mut browser_old = browser_new.clone();
        browser_old.boot_id = BootId::from("s4/triple-browser-old-boot");
        let pico = exact.pico_advertisement;
        let websocket = exact.browser_link;
        let mut usb = exact.pico_link;
        if !link_available {
            usb.availability = LinkAvailability::Unavailable;
        }
        let observations = vec![Observation {
            clue_id: ClueId::from("fleet/clue-1"),
            active_play_id: None,
            presentation_id: None,
            host_id: laptop.host_id.clone(),
            boot_id: laptop.boot_id.clone(),
            plan_id: None,
            placement_id: None,
            connection_id: None,
            kind: ObservationKind::AdvertisementPublished,
        }];
        ObservatorySnapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            hosts: vec![
                host_report(laptop, OperationalState::Available),
                host_report(browser_old, OperationalState::Stale),
                host_report(browser_new, OperationalState::Available),
                host_report(pico, OperationalState::Available),
            ],
            links: vec![
                LinkReport {
                    binding: websocket,
                    state: OperationalState::Available,
                },
                LinkReport {
                    binding: usb,
                    state: if link_available {
                        OperationalState::Available
                    } else {
                        OperationalState::Unreachable
                    },
                },
            ],
            plans: vec![exact.plan],
            plays: Vec::new(),
            retention: RetentionReport {
                item_capacity: 2,
                retained_items: 1,
                dropped_items: 3,
            },
            observations,
        }
    }

    #[test]
    fn bounded_fleet_view_keeps_exact_boot_capability_resource_link_and_gap_facts() {
        let mut topology = PatchbayTopology::new(2).unwrap();
        topology.ingest(&fleet_snapshot(true)).unwrap();
        let document = topology.document(None).unwrap().lines().join("\n");

        assert!(
            document.contains("host=s4/triple-browser boot=s4/triple-browser-old-boot state=Stale")
        );
        assert!(
            document.contains("host=s4/triple-browser boot=s4/triple-browser-boot state=Available")
        );
        assert!(document.contains("operation="));
        assert!(document.contains("resource="));
        assert!(document.contains("base=WebSocket"));
        assert!(document.contains("base=UsbCdc"));
        assert!(document.contains("visible_gaps=3"));
    }

    #[test]
    fn link_update_changes_only_reported_availability_and_retention_is_visible() {
        let mut topology = PatchbayTopology::new(2).unwrap();
        let available = fleet_snapshot(true);
        let unavailable = fleet_snapshot(false);
        let plan_before = available.plans[0].plan_id.clone();
        topology.ingest(&available).unwrap();
        topology.ingest(&unavailable).unwrap();
        topology.ingest(&available).unwrap();

        assert_eq!(topology.retained_reports(), 2);
        assert_eq!(topology.dropped_reports(), 1);
        let current = topology.current_report().unwrap();
        assert_eq!(current.plans[0].plan_id, plan_before);
        assert_eq!(current.hosts[0].host_id, HostId::from("s4/triple-std"));
        let document = topology.document(None).unwrap().lines().join("\n");
        assert!(document.contains("retained=2 capacity=2 dropped=1"));
        assert!(document.contains("availability=Ready"));
    }

    #[test]
    fn portable_planner_offers_are_visible_only_when_the_report_advertises_them() {
        let model = crate::PatchbayModel::with_identity(
            HostId::from("patchbay-planner-host"),
            BootId::from("patchbay-planner-boot"),
        );
        let mut topology = PatchbayTopology::new(1).unwrap();
        topology.ingest(&model.startup_snapshot()).unwrap();
        let document = topology.document(None).unwrap().lines().join("\n");

        assert!(document.contains("planner=conduit.planner/full@1"));
        assert_eq!(
            topology.current_report().unwrap().hosts[0]
                .planner_capabilities
                .len(),
            1
        );
    }

    #[test]
    fn absent_reports_and_presentation_controls_cannot_invent_facts() {
        let topology = PatchbayTopology::new(1).unwrap();
        let empty = topology.document(Some("invented")).unwrap();
        assert_eq!(
            empty.lines(),
            &[
                "CURRENT REPORTS retained=0 capacity=1 dropped=0".to_string(),
                "HOSTS none reported".to_string(),
                "LINKS none reported".to_string()
            ]
        );

        let mut topology = PatchbayTopology::new(1).unwrap();
        topology.ingest(&fleet_snapshot(true)).unwrap();
        let before = topology.current_report().unwrap().clone();
        let filtered = topology.document(Some("pico")).unwrap();
        assert!(filtered
            .lines()
            .iter()
            .all(|line| { line.starts_with("CURRENT REPORTS") || line.contains("pico") }));
        assert_eq!(&before, topology.current_report().unwrap());
        assert!(!filtered
            .lines()
            .iter()
            .any(|line| line.contains("invented")));
    }

    #[test]
    fn invalid_input_does_not_replace_the_last_valid_report() {
        let mut topology = PatchbayTopology::new(1).unwrap();
        topology.ingest(&fleet_snapshot(true)).unwrap();
        let before = topology.current_report().unwrap().clone();
        let mut invalid = fleet_snapshot(true);
        invalid.schema = "unknown".into();

        assert!(matches!(
            topology.ingest(&invalid),
            Err(TopologyViewError::InvalidReport(_))
        ));
        assert_eq!(&before, topology.current_report().unwrap());
    }

    #[test]
    fn oversized_report_is_rejected_before_it_enters_retained_history() {
        let model = crate::PatchbayModel::with_identity(
            HostId::from("bounded-host"),
            BootId::from("bounded-boot"),
        );
        let mut hosts = Vec::new();
        for index in 0..130 {
            let mut advertisement = model.advertisement().clone();
            advertisement.host_id = HostId::from(format!("host-{index}"));
            advertisement.boot_id = BootId::from(format!("boot-{index}"));
            hosts.push(host_report(advertisement, OperationalState::Available));
        }
        let oversized = ObservatorySnapshot {
            schema: SNAPSHOT_SCHEMA.into(),
            hosts,
            links: Vec::new(),
            plans: Vec::new(),
            plays: Vec::new(),
            observations: Vec::new(),
            retention: RetentionReport {
                item_capacity: 0,
                retained_items: 0,
                dropped_items: 0,
            },
        };
        let mut topology = PatchbayTopology::new(1).unwrap();

        assert_eq!(
            topology.ingest(&oversized),
            Err(TopologyViewError::ReportTooLarge)
        );
        assert_eq!(topology.retained_reports(), 0);
    }
}
