//! Concrete composition of real initialized resident/software/native Back loans.
//! Source/assignment checking happens before this inventory is lent; binding
//! consumes every selected slot once, without allocating or starting equipment.
#[path = "execution_factory_residents.rs"]
mod residents;
pub use residents::{PreparedResidentEntry, PreparedResidentSubset, PreparedResidents};

#[path = "execution_factory_appender.rs"]
mod appender;
pub use appender::{
    CallsWithAppender, CallsWithOptionalAppender, PreparedWithAppender,
    PreparedWithOptionalAppender,
};

#[path = "execution_factory_service.rs"]
mod service;
pub use service::{BoxedRecordServiceBack, CallsWithRecordService, PreparedWithRecordService};

#[path = "execution_factory_drivers.rs"]
mod drivers;

#[path = "execution_factory_native.rs"]
mod native;
pub use native::{NativeCallNode, PreparedCallSet, PreparedNativeDrivers};
#[path = "execution_factory_i2c_clock.rs"]
mod i2c_clock;
pub use i2c_clock::{I2cClockCalls, PreparedI2cClockCalls};

extern crate alloc;
use crate::{execution::PreparedBacks, execution_software::SoftwareBack};
use conduit_kernel::{
    scheduler::{HostCallBack, StepBack, StepInputBytes, StepIo, StepOutcome},
    PortId,
};
use conduit_monotonic_wait::{MonotonicWaitHostCall, MonotonicWaitProvider, PreparedAssignedWait};
use conduit_plan_lowering::resident_inventory::OtherNodeIdentity;
use conduit_pulse_capture::{
    installation::PreparedAssignedPulse,
    owner::{PulseHostCall, PulseReadProvider},
};
use conduit_tm1637_wire::{
    installation::PreparedAssignedWrite, owner::WriteHostCall, OpenDrain, WriteBack,
};

#[path = "execution_factory_step.rs"]
mod step;
pub use step::{Driver, NoExtraBack};

pub struct SoftwareConfigurationPin {
    pub node: u16,
    pub record: conduit_core::AssignedRecordRequirement,
}
/// One actual software owner per independently checked Source node.
pub struct SoftwareInventory<'a, const N: usize> {
    slots: [Option<SoftwareBack<'a>>; N],
    identities: [Option<OtherNodeIdentity>; N],
    backing: usize,
    configuration: &'static [SoftwareConfigurationPin],
}
impl<'a, const N: usize> SoftwareInventory<'a, N> {
    /// Root supplies initialized objects, their independently compiled identities
    /// and measured/reachable owned backing. This creates no grant or Plan.
    /// # Safety
    /// Identities must describe these exact implementations/configurations; the
    /// backing charge must cover every retained allocation and excluded aliases.
    pub unsafe fn new(
        slots: [Option<SoftwareBack<'a>>; N],
        identities: [Option<OtherNodeIdentity>; N],
        backing: usize,
        configuration: &'static [SoftwareConfigurationPin],
    ) -> Result<Self, &'static str> {
        for index in 0..N {
            if slots[index].is_some() != identities[index].is_some()
                || identities[index].is_some_and(|id| usize::from(id.node) != index)
            {
                return Err("software-inventory");
            }
        }
        Ok(Self {
            slots,
            identities,
            backing,
            configuration,
        })
    }
    pub fn identities(&self) -> impl Iterator<Item = OtherNodeIdentity> + '_ {
        self.identities.iter().flatten().copied()
    }
    /// Verify immutable Source configuration before lending any software owner.
    /// Core validates the full projection separately; this exact comparison does
    /// not authenticate a command, admit resources or authorize Start.
    pub fn prepare_assigned(
        &mut self,
        projection: &[u8],
        selected: [bool; N],
    ) -> Result<PreparedSoftware<'_, 'a, N>, &'static str> {
        if self.configuration.len() > 256 {
            return Err("software-configuration-bound");
        }
        let mut seen = [false; 256];
        for record in
            conduit_core::AssignedPlanRecords::new(projection).map_err(|_| "software-projection")?
        {
            let record = record.map_err(|_| "software-projection")?;
            if record.tag != conduit_core::ASSIGNED_CONFIGURATION {
                continue;
            }
            let bytes: [u8; 2] = record
                .payload
                .get(..2)
                .ok_or("software-configuration")?
                .try_into()
                .map_err(|_| "software-configuration")?;
            let node = u16::from_le_bytes(bytes);
            let Some(selected) = selected.get(usize::from(node)) else {
                return Err("software-node");
            };
            if !*selected {
                continue;
            }
            let actual = conduit_core::AssignedRecordRequirement::from_record(record);
            let index = self
                .configuration
                .iter()
                .enumerate()
                .find(|(i, pin)| !seen[*i] && pin.node == node && pin.record == actual)
                .map(|(i, _)| i)
                .ok_or("software-configuration")?;
            seen[index] = true;
        }
        if self
            .configuration
            .iter()
            .enumerate()
            .any(|(i, pin)| selected.get(usize::from(pin.node)) == Some(&true) && !seen[i])
        {
            return Err("software-configuration");
        }
        // SAFETY: every selected software record matches its independently
        // linked Source configuration above; owner availability is checked next.
        unsafe { self.prepare(selected) }
    }
    /// # Safety
    /// Caller has independently checked every selected software configuration
    /// against this exact initialized inventory (prefer prepare_assigned).
    pub unsafe fn prepare(
        &mut self,
        selected: [bool; N],
    ) -> Result<PreparedSoftware<'_, 'a, N>, &'static str> {
        if self.selected_recording_is_unbound(&selected) {
            return Err("recording-scope-required");
        }
        for (index, selected) in selected.iter().enumerate() {
            if *selected && self.slots[index].is_none() {
                return Err("software-busy");
            }
        }
        Ok(PreparedSoftware {
            owner: self,
            selected,
        })
    }
}
pub struct PreparedSoftware<'o, 'f, const N: usize> {
    owner: &'o mut SoftwareInventory<'f, N>,
    selected: [bool; N],
}
impl<'f, const N: usize> PreparedSoftware<'_, 'f, N> {
    #[cfg(feature = "execution-workspace")]
    fn workspace_requirements<const P: usize>(
        &self,
    ) -> Option<[Option<conduit_kernel::scheduler::StepWorkspaceRequirements>; N]> {
        let mut actual = [None; N];
        for (index, selected) in self.selected.iter().enumerate() {
            if *selected {
                actual[index] = <SoftwareBack as StepBack<P>>::workspace_requirements(
                    self.owner.slots[index].as_ref()?,
                );
            }
        }
        Some(actual)
    }

    #[inline(always)] // Fuse the affine owner transfer with its final destination.
    fn bind(self) -> [Option<SoftwareBack<'f>>; N] {
        core::array::from_fn(|index| {
            if self.selected[index] {
                self.owner.slots[index].take()
            } else {
                None
            }
        })
    }
}
pub struct PreparedCalls<
    'a,
    P: PulseReadProvider,
    T: OpenDrain,
    W: MonotonicWaitProvider,
    const A: usize,
