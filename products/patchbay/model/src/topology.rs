//! Bounded, read-only presentation history over validated Observatory reports.

use conduit_observatory::{build_report, ObservatoryReport, ObservatorySnapshot};
use std::collections::VecDeque;

use crate::topology_hosts::render_hosts;

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
        if report_bytes > MAX_RETAINED_REPORT_BYTES
            || topology_line_upper_bound(&report) > MAX_TOPOLOGY_LINES
        {
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
        let Some(report) = self.current_report() else {
            push_line(&mut lines, "HOSTS none reported".into())?;
            push_line(&mut lines, "LINES none reported".into())?;
            return Ok(TopologyDocument { lines });
        };
        render_hosts(&mut lines, report)?;
        render_bases(&mut lines, report)?;
        render_plans_and_plays(&mut lines, report)?;
        render_lines(&mut lines, report)?;
        render_signs(&mut lines, report)?;
        render_provenance(&mut lines, report)?;
        if let Some(filter) = filter {
            let header = lines.remove(0);
            lines.retain(|line| line.contains(filter));
            lines.insert(0, header);
        }
        Ok(TopologyDocument { lines })
    }
}

fn render_bases(
    lines: &mut Vec<String>,
    report: &ObservatoryReport,
) -> Result<(), TopologyViewError> {
    let mut bases = report.bases.iter().collect::<Vec<_>>();
    bases.sort_by(|left, right| left.base_id.cmp(&right.base_id));
    push_line(lines, format!("BASES {}", bases.len()))?;
    for base in bases {
        push_line(
            lines,
            format!(
                "  BASE {} kind={} host={} boot={} state={:?} capacity={}",
                base.base_id.as_str(),
                base.kind_id.as_str(),
                base.host_id.as_str(),
                base.boot_id.as_str(),
                base.state,
                base.capacity_units
            ),
        )?;
    }
    Ok(())
}

fn render_plans_and_plays(
    lines: &mut Vec<String>,
    report: &ObservatoryReport,
) -> Result<(), TopologyViewError> {
    push_line(lines, format!("PLANS {}", report.plans.len()))?;
    for plan in &report.plans {
        push_line(
            lines,
            format!(
                "  PLAN {} source={} checked={} expanded={} fragments={} placements={} connections={}",
                plan.plan_id.as_str(),
                plan.source_document_id.as_str(),
                plan.checked_form_id.as_str(),
                plan.expanded_form_id.as_str(),
                plan.fragment_count,
                plan.placement_count,
                plan.connection_count
            ),
        )?;
    }
    for fragment in &report.fragments {
        push_line(
            lines,
            format!(
                "    FRAGMENT {} plan={} host={} boot={}",
                fragment.fragment_id.as_str(),
                fragment.plan_id.as_str(),
                fragment.host_id.as_str(),
                fragment.boot_id.as_str()
            ),
        )?;
    }
    for region in &report.execution_regions {
        push_line(
            lines,
            format!(
                "    REGION {} plan={} fragment={} profile={} scheduling={:?} lanes={} lane-pool={} lane-class={} lane-units={} lane-base={} runtime-memory={} timer-slots={} cord-items={} cord-bytes={} sign-items={} sign-bytes={}",
                region.region_id.as_str(), region.plan_id.as_str(), region.fragment_id.as_str(),
                region.execution_profile_id.as_str(), region.scheduling, region.lane_count,
                region.lane_resource.pool_id.as_str(), region.lane_resource.class_id.as_str(),
                region.lane_resource.units, region.lane_base_id.as_str(),
                region.requirements.runtime_memory_bytes, region.requirements.timer_slots,
                region.requirements.cord_item_capacity, region.requirements.cord_byte_capacity,
                region.requirements.mandatory_sign_items, region.requirements.mandatory_sign_bytes
            ),
        )?;
    }
    for placement in &report.placements {
        push_line(
            lines,
            format!(
                "    PLACEMENT {} plan={} host={} boot={} capability={} kind={} contract={} profile={} implementation={} artifact={} host-operations={:?} resources={:?}",
                placement.placement_id.as_str(),
                placement.plan_id.as_str(),
                placement.host_id.as_str(),
                placement.boot_id.as_str(),
                placement.capability_id.as_str(),
                placement.kind_id.as_str(),
                placement.kind_contract_revision.as_str(),
                placement.execution_profile_id.as_str(),
                placement.implementation_id.as_str(),
                placement.artifact_id.as_str(),
                placement.host_operations,
                placement.resources
            ),
        )?;
        if let Some(device) = super::topology_hosts::current_device_for_placement(report, placement)
        {
            push_line(lines, format!(
                "      SELECTED DEVICE {} generation={} identity={:?} provider={} resources={:?} (current association; not proof of a physical effect)",
                device.device_id.as_str(), placement.offer_generation.0,
                device.identity_evidence.strength, device.identity_evidence.provider.as_str(),
                device.resources
            ))?;
        } else {
            push_line(
                lines,
                format!(
                    "      SELECTED DEVICE not identified at sealed offer generation {}",
                    placement.offer_generation.0
                ),
            )?;
        }
    }
    for connection in &report.connections {
        push_line(
            lines,
            format!(
                "    CORD {} plan={} source={} sink={} info={} items={} bytes={}",
                connection.connection_id.as_str(),
                connection.plan_id.as_str(),
                connection.source_placement_id.as_str(),
                connection.sink_placement_id.as_str(),
                connection.value_kind.as_str(),
                connection.item_capacity,
                connection.byte_capacity
            ),
        )?;
    }
    push_line(lines, format!("PLAYS {}", report.plays.len()))?;
    for play in &report.plays {
        push_line(
            lines,
            format!(
                "  PLAY {} plan={} host={} boot={} lifecycle={:?} terminal={:?}",
                play.active_play_id.as_str(),
                play.plan_id.as_str(),
                play.host_id.as_str(),
                play.boot_id.as_str(),
                play.lifecycle,
                play.terminal_disposition
            ),
        )?;
        for placement in &play.placements {
            push_line(
                lines,
                format!(
                    "    PLAY PLACEMENT {} lifecycle={:?} terminal={:?}",
                    placement.placement_id.as_str(),
                    placement.lifecycle,
                    placement.terminal_disposition
                ),
            )?;
        }
        for connection in &play.connections {
            push_line(
                lines,
                format!(
                    "    PLAY CORD {} lifecycle={:?} pressure={:?}",
                    connection.connection_id.as_str(),
                    connection.lifecycle,
                    connection.pressure
                ),
            )?;
        }
    }
    Ok(())
}

