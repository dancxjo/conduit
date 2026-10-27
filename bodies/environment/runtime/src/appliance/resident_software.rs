//! Initialized software retained before planning, with no native possession.
use super::*;
use alloc::{boxed::Box, vec::Vec};
#[cfg(feature = "local-planning")]
#[path = "resident_local.rs"]
mod local;
#[path = "resident_transfer.rs"]
mod transfer;
#[cfg(feature = "local-planning")]
pub use local::{PreparedDirectLocalAppliance, PreparedLocalAppliance};

#[derive(PartialEq, Eq)]
pub(super) struct Construction {
    pub(super) roles: ResidentRoles,
    pub(super) computations: [c::AssignedComputationRequirement; NODES - 8],
    pub(super) count: u64,
    pub(super) period_ms: u64,
    pub(super) source: ResidentSourceIdentity,
}
impl Construction {
    pub(super) fn prepare(
        resident: &ResidentSoftwareSpec<'_>,
        count: u64,
        period_ms: u64,
    ) -> Result<Self, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        if (count, period_ms) != (resident.cadence.count, resident.cadence.period_ms) {
            return Err(E::Configuration);
        }
        let roles = resident.roles;
        let mut occupied = [false; NODES];
        for node in [roles.pulse, roles.tick, roles.trigger, roles.observation]
            .into_iter()
            .chain(roles.publications)
        {
            let used = occupied.get_mut(usize::from(node.0)).ok_or(E::Roles)?;
            if *used {
                return Err(E::Roles);
            }
            *used = true;
        }
        if resident.computations.requirements.len() != NODES - 8 {
            return Err(E::Roles);
        }
        let mut total_text = 0_u32;
        for computation in resident.computations.requirements {
            if computation.text_bytes > resident.computations.maximum_text_bytes {
                return Err(E::Configuration);
            }
            total_text = total_text
                .checked_add(u32::from(computation.text_bytes))
                .ok_or(E::Configuration)?;
            if total_text > resident.computations.maximum_total_text_bytes {
                return Err(E::Configuration);
            }
            let used = occupied
                .get_mut(usize::from(computation.node))
                .ok_or(E::Roles)?;
            if *used {
                return Err(E::Roles);
            }
            *used = true;
        }
        if occupied.contains(&false) {
            return Err(E::Roles);
        }
        if count > conduit_time::MAX_TICK_COUNT {
            return Err(E::Configuration);
        }
        let computations = resident
            .computations
            .requirements
            .try_into()
            .map_err(|_| E::Roles)?;
        Ok(Self {
            roles,
            computations,
            count,
            period_ms,
            source: resident.source,
        })
    }
}

