//! Initialized software Backs retained before advertisement and ordinary planning.
//! Pulse capture and native waiting need independently initialized owners; this
//! set neither supplies them nor grants their authority.
use crate::{
    appliance::ApplianceBack,
    construction::ApplianceSource,
    observation::ObservationContract,
    publication::{PublicationChannel, PublicationContract, PublicationSequence},
    CaptureBack, PreparedCaptureSelector,
};
use alloc::{format, vec, vec::Vec};
use conduit_core as c;

pub const PROFILE: &str = "conduit.environment/prepared-software@1";
pub const ARTIFACT: &str = "conduit-environment-runtime/appliance-software@1";
const BYTES: u32 = 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareRefusal {
    Count,
    Unsupported,
    Contract,
    Initialization,
    Advertisement,
    Selection,
    AlreadySelected,
}
pub struct PreparedSoftwareOperation {
    pub driver: ApplianceBack,
    pub selector: Option<PreparedCaptureSelector>,
}
struct ReadyOperation {
    gear: c::GearId,
    configuration: Vec<c::ConfigurationEntry>,
    offer: c::CapabilityOffer,
    prepared: Option<PreparedSoftwareOperation>,
}
/// All codec/interpreter/publication workspaces are initialized and owned here.
/// Root supplies trusted checked Source and actual Host/Boot; it must separately
/// admit preparation memory, the complete kernel and work before starting Play.
/// This owner is not a Body, a provider for hardware effects or a scheduler.
pub struct ReadySoftwareBacks {
    host: c::HostId,
    boot: c::BootId,
    source: c::SourceDocumentId,
    checked: c::CheckedPlotId,
    expanded: c::ExpandedPlotId,
    generation: Option<c::OfferGeneration>,
    operations: Vec<ReadyOperation>,
}
impl ReadySoftwareBacks {
    pub fn prepare(
        source: &ApplianceSource,
        host: c::HostId,
        boot: c::BootId,
    ) -> Result<Self, SoftwareRefusal> {
        let count = u32::try_from(source.count).map_err(|_| SoftwareRefusal::Count)?;
        let sequence = PublicationSequence::prepare(source.count);
        let mut operations = Vec::new();
        for gear in &source.expanded.gears {
            if [
                conduit_pulse_capture::PULSE_READ_KIND,
                conduit_time::TICK_KIND,
            ]
            .contains(&gear.kind_id.as_str())
            {
                continue;
            }
            let (mut kind, prepared, calls) = initialize(gear, count, &sequence)?;
            if gear.kind_id != kind.kind_id
                || gear.kind_contract_revision != kind.kind_contract_revision
                || gear.startup_parameters != kind.startup_parameters
                || gear.shorthand != kind.shorthand
                || gear.inputs != kind.inputs
                || gear.outputs != kind.outputs
                || gear.semantic_contract != kind.semantic_contract()
            {
                return Err(SoftwareRefusal::Contract);
            }
            kind.limits = c::CapabilityLimits {
                max_active_instances: 1,
                max_queue_items: 1,
                max_queue_bytes: 4096,
            };
            let id = format!("environment/software/{}", gear.gear_id.as_str());
            let offer = c::BackOfferBuilder::new(
                kind,
                c::Back {
                    capability_id: id.as_str().into(),
                    execution_profile_id: PROFILE.into(),
                    implementation_id: id.as_str().into(),
                    artifact_id: ARTIFACT.into(),
                    host_calls: calls,
                    resource_requirements: vec![],
                    authority_requirements: vec![],
                },
            )
            .build();
            operations.push(ReadyOperation {
                gear: gear.gear_id.clone(),
                configuration: gear.configuration.clone(),
                offer,
                prepared: Some(prepared),
            });
        }
        Ok(Self {
            host,
            boot,
            source: source.expanded.source_document_id.clone(),
            checked: source.expanded.checked_plot_id.clone(),
            expanded: source.expanded.expanded_plot_id.clone(),
            generation: None,
            operations,
        })
    }
    /// A refusal leaves the advertisement and retained operations unchanged.
    /// Selected operations are omitted from newly built advertisements. Root
    /// must replace historical snapshots when availability changes.
    pub fn append_to_advertisement(
        &mut self,
        host: &mut c::HostAdvertisement,
    ) -> Result<(), SoftwareRefusal> {
        if host.host_id != self.host
            || host.boot_id != self.boot
            || self
                .operations
                .iter()
                .filter(|op| op.prepared.is_some())
                .any(|op| {
                    host.capabilities
                        .iter()
                        .any(|offer| offer.capability_id == op.offer.capability_id)
                })
        {
            return Err(SoftwareRefusal::Advertisement);
        }
        host.capabilities.extend(
            self.operations
                .iter()
                .filter(|op| op.prepared.is_some())
                .map(|op| op.offer.clone()),
        );
        self.generation = Some(host.offer_generation);
        Ok(())
    }
    /// Match the actual initialized object, not just a Kind descriptor. All
    /// checks precede ownership transfer. Root still owns current-Plan/Play,
    /// memory admission and any independent native Host Call authorization.
    pub fn take_selected(
        &mut self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<PreparedSoftwareOperation, SoftwareRefusal> {
        if !c::verify_plan_fragment(fragment)
            || fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || fragment.source_document_id != self.source
            || fragment.checked_plot_id != self.checked
            || fragment.expanded_plot_id != self.expanded
        {
            return Err(SoftwareRefusal::Selection);
        }
        let gear = fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == placement)
            .ok_or(SoftwareRefusal::Selection)?;
        let op = self
            .operations
            .iter_mut()
            .find(|op| op.gear == gear.gear_id)
            .ok_or(SoftwareRefusal::Selection)?;
        let offer = &op.offer;
        if self.generation != Some(gear.offer_generation)
            || gear.host_id != self.host
            || gear.boot_id != self.boot
            || gear.limits != offer.limits
            || gear.configuration != op.configuration
            || gear.kind_id != offer.kind_id
            || gear.kind_contract_revision != offer.kind_contract_revision
            || gear.semantic_contract != offer.semantic_contract
            || gear.inputs != offer.inputs
            || gear.outputs != offer.outputs
            || gear.capability_id != offer.capability_id
            || gear.execution_profile_id != offer.implementation.execution_profile_id
            || gear.implementation_id != offer.implementation.implementation_id
            || gear.artifact_id != offer.implementation.artifact_id
            || gear.host_calls != offer.host_calls
            || gear.base.is_some()
            || !gear.resources.is_empty()
            || !gear.authority.is_empty()
            || !gear.pool_references.is_empty()
            || !gear.terminal_transductions.is_empty()
            || !gear.realization_properties.is_empty()
            || !gear.realization_characteristics.is_empty()
        {
            return Err(SoftwareRefusal::Selection);
        }
        op.prepared.take().ok_or(SoftwareRefusal::AlreadySelected)
    }
}
fn initialize(
    gear: &conduit_plot::CheckedGear,
    count: u32,
    sequence: &PublicationSequence,
) -> Result<
    (
        c::Kind,
        PreparedSoftwareOperation,
        Vec<c::HostCallRequirement>,
    ),
    SoftwareRefusal,
