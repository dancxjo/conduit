//! Initialized software retained before planning, with no native possession.
use super::*;

#[derive(PartialEq, Eq)]
pub(super) struct Construction {
    pub(super) roles: ResidentRoles,
    pub(super) computations: [c::AssignedComputationRequirement; NODES - 8],
    pub(super) count: u64,
    pub(super) period_ms: u64,
    layout_digest: [u8; 32],
}
impl Construction {
    pub(super) fn prepare(
        resident: &ResidentAppliance<'_>,
        count: u64,
        period_ms: u64,
    ) -> Result<Self, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
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
        let maximum_records: usize = resident.layout.counts.iter().map(|n| usize::from(*n)).sum();
        if count > conduit_time::MAX_TICK_COUNT
            || resident.layout.records.is_empty()
            || resident.layout.records.len() > maximum_records
            || resident.layout.records.len() > crate::assigned::MAXIMUM_RESIDENT_RECORDS
        {
            return Err(E::Configuration);
        }
        let computations = resident
            .computations
            .requirements
            .try_into()
            .map_err(|_| E::Roles)?;
        // Fixed-width hash chaining pins the complete canonical resident inventory
        // without retaining a second copy of its records in MCU heap storage.
        let mut layout_digest =
            c::assigned_plan_payload_digest(b"conduit.environment/resident-software@1");
        // Layout admission treats records as a set. Preserve that meaning when
        // fingerprinting, using bounded selection instead of an allocated sort.
        let mut previous = None;
        for _ in 0..resident.layout.records.len() {
            let key = resident
                .layout
                .records
                .iter()
                .map(|r| (r.tag, r.payload_digest))
                .filter(|key| previous.is_none_or(|p| *key > p))
                .min()
                .ok_or(E::Configuration)?;
            previous = Some(key);
            let mut bytes = [0; 65];
            bytes[..32].copy_from_slice(&layout_digest);
            bytes[32] = key.0;
            bytes[33..].copy_from_slice(&key.1);
            layout_digest = c::assigned_plan_payload_digest(&bytes);
        }
        let mut counts = [0; 32 + c::ASSIGNED_PLAN_COUNT_KINDS];
        counts[..32].copy_from_slice(&layout_digest);
        counts[32..].copy_from_slice(&resident.layout.counts);
        layout_digest = c::assigned_plan_payload_digest(&counts);
        Ok(Self {
            roles,
            computations,
            count,
            period_ms,
            layout_digest,
        })
    }
}

/// Owns initialized pure Backs, routing placeholders and value storage before
/// planning. It grants no hardware authority, installs no kernel and starts no
/// Body. Root must separately admit the complete memory/work closure.
pub struct PreparedResidentSoftware<V> {
    pub(super) construction: Construction,
    drivers: [ApplianceBack; NODES],
    selectors: [Option<PreparedCaptureSelector>; NODES],
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
        let construction = Construction::prepare(resident, cadence.count, cadence.period_ms)?;
        validate_texts(&construction, texts)?;
        Self::initialize_with(
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
        if slot
            .as_ref()
            .ok_or(NativePreparationRefusal::Initialization)?
            .construction
            != selected.construction
        {
            return Err(NativePreparationRefusal::Configuration);
        }
        Ok(slot
            .take()
            .expect("preflighted resident software")
            .into_assigned(selected.tables))
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
            drivers: self.drivers,
            selectors: self.selectors,
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
        mut values: V,
    ) -> Result<Self, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        let Construction {
            roles,
            computations,
            count,
            period_ms,
            ..
        } = &construction;
        let (roles, computations, count, period_ms) = (*roles, computations, *count, *period_ms);
        let maximum = u32::try_from(count).map_err(|_| E::Configuration)?;
        let mut drivers: [Option<ApplianceBack>; NODES] = core::array::from_fn(|_| None);
        let mut selectors = core::array::from_fn(|_| None);
        let wait = values
            .store(&c::encode_monotonic_duration(period_ms))
            .map_err(E::Storage)?;
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
        drivers[usize::from(roles.tick.0)] = Some(ApplianceBack::Tick(
            conduit_time::TickBack::counted(wait, count).ok_or(E::Configuration)?,
        ));
        drivers[usize::from(roles.pulse.0)] = Some(ApplianceBack::Native(CaptureBack::pulse(1024)));
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
                ApplianceBack::publication(
                    &PublicationContract::prepare(channel).map_err(init)?,
                    maximum,
                    order.clone(),
                )
                .map_err(init)?,
            );
        }
        if drivers.iter().any(Option::is_none) {
            return Err(E::Roles);
        }
        Ok(Self {
            construction,
            drivers: drivers.map(|d| d.expect("complete resident role closure")),
            selectors,
            values,
        })
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
