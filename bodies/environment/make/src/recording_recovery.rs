//! One exact-generation read/recovery workload over independently ready offers.
//! No store, software provider, authority, Host, Boot or Body is synthesized here.
use conduit_core as c;
use conduit_environment_runtime::recording as r;
use conduit_plan_lowering::lowering::{LoweredPlanFragment, LoweringError};
use std::collections::BTreeMap;
mod batch;
pub use batch::RecordingBatchRecoverySource;

pub struct RecordingRecoverySource {
    expanded: conduit_plot::ExpandedCanonicalPlot,
    reference: c::BoundedResourceRef,
}
impl RecordingRecoverySource {
    /// The immutable reference names semantic content, not a residence or grant.
    pub fn compile(reference: &c::BoundedResourceRef) -> Result<Self, String> {
        reference
            .validate()
            .map_err(|e| format!("reference: {e:?}"))?;
        if reference.content_profile.as_str() != "text/json-utf8@1"
            || reference.access_class.as_str() != "resource/snapshot@1"
            || reference.extent.items != Some(1)
            || reference.extent.bytes == 0
            || reference.extent.bytes > r::MAXIMUM_CHUNK_BYTES as u64
        {
            return Err("recording snapshot profile".into());
        }
        let encoded = reference
            .encode()
            .map_err(|e| format!("reference: {e:?}"))?;
        let hex: String = encoded.iter().map(|b| format!("{b:02x}")).collect();
        let (startup, profiles) = catalogs()?;
        let source = format!(
            "{}\nplot recorder {{\n recovery: environment-recording-recovery(\"{hex}\")\n}}\n",
            include_str!("../../profiles/recovery.conduit")
        );
        let expanded = checked_source(&source, &startup, &profiles)?;
        Ok(Self {
            expanded,
            reference: reference.clone(),
        })
    }
    pub fn expanded(&self) -> &conduit_plot::ExpandedCanonicalPlot {
        &self.expanded
    }
    pub fn reference(&self) -> &c::BoundedResourceRef {
        &self.reference
    }
}
#[derive(Debug)]
pub enum RecordingPlanningRefusal {
    Placement(conduit_planner::PlannerError),
    Planning(conduit_planner::PlannerError),
    ForeignFragment,
    ReferenceBinding,
    Lowering(LoweringError),
}
pub struct PreparedRecordingRecovery {
    plan: c::Plan,
    lowered: LoweredPlanFragment,
}
impl PreparedRecordingRecovery {
    /// Caller retains actual software/store owners and provides current resource
    /// residence and independent authority. A reference cannot supply either.
    pub fn prepare(
        source: &RecordingRecoverySource,
        host: &c::HostAdvertisement,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<Self, RecordingPlanningRefusal> {
        let expanded = source.expanded();
        let hosts = core::slice::from_ref(host);
        let placements = conduit_planner::default_expanded_placements(expanded, hosts)
            .map_err(RecordingPlanningRefusal::Placement)?;
        Self::prepare_selected(
            expanded,
            core::slice::from_ref(source.reference()),
            host,
            &placements,
            bases,
            authority,
        )
    }
    /// Root supplies exact current choices from its retained owners. This uses
    /// the ordinary Planner and does not infer availability from descriptors.
    pub fn prepare_batch(
        source: &RecordingBatchRecoverySource,
        host: &c::HostAdvertisement,
        placements: &conduit_planner::PlacementChoices,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<Self, RecordingPlanningRefusal> {
        Self::prepare_selected(
            source.expanded(),
            source.references(),
            host,
            placements,
            bases,
            authority,
        )
    }
    fn prepare_selected(
        expanded: &conduit_plot::ExpandedCanonicalPlot,
        references: &[c::BoundedResourceRef],
        host: &c::HostAdvertisement,
        placements: &conduit_planner::PlacementChoices,
        bases: &[c::BaseImplementationId],
        authority: &[c::AuthorityGrant],
    ) -> Result<Self, RecordingPlanningRefusal> {
        use RecordingPlanningRefusal as E;
        let hosts = core::slice::from_ref(host);
        let connection_limits = expanded
            .connections
            .iter()
            .filter(|cord| {
                expanded
                    .gears
                    .iter()
                    .find(|gear| gear.gear_id == cord.source_gear_id)
                    .and_then(|gear| {
                        gear.outputs
                            .iter()
                            .find(|port| port.port_id == cord.source_port_id)
                    })
                    .is_some_and(|port| port.value_kind.as_str() == "text/json-utf8@1")
            })
            .map(|cord| {
                (
                    (
                        cord.source_gear_id.clone(),
                        cord.source_port_id.clone(),
                        cord.sink_gear_id.clone(),
                        cord.sink_port_id.clone(),
                    ),
                    conduit_planner::ConnectionQueueLimits {
                        item_capacity: 1,
                        byte_capacity: r::MAXIMUM_CHUNK_BYTES as u32,
                    },
                )
            })
            .collect();
        let plan = conduit_planner::plan_expanded_canonical_with_connection_limits(
            expanded,
            hosts,
            placements,
            bases,
            conduit_planner::PlanningOptions {
                connection_bases: &BTreeMap::new(),
                line_candidates: &BTreeMap::new(),
                connection_item_capacity: 1,
                connection_byte_capacity: c::MAXIMUM_RESOURCE_REFERENCE_ENCODED_BYTES as u32,
                authority_grants: authority,
                protected_resource_grants: &[],
                line_offers: &[],
            },
            &connection_limits,
        )
        .map_err(E::Planning)?;
        let [fragment] = plan.fragments.as_slice() else {
            return Err(E::ForeignFragment);
        };
        if fragment.host_id != host.host_id || fragment.boot_id != host.boot_id {
            return Err(E::ForeignFragment);
        }
        let reads: Vec<_> = fragment
            .placements
            .iter()
            .filter(|gear| gear.kind_id.as_str() == conduit_semantic_catalog::SNAPSHOT_READ_KIND)
            .collect();
        if reads.len() != references.len() {
            return Err(E::ReferenceBinding);
        }
        for reference in references {
            let hex: String = reference
                .encode()
                .map_err(|_| E::ReferenceBinding)?
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect();
            let mut matching = reads.iter().filter(|gear| {
                gear.configuration.iter().any(|entry| {
                    entry.key == "reference"
                        && entry.value == c::ConfigurationValue::Text(hex.clone())
                })
            });
            let read = matching.next().ok_or(E::ReferenceBinding)?;
            if matching.next().is_some() {
                return Err(E::ReferenceBinding);
            }
            let [binding] = read.resources.as_slice() else {
                return Err(E::ReferenceBinding);
            };
            binding
                .content
                .as_ref()
                .ok_or(E::ReferenceBinding)?
                .contract
                .accepts_reference(reference)
                .map_err(|_| E::ReferenceBinding)?;
        }
        let lowered =
            conduit_plan_lowering::lowering::lower_plan_fragment(fragment).map_err(E::Lowering)?;
        Ok(Self { plan, lowered })
    }
    pub fn plan(&self) -> &c::Plan {
        &self.plan
    }
    pub fn lowered(&self) -> &LoweredPlanFragment {
        &self.lowered
    }
    /// Preparation material for the ordinary Body owner, never a start receipt.
    pub fn body_plot_plan(&self) -> conduit_body::BodyPlotPlan {
        conduit_body::BodyPlotPlan {
            plot: conduit_body::ResidentPlot::new(
                self.plan.source_document_id.clone(),
                self.plan.checked_plot_id.clone(),
            ),
            plan: self.plan.clone(),
        }
    }
}

fn catalogs() -> Result<(conduit_plot::StartupCatalog, conduit_plot::ProfileCatalog), String> {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    r::install_generation_value_kind(&mut startup, &mut profiles)?;
    r::install_recovery_kind(&mut startup, &mut profiles)?;
    let singleton = r::recording_singleton_kind();
    startup.insert(conduit_plot::KindSignature {
        kind: singleton.kind_id.as_str().into(),
        startup_parameters: vec![],
    })?;
    startup.insert_fore(singleton.kind_id.as_str(), singleton.checked_front())?;
    profiles
        .insert_kind(singleton)
        .map_err(|e| format!("{e:?}"))?;
    conduit_semantic_catalog::install_resource_snapshot_catalogs(&mut startup, &mut profiles)?;
    let read = conduit_semantic_catalog::resource_snapshot_semantic_contract(false);
    startup.insert_fore(read.kind_id.as_str(), read.checked_front())?;
    Ok((startup, profiles))
}

fn checked_source(
    source: &str,
    startup: &conduit_plot::StartupCatalog,
    profiles: &conduit_plot::ProfileCatalog,
) -> Result<conduit_plot::ExpandedCanonicalPlot, String> {
    let syntax = conduit_plot::parse_syntax_document(source);
    if !syntax.diagnostics.is_empty() {
        return Err(format!("recording Source: {:?}", syntax.diagnostics));
    }
    let checked =
        conduit_plot::check_syntax_document(&syntax, startup).map_err(|e| format!("{e:?}"))?;
    let expanded = conduit_plot::expand_canonical_plot(&checked, "recorder", profiles)
        .map_err(|e| format!("{e:?}"))?;
    Ok(expanded)
}