fn render_lines(
    lines: &mut Vec<String>,
    report: &ObservatoryReport,
) -> Result<(), TopologyViewError> {
    let mut links = report.lines.iter().collect::<Vec<_>>();
    links.sort_by(|left, right| {
        left.offer
            .binding
            .binding_id
            .cmp(&right.offer.binding.binding_id)
    });
    push_line(lines, format!("LINES {}", links.len()))?;
    for link in links {
        push_line(
            lines,
            format!(
                "  LINE {} {}@{} > {}@{} base={} instance={} report={:?} availability={:?}",
                link.offer.binding.binding_id.as_str(),
                link.offer.binding.source.host_id.as_str(),
                link.offer.binding.source.boot_id.as_str(),
                link.offer.binding.sink.host_id.as_str(),
                link.offer.binding.sink.boot_id.as_str(),
                link.offer.binding.base.as_str(),
                link.offer.binding.base_instance_id.as_str(),
                link.state,
                link.offer.availability.availability
            ),
        )?;
    }
    Ok(())
}

fn render_signs(
    lines: &mut Vec<String>,
    report: &ObservatoryReport,
) -> Result<(), TopologyViewError> {
    push_line(
        lines,
        format!(
            "SIGNS {} retained={} capacity={} visible_gaps={}",
            report.signs.len(),
            report.retention.retained_items,
            report.retention.item_capacity,
            report.retention.visible_gap_count
        ),
    )?;
    for sign in &report.signs {
        push_line(
            lines,
            format!(
                "  SIGN {} history={} host={} boot={} kind={:?}",
                sign.sign_id.as_str(),
                if sign.historical {
                    "historical"
                } else {
                    "current"
                },
                sign.host_id.as_str(),
                sign.boot_id.as_str(),
                sign.kind
            ),
        )?;
    }
    Ok(())
}

fn render_provenance(
    lines: &mut Vec<String>,
    report: &ObservatoryReport,
) -> Result<(), TopologyViewError> {
    push_line(
        lines,
        format!(
            "BOOT PROVENANCE [SEALED] {}",
            report.sealed_boot_provenance.len()
        ),
    )?;
    for provenance in &report.sealed_boot_provenance {
        push_line(
            lines,
            format!(
                "  SEALED host={} boot={} firmware={} adapter={} version={} revision={} image={} build={} profile={} memory-regions={} arena-bytes={} artifacts={} framebuffers={} proof={:?}",
                provenance.host_id.as_str(),
                provenance.boot_id.as_str(),
                provenance.firmware_environment,
                provenance.adapter_name,
                provenance.adapter_version,
                provenance.adapter_revision,
                provenance.image_id.as_str(),
                provenance.build_id.as_str(),
                provenance
                    .image_build_trace
                    .as_ref()
                    .map_or("none", |trace| trace.profile_id.as_str()),
                provenance.memory_map.normalized_region_count,
                provenance.memory_map.runtime_arena_bytes,
                provenance.boot_artifacts.len(),
                provenance.framebuffers.len(),
                provenance.proof_class
            ),
        )?;
    }
    Ok(())
}

fn topology_line_upper_bound(report: &ObservatoryReport) -> usize {
    7usize
        .saturating_add(report.hosts.len())
        .saturating_add(report.capabilities.len())
        .saturating_add(report.capabilities.len())
        .saturating_add(report.devices.len())
        .saturating_add(report.bases.len())
        .saturating_add(report.plans.len())
        .saturating_add(report.fragments.len())
        .saturating_add(report.placements.len())
        .saturating_add(report.placements.len())
        .saturating_add(report.connections.len())
        .saturating_add(report.plays.len())
        .saturating_add(report.lines.len())
        .saturating_add(report.signs.len())
        .saturating_add(report.sealed_boot_provenance.len())
        .saturating_add(
            report
                .hosts
                .iter()
                .map(|host| host.planner_capabilities.len() + host.resources.len())
                .sum::<usize>(),
        )
        .saturating_add(
            report
                .plays
                .iter()
                .map(|play| play.placements.len() + play.connections.len())
                .sum::<usize>(),
        )
}

pub(crate) fn push_line(lines: &mut Vec<String>, line: String) -> Result<(), TopologyViewError> {
    if lines.len() == MAX_TOPOLOGY_LINES {
        return Err(TopologyViewError::PresentationTooLarge);
    }
    lines.push(line);
    Ok(())
}

#[cfg(test)]
#[path = "topology_tests.rs"]
mod tests;
