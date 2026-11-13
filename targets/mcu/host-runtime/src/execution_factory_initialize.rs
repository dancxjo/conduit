//! In-place cold construction and all-or-empty publication of software owners.
use super::*;

pub(super) fn validate<const N: usize>(
    slots: &[Option<SoftwareBack<'_>>; N],
    identities: &[Option<OtherNodeIdentity>; N],
) -> Result<(), &'static str> {
    for index in 0..N {
        if slots[index].is_some() != identities[index].is_some()
            || identities[index].is_some_and(|id| usize::from(id.node) != index)
        {
            return Err("software-inventory");
        }
    }
    Ok(())
}

// The destination cannot be borrowed by another observer during construction.
// Guarded slot writes own every partial Back; no second inventory is created.
struct Pending<'a, 'f, const N: usize> {
    inventory: &'a mut SoftwareInventory<'f, N>,
    committed: bool,
}
impl<const N: usize> Drop for Pending<'_, '_, N> {
    fn drop(&mut self) {
        if !self.committed {
            for slot in &mut self.inventory.slots {
                *slot = None;
            }
            self.inventory.identities.fill(None);
            self.inventory.configuration = &[];
            self.inventory.backing = 0;
        }
    }
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
        let mut result = Self::empty();
        // SAFETY: forward the caller's unchanged checked Source, recording and
        // exclusive allocator-measurement obligations to the same initializer.
        unsafe {
            result.initialize_into(
                definitions,
                identities,
                publisher,
                sequence,
                now,
                allocated_bytes,
                configuration,
                recording,
            )?;
        }
        Ok(result)
    }

    /// An inert destination with no initialized Backs or advertised identities.
    /// Construct once in final caller storage before in-place preparation.
    pub const fn empty() -> Self {
        Self {
            slots: [const { None }; N],
            identities: [None; N],
            backing: 0,
            configuration: &[],
        }
    }

    /// Construct into an empty retained owner without returning the whole
    /// inventory. Occupied destinations are unchanged. Every later refusal
    /// drops all newly constructed Backs and restores the empty destination.
    /// Identity/configuration/backing facts become visible only after success.
    ///
    /// # Safety
    /// The same independently checked Source/identity/configuration pairing,
    /// exclusive allocator measurement and original admitted recording-scope
    /// obligations as `initialize_with_recording_scope` apply unchanged.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn initialize_into(
        &mut self,
        definitions: [Option<SoftwareDefinition<'_>>; N],
        identities: [Option<OtherNodeIdentity>; N],
        publisher: &'f dyn crate::execution_software::PublicationTarget,
        sequence: conduit_environment_runtime::publication::PublicationSequence,
        now: crate::execution_software::ReadMonotonicMillis,
        allocated_bytes: fn() -> usize,
        configuration: &'static [SoftwareConfigurationPin],
        recording: Option<crate::execution_software::RecordingLogScope<'_>>,
    ) -> Result<(), &'static str> {
        if self.slots.iter().any(Option::is_some)
            || self.identities.iter().any(Option::is_some)
            || self.backing != 0
            || !self.configuration.is_empty()
        {
            return Err("software-inventory-occupied");
        }
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
        let mut definitions = definitions.into_iter();
        let mut identities = identities.into_iter();
        // SAFETY: the two adapters consume each corresponding Source array
        // element once, preserving the original initialization obligations.
        unsafe {
            self.initialize_by(
                |_| definitions.next().ok_or("software-definition-index"),
                |_| identities.next().ok_or("software-identity-index"),
                publisher,
                sequence,
                now,
                allocated_bytes,
                configuration,
                recording,
            )
        }
    }

    /// Construct from one independently checked Source ordinal at a time,
    /// without N-sized definition or identity staging arrays. On success each
    /// callback is invoked exactly once per index in increasing order. An error
    /// stops immediately, drops every newly built Back, and restores empty.
    /// An occupied destination invokes neither callback. Multiple recording
    /// owners are refused before any completed inventory becomes visible.
    ///
    /// # Safety
    /// Callbacks must select the SAME independently installed checked Source
    /// definitions/identities/configuration and original recording scope as the
    /// array APIs. Their work and all constructors share the unchanged exclusive
    /// allocator-measurement and caller-funded backing/preparation obligations.
    #[allow(clippy::too_many_arguments)]
    pub unsafe fn initialize_by<'d>(
        &mut self,
        mut definitions: impl FnMut(usize) -> Result<Option<SoftwareDefinition<'d>>, &'static str>,
        mut identities: impl FnMut(usize) -> Result<Option<OtherNodeIdentity>, &'static str>,
        publisher: &'f dyn crate::execution_software::PublicationTarget,
        sequence: conduit_environment_runtime::publication::PublicationSequence,
        now: crate::execution_software::ReadMonotonicMillis,
        allocated_bytes: fn() -> usize,
        configuration: &'static [SoftwareConfigurationPin],
        recording: Option<crate::execution_software::RecordingLogScope<'_>>,
    ) -> Result<(), &'static str> {
        if self.slots.iter().any(Option::is_some)
            || self.identities.iter().any(Option::is_some)
            || self.backing != 0
            || !self.configuration.is_empty()
        {
            return Err("software-inventory-occupied");
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

        let mut pending = Pending {
            inventory: self,
            committed: false,
        };
        let mut recording_owners = 0usize;
        for index in 0..N {
            let definition = definitions(index)?;
            if matches!(definition, Some(SoftwareDefinition::RecordingLog { .. })) {
                recording_owners += 1;
                if recording_owners > 1 {
                    return Err("recording-scope-multiple-owners");
                }
            }
            pending.inventory.slots[index] = match definition {
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
            let identity = identities(index)?;
            if pending.inventory.slots[index].is_some() != identity.is_some()
                || identity.is_some_and(|id| usize::from(id.node) != index)
            {
                return Err("software-inventory");
            }
            pending.inventory.identities[index] = identity;
        }
        let backing = allocated_bytes()
            .checked_sub(before)
            .ok_or("allocator-measurement")?;
        pending.inventory.configuration = configuration;
        pending.inventory.backing = backing;
        pending.committed = true;
        Ok(())
    }
}

#[cfg(test)]
#[path = "execution_factory_initialize_tests.rs"]
mod tests;

#[cfg(test)]
#[path = "execution_factory_initialize_by_tests.rs"]
mod by_tests;
