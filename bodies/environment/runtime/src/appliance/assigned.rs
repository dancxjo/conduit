//! Constrained native preparation from independently pinned resident layout and
//! computations. No rich Source graph, Plan, offer catalog or authority is made.
use super::ApplianceBack;
use crate::{
    assigned::*, observation::ObservationContract, publication::*, CaptureBack,
    PreparedCaptureSelector,
};
use alloc::boxed::Box;
use conduit_core as c;
use conduit_kernel::{self as k, ValueStorage};
use core::mem::MaybeUninit;

#[path = "resident_offers.rs"]
mod resident_offers;
#[path = "resident_software.rs"]
mod resident_software;
pub use resident_offers::{ReadyResidentOffers, ResidentSoftwareOffer};
#[cfg(feature = "local-planning")]
pub use resident_software::{PreparedDirectLocalAppliance, PreparedLocalAppliance};
pub use resident_software::{PreparedResidentSoftware, ResidentExpressionMaterial};

pub const NODES: usize = 15;
pub const CORDS: usize = 14;
// The checked appliance uses one input/output ordinal per node. Expanding its
// Fore requires renewing this profile; incoming extra ports are refused.
pub const PORTS: usize = 1;
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
/// Full-width Make-side representation of the independently checked Source.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct ResidentSourceIdentity {
    pub expanded_plot_digest: [u8; 32],
}
impl ResidentSourceIdentity {
    pub fn from_expanded(id: &c::ExpandedPlotId) -> Self {
        Self {
            expanded_plot_digest: c::assigned_plan_payload_digest(id.as_str().as_bytes()),
        }
    }
}
/// Construction constraints only; contains no routing, Host, Boot or Plan.
#[derive(Clone)]
pub struct ResidentSoftwareSpec<'a> {
    pub cadence: conduit_time::TickConfiguration,
    pub source: ResidentSourceIdentity,
    pub computations: c::AssignedComputationProfile<'a>,
    pub roles: ResidentRoles,
}
pub struct ResidentAppliance<'a> {
    pub cadence: conduit_time::TickConfiguration,
    pub source: ResidentSourceIdentity,
    pub layout: ResidentLayout<'a>,
    pub computations: c::AssignedComputationProfile<'a>,
    pub roles: ResidentRoles,
}
impl ResidentAppliance<'_> {
    pub fn software_spec(&self) -> ResidentSoftwareSpec<'_> {
        ResidentSoftwareSpec {
            cadence: self.cadence.clone(),
            source: self.source,
            computations: self.computations,
            roles: self.roles,
        }
    }
}
#[derive(Debug, PartialEq, Eq)]
pub enum NativePreparationRefusal {
    Tables(AssignedKernelRefusal),
    #[cfg(feature = "local-planning")]
    LocalTables(conduit_plan_lowering::local_tables::LocalTableRefusal),
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
        ValidatedAssignedAppliance::admit(bytes, maxima, required, resident)?.initialize_with(
            |computation| Ok((computation_text(bytes, computation)?, None)),
            values,
        )
    }
}
/// Independently provisioned firmware data, not a received assignment or grant.
#[derive(Clone, Copy)]
pub struct ResidentComputation<'a> {
    pub node: u16,
    pub key: c::AssignedIdentity,
    pub text: &'a str,
    /// Optional Make-decoded bytes. Checked against the pinned text before allocation.
    pub canonical_bytes: Option<&'a [u8]>,
}
/// Optional linked storage for one independently checked selector's exact Types.
/// No readiness, assignment or authority is conveyed by these immutable bytes.
#[derive(Clone, Copy)]
pub struct ResidentSelectorSchemas {
    pub node: u16,
    pub key: c::AssignedIdentity,
    pub text_digest: [u8; 32],
    pub input_schema: &'static [u8],
    pub output_schema: &'static [u8],
}
/// Owns validated current tables and Source cadence; borrows no assignment bytes.
/// Admission is not native possession, Body start, authority or memory admission.
pub struct ValidatedAssignedAppliance {
    tables: NativeTables,
    construction: resident_software::Construction,
}
impl ValidatedAssignedAppliance {
    /// Validated descriptive identity; this confers no native possession or Play.
    pub fn identity(&self) -> c::AssignedPlanView {
        self.tables.identity
    }
    /// Source cadence retained by admission, not an authority or start receipt.
    pub fn cadence(&self) -> conduit_time::TickConfiguration {
        conduit_time::TickConfiguration {
            count: self.construction.count,
            period_ms: self.construction.period_ms,
        }
    }
    pub fn admit(
        bytes: &[u8],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
    ) -> Result<Self, NativePreparationRefusal> {
        Self::admit_input(
            crate::assigned::RecordInput::Bytes(bytes),
            maxima,
            required,
            resident,
        )
    }
    /// Identical Source/role/cadence admission with resident and wire records
    /// borrowed separately. No owner transfer or native authority is performed.
    pub fn admit_records(
        header: &[u8],
        records: &[c::AssignedRecord<'_>],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
    ) -> Result<Self, NativePreparationRefusal> {
        Self::admit_input(
            crate::assigned::RecordInput::Records { header, records },
            maxima,
            required,
            resident,
        )
    }
    /// Preparation-only heap destination for constrained stacks. The same
    /// admission fills this storage; refusal drops it without exposing tables.
    pub fn admit_records_boxed(
        header: &[u8],
        records: &[c::AssignedRecord<'_>],
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
    ) -> Result<Box<Self>, NativePreparationRefusal> {
        let mut output = Box::<Self>::new_uninit();
        Self::admit_input_into(
            crate::assigned::RecordInput::Records { header, records },
            maxima,
            required,
            resident,
            &mut output,
        )?;
        // SAFETY: the common admission writes the complete value before Ok.
        Ok(unsafe { output.assume_init() })
    }
    fn admit_input(
        input: crate::assigned::RecordInput<'_>,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
    ) -> Result<Self, NativePreparationRefusal> {
        let mut output = MaybeUninit::uninit();
        Self::admit_input_into(input, maxima, required, resident, &mut output)?;
        // SAFETY: the common admission writes the complete value before Ok.
        Ok(unsafe { output.assume_init() })
    }
    #[inline(never)]
    fn admit_input_into(
        input: crate::assigned::RecordInput<'_>,
        maxima: c::AssignedPlanMaxima,
        required: c::AssignedPlanRequirements<'_>,
        resident: &ResidentAppliance<'_>,
        output: &mut MaybeUninit<Self>,
    ) -> Result<(), NativePreparationRefusal> {
        let target = output.as_mut_ptr();
        // SAFETY: the tables field has the same layout as MaybeUninit<NativeTables>.
        // We create only a reference to that uninitialized storage, never to Self
        // or uninitialized tables. The parent output is exclusively borrowed.
        let tables = unsafe {
            &mut *core::ptr::addr_of_mut!((*target).tables).cast::<MaybeUninit<NativeTables>>()
        };
        NativeTables::prepare_input_into(
            input,
            maxima,
            required,
            resident.computations,
            &resident.layout,
            tables,
        )
        .map_err(NativePreparationRefusal::Tables)?;
        // SAFETY: this disjoint field is exclusively owned uninitialized storage.
        let construction = unsafe {
            &mut *core::ptr::addr_of_mut!((*target).construction)
                .cast::<MaybeUninit<resident_software::Construction>>()
        };
        if let Err(error) = Self::finish_input(input, resident, construction) {
            // SAFETY: the successful table builder initialized this field.
            unsafe {
                tables.assume_init_drop();
            }
            return Err(error);
        }
        Ok(())
    }
    #[inline(never)]
    fn finish_input(
        input: crate::assigned::RecordInput<'_>,
        resident: &ResidentAppliance<'_>,
        output: &mut MaybeUninit<resident_software::Construction>,
    ) -> Result<(), NativePreparationRefusal> {
        let roles = resident.roles;
        let count = numeric(input, roles.tick, "count")?;
        let period_ms = numeric(input, roles.tick, "period-ms")?;
        let construction =
            resident_software::Construction::prepare(&resident.software_spec(), count, period_ms)?;
        validate_roles(input, roles)?;
        output.write(construction);
        Ok(())
    }
    /// Compatibility preparation after assignment admission. Native Roots may
    /// instead retain software before planning and use `bind_retained` later.
    pub fn initialize<V: ValueStorage>(
        self,
        resident: &[ResidentComputation<'_>],
        values: V,
    ) -> Result<PreparedAssignedAppliance<V>, NativePreparationRefusal> {
        resident_software::validate_texts(&self.construction, resident)?;
        self.initialize_with(
            |expected| {
                resident
                    .iter()
                    .find(|text| text.node == expected.node && text.key == expected.key)
                    .map(|text| (text.text, text.canonical_bytes))
                    .ok_or(NativePreparationRefusal::Configuration)
            },
            values,
        )
    }
    fn initialize_with<'a, V: ValueStorage>(
        self,
        text: impl Fn(
            &c::AssignedComputationRequirement,
        ) -> Result<(&'a str, Option<&'a [u8]>), NativePreparationRefusal>,
        values: V,
    ) -> Result<PreparedAssignedAppliance<V>, NativePreparationRefusal> {
        Ok(
            PreparedResidentSoftware::initialize_with(self.construction, text, values)?
                .into_assigned(self.tables),
        )
    }
}

fn numeric(
    input: crate::assigned::RecordInput<'_>,
    node: k::NodeId,
    key: &str,
) -> Result<u64, NativePreparationRefusal> {
    let key = c::AssignedIdentity::from_text(key);
    let mut result = None;
    for record in input
        .records()
        .map_err(|_| NativePreparationRefusal::Configuration)?
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

fn validate_roles(
    input: crate::assigned::RecordInput<'_>,
    roles: ResidentRoles,
) -> Result<(), NativePreparationRefusal> {
    let fixed = [
        (roles.pulse, conduit_pulse_capture::PULSE_READ_KIND),
        (roles.tick, conduit_time::TICK_KIND),
        (roles.trigger, conduit_time::TICK_TRIGGER_KIND),
        (roles.observation, crate::observation::KIND),
    ];
    for record in input
        .records()
        .map_err(|_| NativePreparationRefusal::Configuration)?
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

// Compare without a decode buffer; hexadecimal spelling remains part of the
// independently pinned assignment identity. The ordinary parser validates bytes.
fn resident_bytes_match(value: &ResidentComputation<'_>) -> bool {
    let Some(bytes) = value.canonical_bytes else {
        return true;
    };
    if bytes.len().checked_mul(2) != Some(value.text.len()) {
        return false;
    }
    bytes
        .iter()
        .zip(value.text.as_bytes().as_chunks::<2>().0.iter())
        .all(|(byte, pair)| {
            let digit = |n| match n {
                b'0'..=b'9' => Some(n - b'0'),
                b'a'..=b'f' => Some(n - b'a' + 10),
                _ => None,
            };
            digit(pair[0])
                .zip(digit(pair[1]))
                .is_some_and(|(hi, lo)| *byte == (hi << 4) | lo)
        })
}