/// Owns initialized pure Backs, routing placeholders and value storage before
/// planning. It grants no hardware authority, installs no kernel and starts no
/// Body. Root must separately admit the complete memory/work closure.
pub struct PreparedResidentSoftware<V> {
    pub(super) construction: Construction,
    drivers: Box<[Option<ApplianceBack>; NODES]>,
    selectors: Box<[Option<PreparedCaptureSelector>; NODES]>,
    values: V,
}
impl<V: ValueStorage> PreparedResidentSoftware<V> {
    /// All expectations come from trusted Make construction data, independently
    /// of incoming assignments. No rich Source graph or Plan is required here.
    pub fn prepare(
        resident: &ResidentAppliance<'_>,
        texts: &[ResidentComputation<'_>],
        cadence: &conduit_time::TickConfiguration,
        values: V,
    ) -> Result<Self, NativePreparationRefusal> {
        if *cadence != resident.cadence {
            return Err(NativePreparationRefusal::Configuration);
        }
        Self::prepare_source(&resident.software_spec(), texts, values)
    }
    /// Initialize from independently checked Source constraints without a Plan
    /// or routing layout. Assignment admission still checks its exact graph.
    pub fn prepare_source(
        resident: &ResidentSoftwareSpec<'_>,
        texts: &[ResidentComputation<'_>],
        values: V,
    ) -> Result<Self, NativePreparationRefusal> {
        Self::prepare_source_retained(resident, texts, values).map_err(|(error, _)| error)
    }
    /// Prepare fresh finite Backs while retaining the actual value store on every
    /// refusal. Existing slot generations and live references are never reset.
    /// This does not renew a Play or reattach any native provider.
    pub fn prepare_source_retained(
        resident: &ResidentSoftwareSpec<'_>,
        texts: &[ResidentComputation<'_>],
        values: V,
    ) -> Result<Self, (NativePreparationRefusal, V)> {
        let checked = (|| {
            let cadence = &resident.cadence;
            let construction = Construction::prepare(resident, cadence.count, cadence.period_ms)?;
            validate_texts(&construction, texts)?;
            Ok(construction)
        })();
        let construction = match checked {
            Ok(construction) => construction,
            Err(error) => return Err((error, values)),
        };
        Self::initialize_with_retained(
            construction,
            |expected| {
                texts
                    .iter()
                    .find(|text| text.node == expected.node && text.key == expected.key)
                    .map(|text| (text.text, text.canonical_bytes))
                    .ok_or(NativePreparationRefusal::Configuration)
            },
            values,
        )
    }
    /// Preflight before transfer. Refusal leaves initialized ownership retained.
    /// Native binding, current Body start and authority remain Root obligations.
    pub fn bind_retained(
        slot: &mut Option<Self>,
        selected: ValidatedAssignedAppliance,
    ) -> Result<PreparedAssignedAppliance<V>, NativePreparationRefusal> {
        slot.as_ref()
            .ok_or(NativePreparationRefusal::Initialization)?
            .check_assignment(&selected)?;
        Ok(slot
            .take()
            .expect("preflighted resident software")
            .into_assigned(selected.tables))
    }
    fn check_assignment(
        &self,
        selected: &ValidatedAssignedAppliance,
    ) -> Result<(), NativePreparationRefusal> {
        if self.construction != selected.construction {
            return Err(NativePreparationRefusal::Configuration);
        }
        if self.drivers.iter().any(Option::is_none) {
            return Err(NativePreparationRefusal::Initialization);
        }
        Ok(())
    }
    pub(super) fn into_assigned(self, tables: NativeTables) -> PreparedAssignedAppliance<V> {
        let Construction {
            roles,
            count,
            period_ms,
            ..
        } = self.construction;
        PreparedAssignedAppliance {
            tables,
            drivers: (*self.drivers).map(|d| d.expect("complete resident role closure")),
            selectors: *self.selectors,
            values: self.values,
            roles,
            count,
            period_ms,
        }
    }
    pub(super) fn initialize_with<'a>(
        construction: Construction,
        text: impl Fn(
            &c::AssignedComputationRequirement,
        ) -> Result<(&'a str, Option<&'a [u8]>), NativePreparationRefusal>,
        values: V,
    ) -> Result<Self, NativePreparationRefusal> {
        Self::initialize_with_retained(construction, text, values).map_err(|(error, _)| error)
    }
    fn initialize_with_retained<'a>(
        construction: Construction,
        text: impl Fn(
            &c::AssignedComputationRequirement,
        ) -> Result<(&'a str, Option<&'a [u8]>), NativePreparationRefusal>,
        mut values: V,
    ) -> Result<Self, (NativePreparationRefusal, V)> {
        let parts = (|| {
            use NativePreparationRefusal as E;
            let Construction {
                roles,
                computations,
                count,
                period_ms,
                ..
            } = &construction;
            let (roles, computations, count, period_ms) =
                (*roles, computations, *count, *period_ms);
            let maximum = u32::try_from(count).map_err(|_| E::Configuration)?;
            // Fixed preparation backing lives in the admitted heap, not beneath the
            // recursive program/type decoder on a constrained target's stack.
            let mut drivers = empty_slots::<ApplianceBack, NODES>();
            let mut selectors = empty_slots::<PreparedCaptureSelector, NODES>();
            let program_key = c::AssignedIdentity::from_text("program");
            let selector_key = c::AssignedIdentity::from_text("selector");
            // Prepare the largest temporary evaluator first, while other retained
            // codecs/publications do not yet occupy the admission arena. This is
            // preparation ordering only; runtime order remains the admitted tables.
            let mut initialized = [false; NODES];
            for _ in 0..computations.len() {
                let computation = computations
                    .iter()
                    .filter(|r| !initialized[usize::from(r.node)])
                    .max_by_key(|r| r.text_bytes)
                    .ok_or(E::Roles)?;
                let node = usize::from(computation.node);
                initialized[node] = true;
                let (text, canonical_bytes) = text(computation)?;
                let driver = if computation.key == program_key {
                    let program = match canonical_bytes {
                        Some(bytes) => {
                            conduit_plot::PortableExpressionProgram::from_canonical_bytes(bytes)
                        }
                        None => conduit_plot::PortableExpressionProgram::from_canonical_hex(text),
                    }
                    .map_err(init)?;
                    CaptureBack::expression(&program, c::PortTemporal::Flow { closes: true })
                        .map_err(init)?
                } else if computation.key == selector_key {
                    let selector = c::StructuredSelector::from_canonical_hex(text).map_err(init)?;
                    selectors[node] =
                        Some(PreparedCaptureSelector::prepare(selector, 1024, 1024).map_err(init)?);
                    CaptureBack::selector(1024)
                } else {
                    return Err(E::Configuration);
                };
                drivers[node] = Some(ApplianceBack::Native(driver));
            }
            drivers[usize::from(roles.pulse.0)] =
                Some(ApplianceBack::Native(CaptureBack::pulse(1024)));
            drivers[usize::from(roles.trigger.0)] = Some(ApplianceBack::Trigger(
                conduit_time::TickTriggerBack::default(),
            ));
            drivers[usize::from(roles.observation.0)] = Some(ApplianceBack::Native(
                CaptureBack::observation(&ObservationContract::prepare().map_err(init)?)
                    .map_err(init)?,
            ));
            let order = PublicationSequence::prepare(count);
            for (node, channel) in roles.publications.into_iter().zip(PublicationChannel::ALL) {
                drivers[usize::from(node.0)] = Some(
                    ApplianceBack::source_publication(
                        &PublicationContract::prepare(channel).map_err(init)?,
                        maximum,
                        order.clone(),
                    )
                    .map_err(init)?,
                );
            }
            if drivers
                .iter()
                .enumerate()
                .any(|(node, driver)| node != usize::from(roles.tick.0) && driver.is_none())
            {
                return Err(E::Roles);
            }
            // Allocate the only stored value after all fallible codec preparation.
            // Codec refusals leave storage untouched; storage refusal returns
            // its actual state. No wait reference is orphaned and no generation
            // is reset during a retry.
            let wait = values
                .store(&c::encode_monotonic_duration(period_ms))
                .map_err(E::Storage)?;
            drivers[usize::from(roles.tick.0)] = Some(ApplianceBack::Tick(
                conduit_time::TickBack::counted(wait, count)
                    .expect("Construction checked the finite tick count"),
            ));
            Ok((drivers, selectors))
        })();
        match parts {
            Ok((drivers, selectors)) => Ok(Self {
                construction,
                drivers,
                selectors,
                values,
            }),
            Err(error) => Err((error, values)),
        }
    }
}

pub(super) fn validate_texts(
    construction: &Construction,
    resident: &[ResidentComputation<'_>],
) -> Result<(), NativePreparationRefusal> {
    use NativePreparationRefusal as E;
    if resident.len() != construction.computations.len() {
        return Err(E::Configuration);
    }
    for expected in &construction.computations {
        let mut matching = resident
            .iter()
            .filter(|text| text.node == expected.node && text.key == expected.key);
        let text = matching.next().ok_or(E::Configuration)?;
        if matching.next().is_some() {
            return Err(E::Configuration);
        }
        let key = if expected.key == c::AssignedIdentity::from_text("program") {
            "program"
        } else if expected.key == c::AssignedIdentity::from_text("selector") {
            "selector"
        } else {
            return Err(E::Configuration);
        };
        if text.text.len() != usize::from(expected.text_bytes) || !resident_bytes_match(text) {
            return Err(E::Configuration);
        }
        if c::AssignedComputationRequirement::from_text(text.node, key, text.text)
            .map_err(|_| E::Configuration)?
            != *expected
        {
            return Err(E::Configuration);
        }
    }
    Ok(())
}

/// Build exactly N empty slots without a stack-sized array temporary.
fn empty_slots<T, const N: usize>() -> Box<[Option<T>; N]> {
    let slots: Vec<Option<T>> = (0..N).map(|_| None).collect();
    match slots.into_boxed_slice().try_into() {
        Ok(slots) => slots,
        Err(_) => unreachable!("fixed resident slot count"),
    }
}

#[cfg(test)]
mod retained_preparation_tests {
    use super::*;
    use conduit_kernel::{FixedValueStore, StorageError};

    #[test]
    fn codec_refusal_returns_same_live_store_without_allocating_a_wait() {
        let requirements = core::array::from_fn::<_, { NODES - 8 }, _>(|index| {
            c::AssignedComputationRequirement::from_text((index + 8) as u16, "program", "00")
                .unwrap()
        });
        let spec = ResidentSoftwareSpec {
            cadence: conduit_time::TickConfiguration {
                count: 960,
                period_ms: 60_000,
            },
            source: ResidentSourceIdentity {
                expanded_plot_digest: [0; 32],
            },
            computations: c::AssignedComputationProfile {
                requirements: &requirements,
                maximum_text_bytes: 2,
                maximum_total_text_bytes: 14,
            },
            roles: ResidentRoles {
                pulse: k::NodeId(0),
                tick: k::NodeId(1),
                trigger: k::NodeId(2),
                observation: k::NodeId(3),
                publications: [k::NodeId(4), k::NodeId(5), k::NodeId(6), k::NodeId(7)],
            },
        };
        let texts = core::array::from_fn::<_, { NODES - 8 }, _>(|index| ResidentComputation {
            node: (index + 8) as u16,
            key: c::AssignedIdentity::from_text("program"),
            text: "00",
            canonical_bytes: None,
        });
        let mut values = FixedValueStore::<2, 64>::new(128).unwrap();
        let stale = values.store(b"old").unwrap();
        values.release(stale).unwrap();
        let live = values.store(b"retained").unwrap();
        for _ in 0..3 {
            values = match PreparedResidentSoftware::prepare_source_retained(&spec, &texts, values)
            {
                Ok(_) => panic!("malformed program must refuse"),
                Err((error, values)) => {
                    assert_eq!(error, NativePreparationRefusal::Initialization);
                    assert_eq!(values.get(live).unwrap(), b"retained");
                    assert_eq!(values.get(stale), Err(StorageError::StaleReference));
                    assert_eq!(values.used_items(), 1);
                    assert_eq!(values.used_bytes(), 8);
                    values
                }
            };
        }
        let next = values.store(b"free slot").unwrap();
        assert_eq!(
            next.generation, 1,
            "refused preparation never used the spare slot"
        );
    }
}
