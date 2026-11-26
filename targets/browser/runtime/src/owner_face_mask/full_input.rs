//! Raw Face and nested Plan admission before owning deserialization.
use super::full_profile_pool::Lease;
use super::*;
use conduit_browser_mask_offer::full_profile as profile;
use conduit_face::json_model_preflight::{json_shape, MAX_FACE_JSON_BYTES};
use conduit_host_browser_make as limits;
use core::mem::size_of;
use std::rc::Rc;

#[derive(Deserialize)]
struct Header<'a> {
    #[serde(borrow)]
    kind: &'a str,
    protocol: u16,
    #[serde(borrow)]
    response: ResponseHeader<'a>,
}
#[derive(Deserialize)]
struct ResponseHeader<'a> {
    #[serde(borrow)]
    outcome: &'a str,
    #[serde(borrow)]
    schema: &'a str,
    #[serde(borrow)]
    face: &'a serde_json::value::RawValue,
    #[serde(borrow)]
    route: RouteHeader<'a>,
}
#[derive(Deserialize)]
struct PlanHeader<'a> {
    #[serde(borrow)]
    encoding: &'a str,
    expanded_bytes: usize,
    #[serde(borrow)]
    zlib_hex: &'a str,
}
#[derive(Deserialize)]
struct RouteHeader<'a> {
    #[serde(borrow)]
    planned_mask: MaskHeader<'a>,
}
#[derive(Deserialize)]
struct MaskHeader<'a> {
    #[serde(borrow)]
    plan: PlanHeader<'a>,
}
#[derive(Deserialize)]
struct PlanProfile<'a> {
    #[serde(borrow)]
    fragments: [FragmentProfile<'a>; 1],
}
#[derive(Deserialize)]
struct FragmentProfile<'a> {
    #[serde(borrow)]
    placements: [PlacementProfile<'a>; 1],
}
#[derive(Deserialize)]
struct PlacementProfile<'a> {
    #[serde(borrow)]
    execution_profile_id: &'a str,
}

// Do not deserialize the wide internally-tagged legacy response enum here:
// serde would buffer its entire payload as an additional generic Content tree.
// The borrowed header already selected this exact Snapshot alternative.
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct Snapshot {
    outcome: String,
    schema: String,
    face: Box<Face>,
    interactions_admitted: bool,
    route: Box<RemoteOwnerMaskRouteSeal>,
}
#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct FullFrame {
    kind: String,
    protocol: u16,
    response: Snapshot,
}

pub(super) struct Carrier {
    pub face: Face,
    pub route: RemoteOwnerMaskRouteSeal,
    pub interactions_admitted: bool,
    pub canonical: Vec<u8>,
    pub lease: Rc<Lease>,
    pub input_charge: usize,
    pub plan_charge: usize,
}

pub(super) fn parser_workspace_bytes() -> usize {
    // Both current ABI buffers have separately owned backing. This lease pays
    // the temporary serde escape scratch and nested bounded MaskPlan inflater.
    Lease::metadata_bytes()
        + 2 * input_capacity()
        + 256 * 1024
        + 64 * 1024
        + core::mem::size_of::<Header<'static>>()
}
pub(super) fn input_capacity() -> usize {
    limits::OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize + 12 * 1024
}

/// Header inspection itself runs only after the caller acquired the exclusive
/// fixed preparation slot and admitted parser_workspace_bytes().
pub(super) fn selected(bytes: &[u8]) -> Result<bool, String> {
    let header: Header<'_> = match serde_json::from_slice(bytes) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    if header.kind != "face-snapshot-response"
        || header.protocol != 1
        || header.response.outcome != "snapshot"
        || header.response.schema != OWNER_FACE_RESPONSE_SCHEMA
    {
        return Ok(false);
    }
    let p = &header.response.route.planned_mask.plan;
    let (raw, _) = conduit_face::PlannedMaskPlot::preflight_json_plan(
        p.encoding,
        p.expanded_bytes,
        p.zlib_hex,
    )
    .map_err(str::to_owned)?;
    let profile: PlanProfile<'_> = match serde_json::from_slice(&raw) {
        Ok(v) => v,
        Err(_) => return Ok(false),
    };
    Ok(profile.fragments[0].placements[0].execution_profile_id == profile::PROFILE)
}

