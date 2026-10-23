//! Constrained native preparation from independently pinned resident layout and
//! computations. No rich Source graph, Plan, offer catalog or authority is made.
use super::ApplianceBack;
use crate::{
    assigned::*, observation::ObservationContract, publication::*, CaptureBack,
    PreparedCaptureSelector,
};
use conduit_core as c;
use conduit_kernel::{self as k, ValueStorage};

pub const NODES: usize = 15;
pub const CORDS: usize = 14;
pub const PORTS: usize = 16;
pub type NativeTables = AssignedKernelTables<NODES, CORDS, PORTS, { NODES * PORTS }, CORDS, NODES>;
/// Trusted Make facts tied to this resident artifact, never inferred from the
/// incoming assignment. Native pulse/wait providers remain separately owned.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentRoles {
    pub pulse: k::NodeId,
    pub tick: k::NodeId,
    pub trigger: k::NodeId,
    pub observation: k::NodeId,
    pub publications: [k::NodeId; 4],
}
impl ResidentRoles {
    /// Compiler-side association of checked Make nodes. Root must retain the
    /// result independently; incoming node labels cannot establish this trust.
    pub fn from_nodes<'a>(
        nodes: impl IntoIterator<Item = (u16, &'a str)>,
    ) -> Result<Self, NativePreparationRefusal> {
        let kinds = [
            conduit_pulse_capture::PULSE_READ_KIND,
            conduit_time::TICK_KIND,
            conduit_time::TICK_TRIGGER_KIND,
            crate::observation::KIND,
            PublicationChannel::Reading.kind_id(),
            PublicationChannel::CaptureTimeout.kind_id(),
            PublicationChannel::ProviderFailure.kind_id(),
            PublicationChannel::GeometryRefusal.kind_id(),
        ];
        let mut found = [None; 8];
        let mut seen = [false; NODES];
        for (node, kind) in nodes {
            let previous = seen
                .get_mut(usize::from(node))
                .ok_or(NativePreparationRefusal::Roles)?;
            if *previous {
                return Err(NativePreparationRefusal::Roles);
            }
            *previous = true;
            if let Some(index) = kinds.iter().position(|expected| *expected == kind) {
                if found[index].is_some() {
                    return Err(NativePreparationRefusal::Roles);
                }
                found[index] = Some(k::NodeId(node));
            }
        }
        if seen.contains(&false) || found.contains(&None) {
            return Err(NativePreparationRefusal::Roles);
        }
        let nodes = found.map(|node| node.expect("complete role closure"));
        Ok(Self {
            pulse: nodes[0],
            tick: nodes[1],
            trigger: nodes[2],
            observation: nodes[3],
            publications: [nodes[4], nodes[5], nodes[6], nodes[7]],
        })
    }
}
pub struct ResidentAppliance<'a> {
    pub layout: ResidentLayout<'a>,
    pub computations: c::AssignedComputationProfile<'a>,
    pub roles: ResidentRoles,
}
#[derive(Debug, PartialEq, Eq)]
pub enum NativePreparationRefusal {
    Tables(AssignedKernelRefusal),
    Roles,
    Configuration,
    Initialization,
    Storage(k::StorageError),
}
/// Caller must still admit the complete live storage/work closure, bind actual
/// native Host Call possession and correlate the real Body start before Play.
pub struct PreparedAssignedAppliance<V> {
    pub tables: NativeTables,
    pub drivers: [ApplianceBack; NODES],
    pub selectors: [Option<PreparedCaptureSelector>; NODES],
    pub values: V,
    pub roles: ResidentRoles,
    pub count: u64,
    pub period_ms: u64,
}
impl<V: ValueStorage> PreparedAssignedAppliance<V> {
    /// Convenience path retaining assignment bytes through initialization.
    /// Constrained callers can use `ValidatedAssignedAppliance` to release them.
    pub fn prepare(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
        values: V,
    ) -> Result<Self, NativePreparationRefusal> {
        ValidatedAssignedAppliance::admit(bytes, maxima, required, resident)?
            .initialize_with(|computation| computation_text(bytes, computation), values)
    }
}
/// Independently provisioned firmware data, not a received assignment or grant.
#[derive(Clone, Copy)]
pub struct ResidentComputation<'a> {
    pub node: u16,
    pub key: c::AssignedIdentity,
    pub text: &'a str,
}
/// Owns validated current tables and Source cadence; borrows no assignment bytes.
/// Admission is not native possession, Body start, authority or memory admission.
pub struct ValidatedAssignedAppliance {
    tables: NativeTables,
    roles: ResidentRoles,
    computations: [c::AssignedComputationRequirement; NODES - 8],
    count: u64,
    period_ms: u64,
}
impl ValidatedAssignedAppliance {
    pub fn admit(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
    ) -> Result<Self, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        let tables = NativeTables::prepare(
            bytes,
            maxima,
            required,
            resident.computations,
            &resident.layout,
        )
        .map_err(E::Tables)?;
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
        for computation in resident.computations.requirements {
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
        validate_roles(bytes, roles)?;
        let count = numeric(bytes, roles.tick, "count")?;
        let period_ms = numeric(bytes, roles.tick, "period-ms")?;
        u32::try_from(count).map_err(|_| E::Configuration)?;
        let computations = resident
            .computations
            .requirements
            .try_into()
            .map_err(|_| E::Roles)?;
        Ok(Self {
            tables,
            roles,
            computations,
            count,
            period_ms,
        })
    }
    /// Root may drop the received assignment before entering this allocating
    /// phase. Every supplied resident text must match the independently pinned
    /// inventory admitted above. No missing, extra or duplicate computation is
    /// accepted, and no software is initialized until that check is complete.
    pub fn initialize<V: ValueStorage>(
        self,
        resident: &[ResidentComputation<'_>],
        values: V,
    ) -> Result<PreparedAssignedAppliance<V>, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        if resident.len() != self.computations.len() {
            return Err(E::Configuration);
        }
        for expected in &self.computations {
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
            if c::AssignedComputationRequirement::from_text(text.node, key, text.text)
                .map_err(|_| E::Configuration)?
                != *expected
            {
                return Err(E::Configuration);
            }
        }
        self.initialize_with(
            |expected| {
                resident
                    .iter()
                    .find(|text| text.node == expected.node && text.key == expected.key)
                    .map(|text| text.text)
                    .ok_or(E::Configuration)
            },
            values,
        )
    }
    fn initialize_with<'a, V: ValueStorage>(
        self,
        text: impl Fn(&c::AssignedComputationRequirement) -> Result<&'a str, NativePreparationRefusal>,
        mut values: V,
    ) -> Result<PreparedAssignedAppliance<V>, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        let Self {
            tables,
            roles,
            computations,
            count,
            period_ms,
        } = self;
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
            let text = text(computation)?;
            let driver = if computation.key == program_key {
                let program = conduit_plot::PortableExpressionProgram::from_canonical_hex(text)
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
        Ok(PreparedAssignedAppliance {
            tables,
            drivers: drivers.map(|d| d.expect("complete resident role closure")),
            selectors,
            values,
            roles,
            count,
            period_ms,
        })
    }
}
fn numeric(bytes: &[u8], node: k::NodeId, key: &str) -> Result<u64, NativePreparationRefusal> {
    let key = c::AssignedIdentity::from_text(key);
    let mut result = None;
    for record in
        c::AssignedPlanRecords::new(bytes).map_err(|_| NativePreparationRefusal::Configuration)?
    {
        let record = record.map_err(|_| NativePreparationRefusal::Configuration)?;
        let data = record.payload;
        if record.tag == c::ASSIGNED_CONFIGURATION
            && data.get(..2) == Some(node.0.to_le_bytes().as_slice())
            && data.get(2..18) == Some(key.0.as_slice())
        {
            if result.is_some() || data.get(18) != Some(&2) {
                return Err(NativePreparationRefusal::Configuration);
            }
            result = Some(u64::from_le_bytes(
                data.get(19..27)
                    .ok_or(NativePreparationRefusal::Configuration)?
                    .try_into()
                    .map_err(|_| NativePreparationRefusal::Configuration)?,
            ));
        }
    }
    result.ok_or(NativePreparationRefusal::Configuration)
}
fn init<T>(_: T) -> NativePreparationRefusal {
    NativePreparationRefusal::Initialization
}

