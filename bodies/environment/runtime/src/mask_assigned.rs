//! Compact assigned records for the same ordinary Mask kernel. Resident pins
//! come from checked Make material; received descriptors cannot supply them.
use super::{Kernel, MaskBack};
use crate::assigned::{AssignedKernelRefusal, AssignedKernelTables, ResidentLayout};
use conduit_core as c;
use conduit_kernel::{self as k, SignSink};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MaskRole {
    Tee,
    Renderer,
    ReadOnly,
}
pub struct AssignedMaskProfile<'a> {
    pub layout: ResidentLayout<'a>,
    pub fore: &'a [c::AssignedForeRequirement],
    pub roles: [MaskRole; 3],
    pub present_call: c::AssignedIdentity,
    pub resources: &'a [u16],
    pub face_bytes: u32,
    pub show_bytes: u32,
}
#[derive(Clone, Copy)]
pub struct AssignedMaskInput<'a> {
    pub header: &'a [u8],
    pub records: &'a [c::AssignedRecord<'a>],
    pub maxima: c::AssignedPlanMaxima,
}
/// Expected identities must come from the current authenticated installation,
/// never be copied out of the received assignment for comparison with itself.
#[derive(Clone, Copy)]
pub struct AssignedMaskScope {
    pub host: c::AssignedIdentity,
    pub boot: c::AssignedIdentity,
    pub plan: c::AssignedIdentity,
    pub fragment: c::AssignedIdentity,
}
#[derive(Debug, PartialEq, Eq)]
pub enum AssignedMaskRefusal {
    Shape,
    Scope,
    Bounds,
    Assignment(AssignedKernelRefusal),
    Kernel,
}
type Tables = AssignedKernelTables<3, 6, 2, 6, 6, 3>;
pub struct PreparedAssignedMaskKernel {
    pub identity: c::AssignedPlanView,
    pub kernel: alloc::boxed::Box<Kernel>,
    pub face: c::AssignedForeRequirement,
    pub show: c::AssignedForeRequirement,
    pub interaction: c::AssignedForeRequirement,
    pub renderer: k::NodeId,
}
impl PreparedAssignedMaskKernel {
    /// Exact heap backing for the boxed scheduler and its two canonical slots.
    /// Allocator overhead, caller stack/wrapper, retained assignment/material,
    /// evidence transport, renderer context and native providers are separate.
    pub fn required_heap_bytes(
        profile: &AssignedMaskProfile<'_>,
    ) -> Result<usize, AssignedMaskRefusal> {
        if profile.face_bytes == 0
            || profile.face_bytes > 8192
            || profile.show_bytes == 0
            || profile.show_bytes > 8192
        {
            return Err(AssignedMaskRefusal::Bounds);
        }
        k::HostedValueStore::canonical_storage_bytes([profile.face_bytes, profile.show_bytes])
            .map_err(|_| AssignedMaskRefusal::Bounds)?
            .checked_add(core::mem::size_of::<Kernel>())
            .ok_or(AssignedMaskRefusal::Bounds)
    }
    /// Pure table/storage preparation. This is not Body admission, current offer
    /// validation, native possession or permission to start/complete a Show.
    /// Root must reserve the actual allocator/stack headroom before this call.
    pub fn prepare(
        input: AssignedMaskInput<'_>,
        profile: &AssignedMaskProfile<'_>,
        scope: AssignedMaskScope,
    ) -> Result<Self, AssignedMaskRefusal> {
        use AssignedMaskRefusal as E;
        Self::required_heap_bytes(profile)?;
        let (tables, face, show, interaction, renderer) = admit(input, profile, scope)?;
        let values = k::HostedValueStore::new_with_slot_capacities(
            &[profile.face_bytes, profile.show_bytes],
            profile.face_bytes + profile.show_bytes,
        )
        .map_err(|_| E::Bounds)?;
        let signs = k::FixedSignLog::<64, 12>::new_with_remote_storage(
            (64 * core::mem::size_of::<k::KernelEvent>()) as u32,
            12,
            k::remote_sign_storage_bytes(12).ok_or(E::Bounds)?,
        )
        .map_err(|_| E::Bounds)?;
        signs.ensure_remote_capacity(12).map_err(|_| E::Bounds)?;
        let backs = profile.roles.map(|role| match role {
            MaskRole::Tee => MaskBack::Tee,
            MaskRole::Renderer => MaskBack::Renderer {
                state: 0,
                maximum_input_bytes: profile.face_bytes,
                next_request: 0,
            },
            MaskRole::ReadOnly => MaskBack::ReadOnly { face: false },
        });
        let identity = tables.identity;
        let kernel = Kernel::new_boxed_with_active_counts_and_host_calls(
            3,
            6,
            tables.nodes,
            tables.cords,
            tables.routes,
            tables.calls,
            backs,
            values,
            signs,
        )
        .map_err(|_| E::Kernel)?;
        Ok(Self {
            identity,
            kernel,
            face,
            show,
            interaction,
            renderer,
        })
    }
}
fn admit(
    input: AssignedMaskInput<'_>,
    profile: &AssignedMaskProfile<'_>,
    scope: AssignedMaskScope,
) -> Result<
    (
        Tables,
        c::AssignedForeRequirement,
        c::AssignedForeRequirement,
        c::AssignedForeRequirement,
        k::NodeId,
    ),
    AssignedMaskRefusal,