pub(super) fn prepare(bytes: &[u8], lease: Rc<Lease>) -> Result<Carrier, String> {
    if bytes.is_empty() || bytes.len() > limits::OWNER_FACE_FULL_MAXIMUM_FRAME_BYTES as usize {
        return Err("full Face carrier extent".into());
    }
    lease.admit(parser_workspace_bytes())?;
    let header: Header<'_> = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if header.kind != "face-snapshot-response"
        || header.protocol != 1
        || header.response.outcome != "snapshot"
        || header.response.schema != OWNER_FACE_RESPONSE_SCHEMA
    {
        return Err("full Face frame identity".into());
    }
    let face_bytes = header.response.face.get().as_bytes();
    if face_bytes.len() > MAX_FACE_JSON_BYTES {
        return Err("full Face JSON extent".into());
    }
    let face_charge = profile::preflight_face(face_bytes)?;
    let p = &header.response.route.planned_mask.plan;
    let (plan_bytes, _) = conduit_face::PlannedMaskPlot::preflight_json_plan(
        p.encoding,
        p.expanded_bytes,
        p.zlib_hex,
    )
    .map_err(str::to_owned)?;
    let plan_charge =
        conduit_face::PlannedMaskPlot::generic_json_plan_preparation_bytes(&plan_bytes)
            .map_err(str::to_owned)?;
    drop(plan_bytes);
    let shape = json_shape(bytes).map_err(|e| format!("full Face frame census: {e:?}"))?;
    let fs = json_shape(face_bytes).map_err(|e| format!("full Face census: {e:?}"))?;
    // Subtract the separately counted raw Face subtree. Outside it, the only
    // variable-length table is route.face_basis.sign_ids (String newtypes).
    // Mask/Line records are fixed structs plus strings; the compressed Plan
    // is charged separately before its custom deserializer is invoked.
    let strings = shape
        .string_source_bytes
        .checked_sub(fs.string_source_bytes)
        .ok_or("full frame census underflow")?;
    let elements = shape
        .object_or_string_array_elements
        .checked_sub(fs.object_or_string_array_elements)
        .ok_or("full frame census underflow")?;
    let arrays = shape
        .arrays
        .checked_sub(fs.arrays)
        .ok_or("full frame census underflow")?;
    let frame_charge = size_of::<FullFrame>()
        + size_of::<RemoteOwnerMaskRouteSeal>()
        + (2 * elements + 4 * arrays) * size_of::<SignId>()
        + 4 * strings;
    let input_charge = parser_workspace_bytes()
        .checked_add(frame_charge)
        .and_then(|n| n.checked_add(plan_charge))
        .and_then(|n| n.checked_add(face_charge))
        .and_then(|n| n.checked_add(MAX_FACE_JSON_BYTES))
        .ok_or("full Face preparation overflow")?;
    lease.admit(input_charge)?;
    let frame: FullFrame = serde_json::from_slice(bytes).map_err(|e| e.to_string())?;
    if frame.kind != "face-snapshot-response"
        || frame.protocol != 1
        || frame.response.outcome != "snapshot"
        || frame.response.schema != OWNER_FACE_RESPONSE_SCHEMA
    {
        return Err("Full snapshot identity changed".into());
    }
    let Snapshot {
        face,
        route,
        interactions_admitted,
        ..
    } = frame.response;
    profile::eligible(&face)?;
    let canonical = profile::bounded_json_bytes(&face, MAX_FACE_JSON_BYTES)?;
    if canonical != face_bytes {
        return Err("full Face noncanonical JSON".into());
    }
    if route.face_id != face.identity || route.face_revision != face.revision {
        return Err("full Face route mismatch".into());
    }
    Ok(Carrier {
        face: *face,
        route: *route,
        interactions_admitted,
        canonical,
        lease,
        input_charge,
        plan_charge,
    })
}