> {
    pub pulse: Option<PreparedAssignedPulse<'a, P>>,
    pub write: Option<PreparedAssignedWrite<'a, T>>,
    pub waits: [PreparedAssignedWait<'a, W>; A],
}
pub struct Calls<P, T, W, const A: usize> {
    pub pulse: Option<PulseHostCall<P>>,
    pub write: Option<WriteHostCall<T>>,
    pub waits: [MonotonicWaitHostCall<W>; A],
}
/// All mappings below are independently selected immutable Source facts, not
/// role names decoded from incoming metadata. Native ticket binders already
/// checked exact original opaque possession against the same assigned nodes.
pub struct PreparedDrivers<
    'a,
    'o,
    'f,
    R,
    P: PulseReadProvider,
    T: OpenDrain,
    W: MonotonicWaitProvider,
    const E: usize,
    const N: usize,
    const A: usize,
> {
    residents: R,
    software: PreparedSoftware<'o, 'f, N>,
    native: PreparedCalls<'a, P, T, W, A>,
    resident_nodes: [Option<u16>; E],
    pulse_node: Option<u16>,
    write_node: Option<u16>,
    backing: usize,
}
/// Immutable preparation facts emitted by checked Source/Make, never taken from
/// a received assignment. Hardware decoders and packet encoders remain resident
/// expressions; these cases instantiate only the ordinary software providers.
pub enum SoftwareDefinition<'a> {
    /// Flash schema images are decoded only inside measured inventory construction.
    SeededUntilStatic {
        schema: &'static [u8],
        maximum_bytes: u32,
    },
    MergeStatic {
        schema: &'static [u8],
        maximum_bytes: u32,
    },
    ZipStatic {
        left_schema: &'static [u8],
        left_maximum_bytes: u32,
        right_schema: &'static [u8],
        right_maximum_bytes: u32,
        feedback: bool,
    },

    Tick {
        wait: conduit_kernel::ValueRef,
        count: u64,
    },
    Trigger,
    Selector {
        canonical: &'a [u8],
        input_bytes: usize,
        output_bytes: usize,
    },
    SelectorStatic {
        canonical: &'static [u8],
        input_bytes: usize,
        output_bytes: usize,
    },
    Dht11Observation,
    RecordingLog {
        maximum_samples: u32,
    },
    Publication {
        channel: conduit_environment_runtime::publication::PublicationChannel,
        maximum_events: u32,
    },
}
impl<'f, const N: usize> SoftwareInventory<'f, N> {
    /// Construct the actual independently selected providers before publishing
    /// their availability. Refusal drops every partial construction.
    ///
    /// # Safety
    /// Definitions/identities must be the same independently installed checked
    /// Source selection. `allocated_bytes` must report actual exclusively owned
    /// allocator usage without concurrent allocations/deallocations during this
    /// call. Root separately funds borrowed Face/sequence/value-store backing,
    /// temporary preparation peak, allocator overhead and the resident owners.
    pub unsafe fn initialize(
        definitions: [Option<SoftwareDefinition<'_>>; N],
        identities: [Option<OtherNodeIdentity>; N],
        publisher: &'f dyn crate::execution_software::PublicationTarget,
        sequence: conduit_environment_runtime::publication::PublicationSequence,
        now: crate::execution_software::ReadMonotonicMillis,
        allocated_bytes: fn() -> usize,
        configuration: &'static [SoftwareConfigurationPin],
    ) -> Result<Self, &'static str> {
        unsafe {
            Self::initialize_with_recording_scope(
                definitions,
                identities,
                publisher,
                sequence,
                now,
                allocated_bytes,
                configuration,
                None,
            )
        }
    }
    /// Same construction and funding obligations as `initialize`. Any recording
    /// scope must be selected independently from the actual admitted assignment,
    /// before Start. Without scope, the actual prepared owner remains unbound;
    /// selection and execution refuse until opaque approved scope is supplied.
    ///
    /// # Safety
    /// The original `initialize` safety obligations apply unchanged, including
    /// all temporary and retained backing and actual scoped custody.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn initialize_with_recording_scope(
        definitions: [Option<SoftwareDefinition<'_>>; N],
        identities: [Option<OtherNodeIdentity>; N],
        publisher: &'f dyn crate::execution_software::PublicationTarget,
        sequence: conduit_environment_runtime::publication::PublicationSequence,
        now: crate::execution_software::ReadMonotonicMillis,
        allocated_bytes: fn() -> usize,
        configuration: &'static [SoftwareConfigurationPin],
        recording: Option<crate::execution_software::RecordingLogScope<'_>>,
    ) -> Result<Self, &'static str> {
        // One externally selected stream cannot silently become two sequence
        // owners. A multi-recorder workset needs separately selected scopes.
        if definitions
            .iter()
            .filter(|definition| {
                matches!(definition, Some(SoftwareDefinition::RecordingLog { .. }))
            })
            .count()
            > 1
        {
            return Err("recording-scope-multiple-owners");
        }
        let before = allocated_bytes();
        let decode_composite =
            |bytes: &'static [u8], maximum_bytes: u32| -> Result<_, &'static str> {
                let schema = conduit_core::StructuredInfoType::from_canonical_bytes(bytes)
                    .map_err(|_| "composite-type-image")?;
                let contract = conduit_core::CheckedValueContract::new(
                    schema
                        .profile()
                        .map_err(|_| "composite-type-profile")?
                        .value_kind()
                        .clone(),
                    maximum_bytes,
                    alloc::vec![],
                )
                .map_err(|_| "composite-envelope")?;
                Ok((schema, contract))
            };

