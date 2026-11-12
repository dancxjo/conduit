//! Requested-backing admission for one standard Linux retained Mask invocation.
//! Named construction/submit overlaps, never a caller-authored scalar or RSS claim.
use super::*;
use conduitos::native_face_scene::NativeFaceScene;
use core::mem::size_of;
fn add(a: usize, b: usize) -> Result<usize, String> {
    a.checked_add(b)
        .ok_or_else(|| "native invocation backing overflow".into())
}
fn sum(values: &[usize]) -> Result<usize, String> {
    values.iter().try_fold(0, |total, value| add(total, *value))
}
/// Conservative simultaneous lifetime envelope. Every multiplier below is an
/// explicit set of named owners/overlaps from the actual standard constructors.
pub(super) fn checked_invocation_bytes(
    planned: &PlannedMaskPlot,
    face: &Face,
    front: &str,
    display_base: &HostBaseId,
) -> Result<usize, String> {
    if !cfg!(target_os = "linux") {
        return Err("bounded interactive invocation requires Linux".into());
    }
    let plan = conduitos::product_bases::native_mask_plan_backing_bytes(planned)
        .ok_or("unsupported native invocation Plan owners")?;
    let scene = NativeFaceScene::standard_interactive_memory_bounds(face)
        .map_err(|error| format!("native Scene bound:{error:?}"))?
        .lifetime_peak_bytes()
        .ok_or("native Scene bound overflow")?;
    let face_bytes = face
        .retained_backing_bytes()
        .ok_or("native Face bound unsupported")?;
    let fragment = planned
        .plan
        .fragments
        .first()
        .ok_or("native fragment absent")?;
    let source = fragment
        .placements
        .iter()
        .find(|p| p.base.is_some())
        .ok_or("native resource source absent")?;
    let base = source.base.as_ref().ok_or("native resource Base absent")?;
    let mask_plot = &planned.mask.plot_identity;
    let mask_identity = sum(&[
        mask_plot.source_document_id.0.len(),
        mask_plot.checked_plot_id.0.len(),
        mask_plot.expanded_plot_id.0.len(),
    ])?;
    let face_identity = sum(&[
        face.basis
            .source_document_id
            .as_ref()
            .map_or(0, |id| id.0.len()),
        face.basis
            .checked_plot_id
            .as_ref()
            .map_or(0, |id| id.0.len()),
        face.basis
            .expanded_plot_id
            .as_ref()
            .map_or(0, |id| id.0.len()),
        face.basis.plan_id.as_ref().map_or(0, |id| id.0.len()),
        face.basis
            .body_id
            .as_ref()
            .map_or(0, |id| id.retained_backing_bytes()),
        face.basis
            .wake_id
            .as_ref()
            .map_or(0, |id| id.retained_backing_bytes()),
    ])?;
    // Manifestation fields drawn once from one placement/Plan are a subset of
    // the exhaustive Plan visitor. New active/manifestation/Face hashes are 64B.
    let manifest_fields = sum(&[plan, face_identity, front.len(), SURFACE.len(), 64, 64, 64])?;
    // Each of the two signs copies a subset of Plan IDs plus its three hashes
    // and the fixed prepared/available sign spelling (<32B). Vec grows 1→4;
    // old Vec storage and replacement are charged separately below.
    let sign_payload = sum(&[plan, 64, 64, 64, 32])?;
    let manifest = sum(&[
        manifest_fields,
        4usize
            .checked_mul(size_of::<conduit_face::ManifestationSign>())
            .ok_or("sign slots overflow")?,
        sign_payload,
        sign_payload,
    ])?;
    let show = sum(&[plan, mask_identity, face_identity, 64, 69, manifest])?;
    // prepared Show, available Show, transition's replaced Manifestation/old
    // sign Vec, and current-Show copy retained by input/completed correlation.
    let shows = sum(&[
        show,
        show,
        show,
        show,
        size_of::<conduit_face::ManifestationSign>(),
    ])?;
    let buffers = (conduit_conduitos_mask_offer::NativeMaskStorageProfile::Standard.bytes()
        as usize)
        .checked_mul(2)
        .ok_or("JSON buffers overflow")?;
    // Cached invocation contains three Fore descriptions, each a subset of the
    // checked four-Gear Plan. Its clone capacity cannot exceed source backing.
    let fore_clones = sum(&[plan, plan, plan])?;
    // ActiveSurfacePossession uses one finite table, one issuer request and
    // accepted Cord. Each scope/authority/Cord string set is drawn at most once
    // from the Plan; generation and active/digest/envelope IDs are fresh.
    let generation = add(base.provider_instance_id.0.len(), "/generation/".len() + 20)?
        .checked_mul(2)
        .ok_or("generation formatting overflow")?;
    let scope = sum(&[
        plan,
        generation,
        64,
        64,
        "conduitos/native-mask/present@1".len(),
    ])?;
    let possession = sum(&[
        conduit_core::BaseCapabilityTable::single_issuance_backing_bytes(1, 1)
            .ok_or("table geometry")?,
        plan,  // issuer Host/Boot/instance IDs
        scope, // original scope → eventual operation claim
        scope, // issue request scope → table inspection
        scope, // issue request authority, including generation
        scope, // offered resource Cord (then moved into accepted Cord)
        scope, // accepted Cord + completion receipt/refusal ownership overlap
        size_of::<(
            conduit_core::ResourceCordRefusal,
            conduit_core::OfferedResourceCord,
        )>(), // typed refusal Box
    ])?;
    // Compositor update owns a surface binding and a receipt; caller retains
    // the exact cloned composition receipt. Every ID subset is Plan-owned,
    // plus fixed derived hashes, front, Base, surface and focus spelling.
    let composition = sum(&[
        plan,
        front.len(),
        display_base.0.len(),
        SURFACE.len(),
        64,
        64,
        64,
    ])?;
    let compositor = sum(&[composition, composition, composition, SURFACE.len()])?;
    // Receipt, correlation plot/Face/Show identities and kernel completion
    // serialization coexist with Scene's already-counted typed input owner.
    let completion = sum(&[
        plan,
        display_base.0.len(),
        SURFACE.len(),
        64,
        64,
        128, // show-value/ digest String grows from its fixed prefix
        mask_identity,
        64,
        69,
        conduit_face::MAX_FACE_INTERACTION_BYTES,
        ImageMemoryLease::owned_backing_bytes(),
    ])?;
    sum(&[
        scene,
        face_bytes,
        shows,
        buffers,
        fore_clones,
        possession,
        compositor,
        completion,
    ])
}
