// One-time Source-selected software construction. Arrays and constructor
// scratch are cold stack work; only the original owned inventories remain in
// caller slots. This module creates no admission, grants, or execution effects.
use super::*;
use conduit_kernel::ValueRef;
use conduit_mcu_host_runtime::{
    execution_authority::Operation,
    execution_factory::{SoftwareDefinition, SoftwareInventory},
    execution_software::PublicationTarget,
};
mod recording {
    include!(concat!(env!("OUT_DIR"), "/source_play_recording.rs"));
}

#[inline(never)]
pub(super) fn publication_bounds(
    parameters: &[(u16, ValueRef)],
) -> (u32, Option<Operation<'static>>) {
    let definitions =
        source::sampling::definitions(parameters).expect("checked software definitions");
    let recording = recording::operation(&definitions, source::sampling::NATIVE_KINDS)
        .expect("independently compiled finite recording operation");
    let maximum_events = definitions
        .iter()
        .flatten()
        .filter_map(|d| match d {
            SoftwareDefinition::Publication { maximum_events, .. } => Some(*maximum_events),
            _ => None,
        })
        .max()
        .expect("actual Source publication");
    (maximum_events, recording)
}

#[inline(never)]
pub(super) fn sampling<'f>(
    slot: &mut Option<SoftwareInventory<'f, { source::sampling::NODE_COUNT }>>,
    parameters: &[(u16, ValueRef)],
    clocks: &[(u16, u64, u64); W],
    equipment: &crate::equipment::Equipment,
    publisher: &'f dyn PublicationTarget,
    sequence: PublicationSequence,
) {
    assert!(slot.is_none(), "cold sampling inventory slot");
    let definitions =
        source::sampling::definitions(parameters).expect("checked software definitions");
    let clock_identities: [_; W] = core::array::from_fn(|slot| {
        let actual = equipment.waits[slot]
            .as_ref()
            .expect("actual wait slot")
            .native_identity();
        conduit_plan_lowering::resident_inventory::OtherNodeIdentity {
            node: clocks[slot].0,
            kind: conduit_core::AssignedIdentity::from_text(conduit_time::TICK_KIND),
            implementation: conduit_core::AssignedIdentity::from_text(
                actual.implementation_id.as_str(),
            ),
            artifact: conduit_core::AssignedIdentity::from_text(actual.artifact_id.as_str()),
        }
    });
    // SAFETY: exact definitions/identities/configuration are from the same
    // independently checked Source; this synchronous constructor is the sole
    // allocator user, with the unchanged preparation/backing funding.
    *slot = Some(
        unsafe {
            SoftwareInventory::initialize(
                definitions,
                source::sampling::identities(&clock_identities)
                    .expect("actual Source clock identities"),
                publisher,
                sequence,
                now_ms,
                allocator_used,
                source::sampling::CONFIGURATION,
            )
        }
        .expect("actual initialized sampling Backs"),
    );
}

#[inline(never)]
pub(super) fn output<'f>(
    slot: &mut Option<SoftwareInventory<'f, { source::output::NODE_COUNT }>>,
    publisher: &'f dyn PublicationTarget,
    sequence: PublicationSequence,
) {
    assert!(slot.is_none(), "cold output inventory slot");
    // SAFETY: same independently checked Source/identity and exclusive
    // synchronous allocator obligations as the sampling constructor above.
    *slot = Some(
        unsafe {
            SoftwareInventory::initialize(
                source::output::definitions(&[]).expect("checked output definitions"),
                source::output::identities(&[]).expect("checked output identities"),
                publisher,
                sequence,
                now_ms,
                allocator_used,
                source::output::CONFIGURATION,
            )
        }
        .expect("actual initialized output Backs"),
    );
}

// Fixed-buffer and finite owner initialization belongs to the cold call, not
// the stack frame retained beneath every subsequent admission/actor poll.
#[inline(never)]
pub(super) fn response_buffer() -> [u8; 4096] {
    [0; 4096]
}
#[inline(never)]
pub(super) fn optional_outputs(
) -> conduit_kernel::scheduler::prepared_session::OptionalOutputStorage<6> {
    conduit_kernel::scheduler::prepared_session::OptionalOutputStorage::prepare()
}
#[inline(never)]
pub(super) fn issuers() -> Issuers<W> {
    Issuers::new()
}
#[inline(never)]
pub(super) fn routing() -> ReadyMaskRouting {
    ReadyMaskRouting::new(
        source::mask_layout::MASK_ROLES.expect("checked actual Mask roles"),
        display::RENDERER_FACE_BYTES,
        display::RENDERER_SHOW_BYTES,
    )
    .expect("actual initialized Mask routing")
}
#[inline(never)]
pub(super) fn operations(clocks: &[(u16, u64, u64); W]) -> Operations<'static, W> {
    Operations::from_source(
        source::sampling::NATIVE_KINDS,
        source::mask::NATIVE_KINDS,
        source::output::NATIVE_KINDS,
        core::array::from_fn(|slot| Some(clocks[slot])),
        display::RENDERER_MAXIMUM_FRAMES,
        display::RENDERER_FACE_BYTES,
        display::RENDERER_SHOW_BYTES,
    )
    .expect("finite independently compiled operations")
}