> {
    let mut selector = None;
    let mut calls = vec![];
    let temporal = c::PortTemporal::Flow { closes: true };
    let (kind, driver) = if let Some(channel) = PublicationChannel::ALL
        .into_iter()
        .find(|channel| channel.kind_id() == gear.kind_id.as_str())
    {
        let contract = PublicationContract::prepare(channel).map_err(init)?;
        (
            contract.kind().clone(),
            ApplianceBack::publication(&contract, count, sequence.clone()).map_err(init)?,
        )
    } else if gear.kind_id.as_str() == conduit_time::TICK_TRIGGER_KIND {
        (
            conduit_time::tick_trigger_semantic_contract(),
            ApplianceBack::Trigger(conduit_time::TickTriggerBack::default()),
        )
    } else if gear.kind_id.as_str() == crate::observation::KIND {
        let contract = ObservationContract::prepare().map_err(init)?;
        (
            contract.kind().clone(),
            ApplianceBack::Native(CaptureBack::observation(&contract).map_err(init)?),
        )
    } else if let Some(c::ConfigurationValue::Text(encoded)) = gear
        .configuration
        .iter()
        .find(|c| c.key == "program")
        .map(|c| &c.value)
    {
        let program =
            conduit_plot::PortableExpressionProgram::from_canonical_hex(encoded).map_err(init)?;
        (
            conduit_semantic_catalog::pure_expression_contract(&program, temporal).map_err(init)?,
            ApplianceBack::Native(CaptureBack::expression(&program, temporal).map_err(init)?),
        )
    } else if let Some(c::ConfigurationValue::Text(encoded)) = gear
        .configuration
        .iter()
        .find(|c| c.key == "selector")
        .map(|c| &c.value)
    {
        let value = c::StructuredSelector::from_canonical_hex(encoded).map_err(init)?;
        let kind = conduit_semantic_catalog::structured_selector_contract(&value, temporal);
        selector = Some(
            PreparedCaptureSelector::prepare(value, BYTES as usize, BYTES as usize)
                .map_err(init)?,
        );
        calls.push(c::HostCallRequirement {
            contract_id: "conduit.host/structured-selector@1".into(),
            target_kind: Some(kind.kind_id.clone()),
            maximum_in_flight: 1,
            maximum_input_bytes: BYTES,
            maximum_output_bytes: BYTES,
        });
        (kind, ApplianceBack::Native(CaptureBack::selector(BYTES)))
    } else {
        return Err(SoftwareRefusal::Unsupported);
    };
    Ok((kind, PreparedSoftwareOperation { driver, selector }, calls))
}

fn init<T>(_: T) -> SoftwareRefusal {
    SoftwareRefusal::Initialization
}
