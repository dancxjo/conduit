//! Complete generic reading and lifecycle outputs prepared before Mask Play.
use super::full_profile_pool::Lease;
use super::full_refusal::FullPreparationRefusal as Refusal;
use super::*;
use conduit_browser_mask_offer::full_profile as profile;
use conduit_host_browser_make as limits;
use core::mem::size_of;
use std::rc::Rc;

pub(super) struct Outputs {
    pub lease: Rc<Lease>,
    pub prepared_view: Vec<u8>,
    pub available_view: Vec<u8>,
    pub canonical: Vec<u8>,
    pub receipt: Vec<u8>,
    pub manifestation: Vec<u8>,
    pub available_show: Option<MaskShow>,
    pub replaced_show: Option<MaskShow>,
    pub revision: String,
}
pub(super) struct Basis<'a> {
    pub body: &'a BodyId,
    pub face: &'a Face,
    pub planned: &'a conduit_face::PlannedMaskPlot,
    pub play: &'a conduit_core::ActivePlayIdentity,
    pub show: &'a MaskShow,
    pub route: Option<&'a RouteView>,
    pub interactions_admitted: bool,
}

#[derive(Serialize)]
struct Canonical<'a> {
    schema: &'static str,
    face: &'a Face,
    face_revision_decimal: &'a str,
    route: Option<&'a RouteView>,
}

fn show_metadata_bytes() -> usize {
    const ID: usize = 256;
    let signs = (2 * conduit_face::MAX_MANIFESTATION_SIGNS + 4)
        * (size_of::<conduit_face::ManifestationSign>() + 9 * ID);
    size_of::<MaskShow>() + 9 * ID + size_of::<conduit_face::Manifestation>() + 14 * ID + signs
}
pub(super) fn replacement_preparation_bytes(plan: usize) -> Option<usize> {
    let mask = size_of::<conduit_face::MaskPlot>() + 13 * 256;
    plan.checked_add(mask)?
        .checked_add(show_metadata_bytes())?
        .checked_add(show_metadata_bytes())
}

/// Named owners, not a multiplier on serialized Face length. The three Show
/// copies are the current Prepared Show and the precomputed Available/Replaced
/// Shows. Each owns its own Plan and MaskPlot; transition also clones one
/// Manifestation temporarily. All copied identities are fenced at256bytes.
pub(super) fn preparation_bytes(face: &Face, plan: usize) -> Result<usize, Refusal> {
    const ID: usize = 256;
    let sign_table = (2 * conduit_face::MAX_MANIFESTATION_SIGNS + 4)
        * (size_of::<conduit_face::ManifestationSign>() + 9 * ID);
    let manifestation = size_of::<conduit_face::Manifestation>() + 14 * ID + sign_table;
    let mask_plot = size_of::<conduit_face::MaskPlot>() + 13 * ID;
    let show = size_of::<MaskShow>() + 9 * ID + manifestation;
    let shows = 3usize
        .checked_mul(
            plan.checked_add(mask_plot)
                .and_then(|n| n.checked_add(show))
                .ok_or(Refusal::Pressure)?,
        )
        .ok_or(Refusal::Pressure)?;
    let outputs = 2 * limits::OWNER_FACE_FULL_MAXIMUM_VIEW_BYTES as usize
        + limits::OWNER_FACE_FULL_MAXIMUM_CANONICAL_BYTES as usize
        + 2 * 60 * 1024;
    super::view_memory::view_preparation_bytes(face)?
        .checked_add(shows)
        .and_then(|n| n.checked_add(manifestation))
        .and_then(|n| n.checked_add(outputs))
        .and_then(|n| n.checked_add(size_of::<Outputs>() + 20))
        .ok_or(Refusal::Pressure)
}

impl Outputs {
    pub(super) fn prepare(b: Basis<'_>, lease: Rc<Lease>) -> Result<Self, Refusal> {
        let ids = [
            b.body.as_str(),
            b.face.identity.as_str(),
            b.planned.mask.plot_identity.checked_plot_id.as_str(),
            b.planned.mask.plot_identity.expanded_plot_id.as_str(),
            b.planned.plan.plan_id.as_str(),
            b.play.active_play_id.as_str(),
            b.planned.show_placement().artifact_id.as_str(),
            b.show.show_id.as_str(),
        ];
        if ids.iter().any(|id| id.len() > 256)
            || b.route.is_some_and(|r| {
                [
                    &r.plan_id,
                    &r.owner_host_id,
                    &r.owner_boot_id,
                    &r.mask_host_id,
                    &r.mask_boot_id,
                ]
                .iter()
                .any(|id| id.len() > 256)
            })
        {
            return Err(Refusal::Pressure);
        }
        let available_show = b
            .show
            .transition(
                ManifestationLifecycle::Available,
                SignId::from(format!("sign/browser-owner-face-shown/{}", b.face.revision)),
            )
            .map_err(|_| Refusal::Preparation)?;
        available_show
            .validate(b.face)
            .map_err(|_| Refusal::Preparation)?;
        let replaced_show = available_show
            .transition(
                ManifestationLifecycle::Replaced,
                SignId::from(format!(
                    "sign/browser-owner-face-replaced/{}",
                    b.face.revision
                )),
            )
            .map_err(|_| Refusal::Preparation)?;
        let route = b.route.map(|r| RouteView {
            plan_id: r.plan_id.clone(),
            owner_host_id: r.owner_host_id.clone(),
            owner_boot_id: r.owner_boot_id.clone(),
            mask_host_id: r.mask_host_id.clone(),
            mask_boot_id: r.mask_boot_id.clone(),
        });
        // The ordinary projection is shared with Legacy. Its optional plotting
        // policy is retained; only Legacy's 64 KiB plot-removal pass is omitted.
        let mut view =
            super::view::project_view(b.body, b.face, b.planned, b.play, b.show, false, route);
        let prepared_view = super::full_refusal::bounded_json(
            &view,
            limits::OWNER_FACE_FULL_MAXIMUM_VIEW_BYTES as usize,
        )?;
        view.show_state = "available";
        view.interactions_admitted = b.interactions_admitted;
        let available_view = super::full_refusal::bounded_json(
            &view,
            limits::OWNER_FACE_FULL_MAXIMUM_VIEW_BYTES as usize,
        )?;
        let revision = b.face.revision.to_string();
        let canonical = super::full_refusal::bounded_json(
            &Canonical {
                schema: "conduit.browser/owner-canonical-face@1",
                face: b.face,
                face_revision_decimal: &revision,
                route: b.route,
            },
            limits::OWNER_FACE_FULL_MAXIMUM_CANONICAL_BYTES as usize,
        )?;
        let receipt = super::full_refusal::bounded_json(&available_show, 60 * 1024)?;
        let manifestation = super::full_refusal::bounded_json(&b.show.show, 60 * 1024)?;
        Ok(Self {
            lease,
            prepared_view,
            available_view,
            canonical,
            receipt,
            manifestation,
            available_show: Some(available_show),
            replaced_show: Some(replaced_show),
            revision,
        })
    }
    pub(super) fn view(&self, lifecycle: ManifestationLifecycle) -> &[u8] {
        debug_assert!(self.lease.admitted() <= profile::PREPARATION_BYTES as usize);
        if lifecycle == ManifestationLifecycle::Available {
            &self.available_view
        } else {
            &self.prepared_view
        }
    }
}
