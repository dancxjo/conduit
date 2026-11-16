//! Named owners for one closed native view, its ordinary wardrobe and one
//! synchronous typed local-control export. Input Face/offers/Plan stay borrowed
//! under their independently owned caller/IMAGE custody throughout construction.
use super::*;
use std::{
    io::{self, Write},
    mem::size_of,
};

pub(super) struct Backing {
    pub(super) peak: usize,
}
fn add(total: &mut usize, bytes: usize) -> Option<()> {
    *total = total.checked_add(bytes)?;
    Some(())
}
fn raw_face_bytes(face: &Face) -> Option<usize> {
    struct Count(Option<usize>);
    impl Write for Count {
        fn write(&mut self, bytes: &[u8]) -> io::Result<usize> {
            self.0 = self.0.and_then(|n| n.checked_add(bytes.len()));
            Ok(bytes.len())
        }
        fn flush(&mut self) -> io::Result<()> {
            Ok(())
        }
    }
    let mut count = Count(Some(0));
    serde_json::to_writer(&mut count, face).ok()?;
    count.0
}
fn host_basis_bytes(host: &HostAdvertisement) -> Option<usize> {
    host.host_id
        .0
        .capacity()
        .checked_add(host.boot_id.0.capacity())?
        .checked_add(64)
}
fn json_growth(bytes: usize) -> Option<usize> {
    let next = bytes.max(128).checked_next_power_of_two()?;
    next.checked_add(next / 2)
}
impl Backing {
    pub(super) fn checked(
        credential: &MembershipCredential,
        owner: &HostAdvertisement,
        target: &HostAdvertisement,
        body: &conduit_body::Body,
        face: &Face,
        plan: &PlannedMaskPlot,
        lines: [&LineOffer; 3],
    ) -> Option<Self> {
        let plan_payload = conduit_face::native_mask_plan_backing_bytes(plan)?;
        let verifier = super::super::backing::native_verification_workspace(plan)?;
        let face_json = raw_face_bytes(face)?;
        if face_json > lines[0].binding.limits.maximum_payload_bytes as usize {
            return None;
        }
        let basis = super::super::backing::basis_payload_bytes(&face.basis)?;
        let plot = super::super::backing::plot_payload_bytes(&plan.mask.plot_identity)?;
        // format!(prefix +64hex) starts at twice its literal prefix length;
        // count possible final doubling plus original separately below.
        let remote_prefix = "plan/remote-owner-mask/".len();
        let remote_id = (remote_prefix + 64).max(remote_prefix.checked_mul(4)?);
        let owner_prefix = "plan/owner-face/".len();
        let owner_id = (owner_prefix + 64).max(owner_prefix.checked_mul(4)?);
        let selected_id = ("route/".len() + remote_id).max("route/".len() * 4);
        let mut seal = size_of::<RemoteOwnerMaskRouteSeal>();
        for bytes in [
            remote_id,
            body.body_id.retained_backing_bytes(),
            face.identity.retained_backing_bytes(),
            basis,
            host_basis_bytes(owner)?,
            host_basis_bytes(target)?,
            plan_payload,
        ] {
            add(&mut seal, bytes)?;
        }
        for line in lines {
            add(
                &mut seal,
                super::super::backing::admitted_line_payload_bytes(line)?,
            )?;
        }
        let mut route = size_of::<NativeMaskRoute>();
        for bytes in [
            credential.retained_backing_bytes()?,
            target.host_id.0.capacity(),
            target.boot_id.0.capacity(),
            seal.checked_sub(size_of::<RemoteOwnerMaskRouteSeal>())?,
        ] {
            add(&mut route, bytes)?;
        }
        for line in lines {
            add(
                &mut route,
                super::super::backing::line_offer_payload_bytes(line)?,
            )?;
        }
        // Outer ordinary Plan header and its one boxed native child seal.
        let header = owner_id
            .checked_add(body.body_id.retained_backing_bytes())?
            .checked_add(face.identity.retained_backing_bytes())?
            .checked_add(basis)?;
        let owner_plan = size_of::<conduit_face::OwnerFacePlan>()
            .checked_add(header)?
            .checked_add(size_of::<conduit_face::OwnerFaceChildRoute>())?
            .checked_add(seal)?;
        // Body-scoped worn/preference each own one Plot; selected child owns a
        // third Plot, exact outer Plan ID and child route string.
        let selected = size_of::<conduit_face::SelectedMaskPlotRoute>()
            .checked_add(selected_id)?
            .checked_add(plot)?
            .checked_add(owner_id)?;
        let control = size_of::<conduit_face::MaskWardrobeControl>()
            .checked_add(body.body_id.retained_backing_bytes())?
            .checked_add(size_of::<PlotIdentity>())?
            .checked_add(plot)?
            .checked_add(size_of::<PlotIdentity>())?
            .checked_add(plot)?
            .checked_add(owner_id)?
            .checked_add(selected.checked_sub(size_of::<conduit_face::SelectedMaskPlotRoute>())?)?;
        let wardrobe = size_of::<super::super::super::face_wardrobe::OwnerFaceWardrobe>()
            .checked_add(owner_plan.checked_sub(size_of::<conduit_face::OwnerFacePlan>())?)?
            .checked_add(control.checked_sub(size_of::<conduit_face::MaskWardrobeControl>())?)?;
        // admit_current_routes owns availability, one Sealed route and its4
        // placement ID clones while the actual OwnerFacePlan stays alive.
        let mut admitted = size_of::<conduit_face::SealedMaskPlotRoute>()
            .checked_add(1)?
            .checked_add(selected_id)?
            .checked_add(plot)?
            .checked_add(owner_id)?
            .checked_add(plan.plan.plan_id.0.capacity())?
            .checked_add(remote_id)?;
        for gear in &plan.plan.fragments[0].placements {
            add(
                &mut admitted,
                size_of::<PlacementId>().checked_add(gear.placement_id.0.capacity())?,
            )?;
        }
        let mut peak = size_of::<StagedView>();
        for bytes in [
            route.checked_sub(size_of::<NativeMaskRoute>())?,
            wardrobe.checked_sub(size_of::<
                super::super::super::face_wardrobe::OwnerFaceWardrobe,
            >())?,
            seal.checked_sub(size_of::<RemoteOwnerMaskRouteSeal>())?, // retained descendants; inline is in StagedView
            seal, // witness.child() clone during ordinary admission
            control,
            selected, // reconciliation clone and returned selection
            admitted,
            verifier,
            json_growth(face_json)?,
            plan.json_workspace()?.codec_bytes,
            conduit_face::RemoteMaskRouteProfile::Native.identity_encoding_limit(),
            conduit_face::OwnerFacePlan::identity_encoding_limit(),
            // Original identity literal/hex scratch and the unsealed marker.
            remote_prefix.checked_mul(2)?,
            owner_prefix.checked_mul(2)?,
            64,
            64,
            "unsealed".len(),
            // Existing actor cap is actually enforced before exact output
            // allocation by GuardedResponse; it is not a grant or TLS budget.
            crate::durable_host_control::MAXIMUM_LOCAL_CONTROL_FRAME_BYTES,
            size_of::<RemoteOwnerMaskRouteSeal>(),
            // Active holds this route ID independently of the retained seal.
            remote_id,
        ] {
            add(&mut peak, bytes)?;
        }
        Some(Self { peak })
    }
}