fn validate_roles(bytes: &[u8], roles: ResidentRoles) -> Result<(), NativePreparationRefusal> {
    let fixed = [
        (roles.pulse, conduit_pulse_capture::PULSE_READ_KIND),
        (roles.tick, conduit_time::TICK_KIND),
        (roles.trigger, conduit_time::TICK_TRIGGER_KIND),
        (roles.observation, crate::observation::KIND),
    ];
    for record in
        c::AssignedPlanRecords::new(bytes).map_err(|_| NativePreparationRefusal::Configuration)?
    {
        let record = record.map_err(|_| NativePreparationRefusal::Configuration)?;
        if record.tag != c::ASSIGNED_NODE {
            continue;
        }
        let data = record.payload;
        for (node, kind) in fixed.into_iter().chain(
            roles
                .publications
                .into_iter()
                .zip(PublicationChannel::ALL.map(|c| c.kind_id())),
        ) {
            if data.get(..2) == Some(node.0.to_le_bytes().as_slice())
                && data.get(4..20) != Some(c::AssignedIdentity::from_text(kind).0.as_slice())
            {
                return Err(NativePreparationRefusal::Roles);
            }
        }
    }
    Ok(())
}

fn computation_text<'a>(
    bytes: &'a [u8],
    computation: &c::AssignedComputationRequirement,
) -> Result<&'a str, NativePreparationRefusal> {
    let mut result = None;
    for record in
        c::AssignedPlanRecords::new(bytes).map_err(|_| NativePreparationRefusal::Configuration)?
    {
        let record = record.map_err(|_| NativePreparationRefusal::Configuration)?;
        let data = record.payload;
        if record.tag == c::ASSIGNED_CONFIGURATION
            && data.get(..2) == Some(computation.node.to_le_bytes().as_slice())
            && data.get(2..18) == Some(computation.key.0.as_slice())
        {
            if result.is_some() || data.get(18) != Some(&3) {
                return Err(NativePreparationRefusal::Configuration);
            }
            result = Some(
                core::str::from_utf8(
                    data.get(19..)
                        .ok_or(NativePreparationRefusal::Configuration)?,
                )
                .map_err(|_| NativePreparationRefusal::Configuration)?,
            );
        }
    }
    result.ok_or(NativePreparationRefusal::Configuration)
}
