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
#[derive(Clone, Copy, Debug)]
pub struct ResidentRoles {
    pub pulse: k::NodeId,
    pub tick: k::NodeId,
    pub trigger: k::NodeId,
    pub observation: k::NodeId,
    pub publications: [k::NodeId; 4],
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
    pub fn prepare(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
        mut values: V,
    ) -> Result<Self, NativePreparationRefusal> {
        use NativePreparationRefusal as E;
        // Full structural/configuration/computation validation precedes software
        // initialization. Root supplies current Host/Boot and required effects.
        let tables = NativeTables::prepare(
            bytes,
            maxima,
            required,
            resident.computations,
            &resident.layout,
        )
        .map_err(E::Tables)?;
        let mut drivers: [Option<ApplianceBack>; NODES] = core::array::from_fn(|_| None);
        let mut selectors = core::array::from_fn(|_| None);
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
        let maximum = u32::try_from(count).map_err(|_| E::Configuration)?;
        let wait = values
            .store(&c::encode_monotonic_duration(period_ms))
            .map_err(E::Storage)?;
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
        let program_key = c::AssignedIdentity::from_text("program");
        let selector_key = c::AssignedIdentity::from_text("selector");
        for record in c::AssignedPlanRecords::new(bytes).map_err(|_| E::Configuration)? {
            let record = record.map_err(|_| E::Configuration)?;
            if record.tag != c::ASSIGNED_CONFIGURATION {
                continue;
            }
            let data = record.payload;
            let node = usize::from(u16::from_le_bytes(
                data[..2].try_into().map_err(|_| E::Configuration)?,
            ));
            let Some(computation) = resident.computations.requirements.iter().find(|r| {
                usize::from(r.node) == node && data.get(2..18) == Some(r.key.0.as_slice())
            }) else {
                continue;
            };
            if drivers[node].is_some() || data.get(18) != Some(&3) {
                return Err(E::Configuration);
            }
            let text = core::str::from_utf8(data.get(19..).ok_or(E::Configuration)?)
                .map_err(|_| E::Configuration)?;
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
        if drivers.iter().any(Option::is_none) {
            return Err(E::Roles);
        }
        Ok(Self {
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