        let mut slots = core::array::from_fn(|_| None);
        for (index, definition) in definitions.into_iter().enumerate() {
            slots[index] = match definition {
                None => None,
                Some(SoftwareDefinition::SeededUntilStatic {
                    schema,
                    maximum_bytes,
                }) => {
                    let (decoded, contract) = decode_composite(schema, maximum_bytes)?;
                    Some(SoftwareBack::seeded_until_static(
                        &contract, &decoded, schema,
                    )?)
                }
                Some(SoftwareDefinition::MergeStatic {
                    schema,
                    maximum_bytes,
                }) => {
                    let (decoded, _) = decode_composite(schema, maximum_bytes)?;
                    Some(SoftwareBack::merge_static(&decoded, schema, maximum_bytes)?)
                }
                Some(SoftwareDefinition::ZipStatic {
                    left_schema,
                    left_maximum_bytes,
                    right_schema,
                    right_maximum_bytes,
                    feedback,
                }) => {
                    let (left_type, left) = decode_composite(left_schema, left_maximum_bytes)?;
                    let (right_type, right) = decode_composite(right_schema, right_maximum_bytes)?;
                    Some(SoftwareBack::zip(
                        &left, left_type, &right, right_type, feedback,
                    )?)
                }

                Some(SoftwareDefinition::Tick { wait, count }) => {
                    Some(SoftwareBack::tick(wait, count)?)
                }
                Some(SoftwareDefinition::Trigger) => Some(SoftwareBack::trigger()),
                Some(SoftwareDefinition::Selector {
                    canonical,
                    input_bytes,
                    output_bytes,
                }) => Some(SoftwareBack::selector(
                    conduit_core::StructuredSelector::from_canonical_bytes(canonical)
                        .map_err(|_| "selector-definition")?,
                    input_bytes,
                    output_bytes,
                )?),
                Some(SoftwareDefinition::SelectorStatic {
                    canonical,
                    input_bytes,
                    output_bytes,
                }) => Some(SoftwareBack::selector_static(
                    canonical,
                    input_bytes,
                    output_bytes,
                )?),
                Some(SoftwareDefinition::RecordingLog { maximum_samples }) => {
                    let mut owner = SoftwareBack::recording_log_unbound(maximum_samples, now)?;
                    if let Some(scope) = recording {
                        owner.prepare_recording_scope(scope)?;
                    }
                    Some(owner)
                }
                Some(SoftwareDefinition::Dht11Observation) => {
                    Some(SoftwareBack::dht11_observation()?)
                }
                Some(SoftwareDefinition::Publication {
                    channel,
                    maximum_events,
                }) => {
                    let contract =
                        conduit_environment_runtime::publication::PublicationContract::prepare(
                            channel,
                        )
                        .map_err(|_| "publication-contract")?;
                    Some(SoftwareBack::publication(
                        &contract,
                        maximum_events,
                        sequence.clone(),
                        publisher,
                        now,
                    )?)
                }
            };
        }
        let backing = allocated_bytes()
            .checked_sub(before)
            .ok_or("allocator-measurement")?;
        // SAFETY: carries the caller's checked-Source/allocator obligations and
        // retains exactly the newly constructed objects, with no copied runtime.
        unsafe { Self::new(slots, identities, backing, configuration) }
    }
}