> {
    use AssignedMaskRefusal as E;
    if profile.fore.len() != 3
        || profile.resources.len() > 1
        || profile.face_bytes == 0
        || profile.face_bytes > 8192
        || profile.show_bytes == 0
        || profile.show_bytes > 8192
        || profile
            .layout
            .records
            .iter()
            .filter(|r| r.tag == c::ASSIGNED_CONFIGURATION)
            .count()
            != 3
        || profile.layout.counts[6] != 1
    {
        return Err(E::Shape);
    }
    let node = |role| {
        profile
            .roles
            .iter()
            .position(|r| *r == role)
            .map(|n| k::NodeId(n as u16))
            .ok_or(E::Shape)
    };
    let tee = node(MaskRole::Tee)?;
    let renderer = node(MaskRole::Renderer)?;
    let read_only = node(MaskRole::ReadOnly)?;
    let fore = |name: &str, direction| {
        profile
            .fore
            .iter()
            .find(|f| {
                f.direction() == direction
                    && f.payload()[5..21] == c::AssignedIdentity::from_text(name).0
            })
            .copied()
            .ok_or(E::Shape)
    };
    let face = fore("face", c::AssignedForeDirection::Input)?;
    let show = fore("show", c::AssignedForeDirection::Output)?;
    let interaction = fore("interaction", c::AssignedForeDirection::Output)?;
    for (index, role) in profile.roles.iter().enumerate() {
        let kind = match role {
            MaskRole::Tee => conduit_face::FACE_TEE_KIND,
            MaskRole::Renderer => conduit_face::RENDERER_KIND,
            MaskRole::ReadOnly => conduit_face::FACE_INTERACTION_KIND,
        };
        let record = input
            .records
            .iter()
            .find(|r| {
                r.tag == c::ASSIGNED_NODE
                    && r.payload.get(..2) == Some(&(index as u16).to_le_bytes())
            })
            .ok_or(E::Shape)?;
        if record.payload.get(4..20) != Some(&c::AssignedIdentity::from_text(kind).0) {
            return Err(E::Shape);
        }
    }
    let calls = [profile.present_call];
    let required = c::AssignedPlanRequirements {
        host: scope.host,
        boot: scope.boot,
        host_calls: &calls,
        resources: profile.resources,
        remote_bindings: &[],
    };
    let tables = Tables::prepare_records_with_fore(
        input.header,
        input.records,
        input.maxima,
        required,
        c::AssignedComputationProfile {
            maximum_text_bytes: 0,
            maximum_total_text_bytes: 0,
            requirements: &[],
        },
        &profile.layout,
        profile.fore,
    )
    .map_err(E::Assignment)?;
    if tables.identity.plan != scope.plan || tables.identity.fragment != scope.fragment {
        return Err(E::Scope);
    }
    let face_cord = tables.cords.get(usize::from(face.cord())).ok_or(E::Shape)?;
    let show_cord = tables.cords.get(usize::from(show.cord())).ok_or(E::Shape)?;
    let interaction_cord = tables
        .cords
        .get(usize::from(interaction.cord()))
        .ok_or(E::Shape)?;
    if face_cord.sink != k::CordEndpoint::local(tee, k::PortId(0))
        || show_cord.source != k::CordEndpoint::local(renderer, k::PortId(0))
        || interaction_cord.source != k::CordEndpoint::local(read_only, k::PortId(0))
        || face_cord.byte_capacity != profile.face_bytes
        || show_cord.byte_capacity != profile.show_bytes
        || tables.cords.iter().any(|cord| {
            cord.item_capacity != 1 || cord.maximum_value_bytes > 8192 || cord.byte_capacity > 8192
        })
    {
        return Err(E::Shape);
    }
    let mut call_records = input
        .records
        .iter()
        .filter(|r| r.tag == c::ASSIGNED_HOST_CALL);
    let call = call_records.next().ok_or(E::Shape)?.payload;
    if call_records.next().is_some()
        || call.get(..2) != Some(&renderer.0.to_le_bytes())
        || call.get(2..4) != Some(&0u16.to_le_bytes())
        || call.get(36..38) != Some(&1u16.to_le_bytes())
        || call.get(38..42) != Some(&profile.face_bytes.to_le_bytes())
        || call.get(42..46) != Some(&profile.show_bytes.to_le_bytes())
    {
        return Err(E::Shape);
    }
    match c::assigned_native_binding(call).map_err(|_| E::Shape)? {
        None if profile.resources.is_empty() => (),
        Some(binding)
            if binding.host == scope.host
                && binding.boot == scope.boot
                && profile.resources == [binding.resource] => {}
        _ => return Err(E::Shape),
    }
    // The shared constructor will validate the final tables before publishing a
    // scheduler. No rich Plan/Source/offer or mutable provider is retained here.
    Ok((tables, face, show, interaction, renderer))
}
