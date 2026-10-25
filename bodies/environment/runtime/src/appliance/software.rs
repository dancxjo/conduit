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

#[path = "software_offers.rs"]
mod offers;
pub use offers::ReadySoftwareOffers;
#[cfg(feature = "display-output")]
#[path = "software_assigned.rs"]
mod assigned_output;
pub const PROFILE: &str = "conduit.environment/prepared-software@1";
pub const ARTIFACT: &str = "conduit-environment-runtime/appliance-software@1";
const BYTES: u32 = 1024;
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SoftwareRefusal {
    Source,
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
pub(super) struct ReadyOperation {
    gear: c::GearId,
    pub(super) configuration: Vec<c::ConfigurationEntry>,
    pub(super) offer: c::CapabilityOffer,
    pub(super) prepared: Option<PreparedSoftwareOperation>,
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
    metadata_released: bool,
}
impl ReadySoftwareBacks {
    pub fn prepare(
        source: &ApplianceSource,
        host: c::HostId,
        boot: c::BootId,
    ) -> Result<Self, SoftwareRefusal> {
        let cadence = source
            .checked_cadence()
            .map_err(|_| SoftwareRefusal::Source)?;
        let count = u32::try_from(cadence.count).map_err(|_| SoftwareRefusal::Count)?;
        let sequence = PublicationSequence::prepare(cadence.count);
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
            operations.push(prepare_operation(gear, count, &sequence)?);
        }
        Ok(Self {
            host,
            boot,
            source: source.expanded.source_document_id.clone(),
            checked: source.expanded.checked_plot_id.clone(),
            expanded: source.expanded.expanded_plot_id.clone(),
            generation: None,
            operations,
            metadata_released: false,
        })
    }
    /// Initialize only the explicitly named portable computations from checked
    /// Source. Other gears require independently retained providers. This does
    /// not establish readiness of the whole Plot or grant permission to play.
    pub fn prepare_portable(
        source: &conduit_plot::ExpandedCanonicalPlot,
        gears: &[c::GearId],
        host: c::HostId,
        boot: c::BootId,
    ) -> Result<Self, SoftwareRefusal> {
        source
            .validate_expansion()
            .map_err(|_| SoftwareRefusal::Source)?;
        let mut operations = Vec::new();
        for id in gears {
            if operations.iter().any(|op: &ReadyOperation| &op.gear == id) {
                return Err(SoftwareRefusal::Selection);
            }
            let gear = source
                .gears
                .iter()
                .find(|gear| &gear.gear_id == id)
                .ok_or(SoftwareRefusal::Selection)?;
            operations.push(ready_operation(gear, initialize_portable(gear)?)?);
        }
        Ok(Self {
            host,
            boot,
            source: source.source_document_id.clone(),
            checked: source.checked_plot_id.clone(),
            expanded: source.expanded_plot_id.clone(),
            generation: None,
            operations,
            metadata_released: false,
        })
    }
    /// Initialize portable computations from independently authenticated Make
    /// material, decoding one gear at a time instead of retaining a rich graph.
    /// At most eight portable operations may be prepared through this entrance.
    ///
    /// # Safety
    /// Root must authenticate the complete material and its Source identities
    /// against an independently trusted build artifact. Received metadata,
    /// claimed identities or successful decoding do not establish that origin.
    /// No native readiness, authority, Body membership or Play is created here.
    pub unsafe fn prepare_portable_make(
        source: c::PlotIdentity,
        gears: impl IntoIterator<Item = Result<conduit_plot::CheckedGear, SoftwareRefusal>>,
        host: c::HostId,
        boot: c::BootId,
    ) -> Result<Self, SoftwareRefusal> {
        let mut operations: Vec<ReadyOperation> = Vec::new();
        for gear in gears {
            if operations.len() >= 8 {
                return Err(SoftwareRefusal::Count);
            }
            let gear = gear?;
            if operations.iter().any(|op| op.gear == gear.gear_id) {
                return Err(SoftwareRefusal::Selection);
            }
            operations.push(ready_operation(&gear, initialize_portable(&gear)?)?);
        }
        Ok(Self {
            host,
            boot,
            source: source.source_document_id,
            checked: source.checked_plot_id,
            expanded: source.expanded_plot_id,
            generation: None,
            operations,
            metadata_released: false,
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
        let index = self.selected_index(fragment, placement)?;
        self.operations[index]
            .prepared
            .take()
            .ok_or(SoftwareRefusal::AlreadySelected)
    }
    /// Retire construction descriptors only after every initialized operation
    /// has transferred. Scope and withdrawn-offer inspection remain available;
    /// this owner can never supply another operation. Refusal changes nothing.
    /// No allocation, replacement implementation or readiness is introduced.
    pub fn release_transferred_metadata(&mut self) -> Result<(), SoftwareRefusal> {
        if self.operations.iter().any(|op| op.prepared.is_some()) {
            return Err(SoftwareRefusal::Selection);
        }
        self.operations = Vec::new();
        self.metadata_released = true;
        Ok(())
    }
    /// Preflight a whole closure before transferring any retained operation.
    pub fn check_selected(
        &self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<(), SoftwareRefusal> {
        self.selected_index(fragment, placement).map(|_| ())
    }
    fn selected_index(
        &self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<usize, SoftwareRefusal> {
        if !c::verify_plan_fragment(fragment)
            || fragment.host_id != self.host
            || fragment.boot_id != self.boot
            || fragment.source_document_id != self.source
            || fragment.checked_plot_id != self.checked
            || fragment.expanded_plot_id != self.expanded
        {
            return Err(SoftwareRefusal::Selection);
        }
        if self.metadata_released {
            return Err(SoftwareRefusal::AlreadySelected);
        }
        let gear = fragment
            .placements
            .iter()
            .find(|gear| &gear.placement_id == placement)
            .ok_or(SoftwareRefusal::Selection)?;
        let index = self
            .operations
            .iter()
            .position(|op| op.gear == gear.gear_id)
            .ok_or(SoftwareRefusal::Selection)?;
        let op = &self.operations[index];
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
        if op.prepared.is_none() {
            return Err(SoftwareRefusal::AlreadySelected);
        }
        Ok(index)
    }
}
/// Make-side metadata from the ordinary portable implementation constructor.
/// No Host/Boot, native provider, advertisement or readiness is manufactured.
/// Temporary pure owners validate the compiled artifact and are then released.
pub fn compile_offer_templates(
    source: &ApplianceSource,
) -> Result<Vec<(c::GearId, c::CapabilityOffer)>, SoftwareRefusal> {
    let count = u32::try_from(source.count).map_err(|_| SoftwareRefusal::Count)?;
    let sequence = PublicationSequence::prepare(source.count);
    source
        .expanded
        .gears
        .iter()
        .filter(|gear| {
            ![
                conduit_pulse_capture::PULSE_READ_KIND,
                conduit_time::TICK_KIND,
            ]
            .contains(&gear.kind_id.as_str())
        })
        .map(|gear| {
            let operation = prepare_operation(gear, count, &sequence)?;
            Ok((operation.gear, operation.offer))
        })
        .collect()
}
fn prepare_operation(
    gear: &conduit_plot::CheckedGear,
    count: u32,
    sequence: &PublicationSequence,
) -> Result<ReadyOperation, SoftwareRefusal> {
    ready_operation(gear, initialize(gear, count, sequence)?)
}
type InitializedOperation = (
    c::Kind,
    PreparedSoftwareOperation,
    Vec<c::HostCallRequirement>,
);
pub(super) fn ready_operation(
    gear: &conduit_plot::CheckedGear,
    initialized: InitializedOperation,
) -> Result<ReadyOperation, SoftwareRefusal> {
    let (mut kind, prepared, calls) = initialized;
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
    Ok(ReadyOperation {
        gear: gear.gear_id.clone(),
        configuration: gear.configuration.clone(),
        offer,
        prepared: Some(prepared),
    })
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
    } else {
        return initialize_portable(gear);
    };
    Ok((
        kind,
        PreparedSoftwareOperation {
            driver,
            selector: None,
        },
        vec![],
    ))
}
pub(super) fn initialize_portable(
    gear: &conduit_plot::CheckedGear,
) -> Result<InitializedOperation, SoftwareRefusal> {
    let mut selector = None;
    let mut calls = vec![];
    let temporal = c::PortTemporal::Flow { closes: true };
    let (kind, driver) = if let Some(c::ConfigurationValue::Text(encoded)) = gear
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

/// Make-side descriptor from the same validated portable constructor. This
/// creates no Host, native provider, reservation or authority.
pub fn compile_portable_offer(
    gear: &conduit_plot::CheckedGear,
) -> Result<c::CapabilityOffer, SoftwareRefusal> {
    Ok(ready_operation(gear, initialize_portable(gear)?)?.offer)
}