impl<const N: usize> SoftwareInventory<'_, N> {
    pub fn needs_recording_scope(&self) -> bool {
        self.slots
            .iter()
            .flatten()
            .any(SoftwareBack::needs_recording_scope)
    }
    pub(crate) fn selected_recording_is_unbound(&self, selected: &[bool; N]) -> bool {
        self.slots.iter().zip(selected).any(|(slot, selected)| {
            *selected
                && slot
                    .as_ref()
                    .is_some_and(SoftwareBack::needs_recording_scope)
        })
    }
    /// Copy scope only from an original opaque admitted fragment, not its public
    /// wire metadata DTO. Retrying the same scope validates without resetting
    /// progress. No recording role means no scope construction or allocation.
    /// The temporary typed Plan/Play IDs are preparation workspace, not retained
    /// backing; the funded encoder's scope binding itself allocates nothing.
    pub fn prepare_recording_from_approved(
        &mut self,
        approved: &conduit_body::DelegatedBodyFragment<'_>,
        actual: &conduit_core::HostAdvertisement,
        stream: [u8; 32],
        first_sequence: u64,
    ) -> Result<(), &'static str> {
        let count = self
            .slots
            .iter()
            .flatten()
            .filter(|slot| matches!(slot, SoftwareBack::RecordingLog(_)))
            .count();
        if count == 0 {
            return Ok(());
        }
        if count != 1 {
            return Err("recording-scope-count");
        }
        let metadata = approved.metadata();
        if metadata.host != actual.host_id.as_str() || metadata.boot != actual.boot_id.as_str() {
            return Err("recording-scope-host");
        }
        let plan: conduit_core::PlanId = metadata.plot_plan.into();
        let play: conduit_core::ActivePlayId = metadata.body_play.into();
        let scope = crate::execution_software::RecordingLogScope {
            context: conduit_environment_runtime::recording::LogContext {
                body: metadata.body,
                producer: conduit_environment_runtime::recording::RecordingContext {
                    host: &actual.host_id,
                    boot: &actual.boot_id,
                    plan: &plan,
                    play: &play,
                },
                stream,
            },
            first_sequence,
        };
        self.slots
            .iter_mut()
            .flatten()
            .find(|slot| matches!(slot, SoftwareBack::RecordingLog(_)))
            .ok_or("recording-owner")?
            .prepare_recording_scope(scope)
    }
}

#[cfg(test)]
#[path = "execution_factory_tests.rs"]
mod tests;
