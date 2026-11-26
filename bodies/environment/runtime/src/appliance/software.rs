//! Initialized software Backs retained before advertisement and ordinary planning.
//! Pulse capture and native waiting need independently initialized owners; this
//! set neither supplies them nor grants their authority.
use crate::{
    appliance::ApplianceBack,
    construction::ApplianceSource,
    publication::{PublicationChannel, PublicationContract, PublicationSequence},
    CaptureBack,
};
use alloc::{format, vec, vec::Vec};
use conduit_core as c;
use conduit_dht11_observation::ObservationContract;
use conduit_semantic_catalog::PreparedCanonicalSelector;

pub type ReadySoftwareOffers<'a> =
    conduit_semantic_catalog::ReadySoftwareOffers<'a, PreparedSoftwareOperation>;
pub const PROFILE: &str = "conduit.environment/prepared-software@1";
pub const ARTIFACT: &str = "conduit-environment-runtime/appliance-software@1";
const BYTES: u32 = 1024;
pub use conduit_semantic_catalog::SoftwareRefusal;
pub struct PreparedSoftwareOperation {
    pub driver: ApplianceBack,
    pub selector: Option<PreparedCanonicalSelector>,
}
type ReadyOperation = conduit_semantic_catalog::ReadyOperation<PreparedSoftwareOperation>;
pub struct ReadySoftwareBacks {
    inner: conduit_semantic_catalog::ReadyBacks<PreparedSoftwareOperation>,
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
            inner: conduit_semantic_catalog::ReadyBacks::new(
                c::PlotIdentity {
                    source_document_id: source.expanded.source_document_id.clone(),
                    checked_plot_id: source.expanded.checked_plot_id.clone(),
                    expanded_plot_id: source.expanded.expanded_plot_id.clone(),
                },
                host,
                boot,
                operations,
            ),
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
            inner: conduit_semantic_catalog::ReadyBacks::new(
                c::PlotIdentity {
                    source_document_id: source.source_document_id.clone(),
                    checked_plot_id: source.checked_plot_id.clone(),
                    expanded_plot_id: source.expanded_plot_id.clone(),
                },
                host,
                boot,
                operations,
            ),
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
            inner: conduit_semantic_catalog::ReadyBacks::new(
                c::PlotIdentity {
                    source_document_id: source.source_document_id,
                    checked_plot_id: source.checked_plot_id,
                    expanded_plot_id: source.expanded_plot_id,
                },
                host,
                boot,
                operations,
            ),
        })
    }
    pub fn append_to_advertisement(
        &mut self,
        host: &mut c::HostAdvertisement,
    ) -> Result<(), SoftwareRefusal> {
        self.inner.append_to_advertisement(host)
    }
    pub fn take_selected(
        &mut self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<PreparedSoftwareOperation, SoftwareRefusal> {
        self.inner.take_selected(fragment, placement)
    }
    pub fn check_selected(
        &self,
        fragment: &c::PlanFragment,
        placement: &c::PlacementId,
    ) -> Result<(), SoftwareRefusal> {
        self.inner.check_selected(fragment, placement)
    }
    pub fn release_transferred_metadata(&mut self) -> Result<(), SoftwareRefusal> {
        self.inner.release_transferred_metadata()
    }
    pub fn current_offers(
        &mut self,
        host: &c::HostId,
        boot: &c::BootId,
        generation: c::OfferGeneration,
    ) -> Result<ReadySoftwareOffers<'_>, SoftwareRefusal> {
        self.inner.current_offers(host, boot, generation)
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
fn ready_operation(
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
    } else if gear.kind_id.as_str() == conduit_dht11_observation::KIND {
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
fn initialize_portable(
    gear: &conduit_plot::CheckedGear,
) -> Result<InitializedOperation, SoftwareRefusal> {
    use conduit_semantic_catalog::{PortableOperationBack, PreparedPortableOperation};
    let prepared = PreparedPortableOperation::prepare(
        gear,
        c::PortTemporal::Flow { closes: true },
        BYTES,
        BYTES,
    )
    .map_err(|reason| match reason {
        conduit_semantic_catalog::PortableOperationRefusal::Unsupported => {
            SoftwareRefusal::Unsupported
        }
        conduit_semantic_catalog::PortableOperationRefusal::Initialization => {
            SoftwareRefusal::Initialization
        }
    })?;
    let driver = match prepared.back {
        PortableOperationBack::Expression(back) => CaptureBack::Expression(back),
        PortableOperationBack::Selector(back) => CaptureBack::Selector(back),
    };
    Ok((
        prepared.kind,
        PreparedSoftwareOperation {
            driver: ApplianceBack::Native(driver),
            selector: prepared.selector,
        },
        prepared.host_calls,
    ))
}

fn init<T>(_: T) -> SoftwareRefusal {
    SoftwareRefusal::Initialization
}
