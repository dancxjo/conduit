//! Admission and immutable Boot-witness checks for the raw generic profile.
use super::full_profile_pool::Lease;
use super::*;
use conduit_browser_mask_offer::full_profile as profile;
use std::rc::Rc;

pub(super) struct Admission {
    pub lease: Rc<Lease>,
    pub input_charge: usize,
    pub canonical: Vec<u8>,
}

impl OwnerBrowserMask {
    pub(super) fn prepare_full(
        basis: HostBasis,
        carrier: super::full_input::Carrier,
        sequence: u64,
    ) -> Result<Self, String> {
        if !carrier.lease.matches(&basis.host_id, &basis.boot_id)
            || basis
                .visual_constraints
                .as_ref()
                .is_some_and(|c| c.requires_full_snapshot())
        {
            return Err("Full Mask incarnation or configured Look unsupported".into());
        }
        let evidence = super::full_profile_pool::snapshot(&basis.host_id, &basis.boot_id)?;
        let route = carrier.route;
        if carrier.face.basis.body_id.as_ref() != Some(&basis.body_id)
            || route.body_id != basis.body_id
            || route.face_id != carrier.face.identity
            || route.face_revision != carrier.face.revision
            || route.face_basis != carrier.face.basis
            || &route.mask_host != evidence.basis()
        {
            return Err("Full Mask Face/Body/Host witness mismatch".into());
        }
        profile::validate_template(&route.planned_mask, evidence.template())?;
        let plan_scratch = conduit_face::PlannedMaskPlot::json_wire_preparation_bytes()
            + 256 * 1024
            + 2 * 64 * 1024; // bounded raw Plan and outer seal Vec growth
        let output_charge =
            super::full_outputs::preparation_bytes(&carrier.face, carrier.plan_charge)?;
        let lowering = super::full_lowering::preparation_bytes(&route.planned_mask.plan)?;
        let required = carrier
            .input_charge
            .checked_add(plan_scratch)
            .and_then(|n| n.checked_add(output_charge))
            .and_then(|n| n.checked_add(lowering))
            .and_then(|n| n.checked_add(core::mem::size_of::<Self>()))
            .ok_or("Full Mask preparation storage overflow")?;
        carrier.lease.admit(required)?;
        // Establish the conditional existing Plan-wire workspace proof BEFORE
        // seal verification can serialize/recompress this immutable Plan.
        profile::bounded_json_bytes(&route.planned_mask.plan, 256 * 1024)?;
        profile::bounded_json_bytes(&route, 64 * 1024)?;
        route
            .verify_seal()
            .map_err(|e| format!("Full Mask seal: {e:?}"))?;
        let inspection = RouteView {
            plan_id: route.route_plan_id.as_str().into(),
            owner_host_id: route.owner_host.host_id.as_str().into(),
            owner_boot_id: route.owner_host.boot_id.as_str().into(),
            mask_host_id: route.mask_host.host_id.as_str().into(),
            mask_boot_id: route.mask_host.boot_id.as_str().into(),
        };
        let selected = (route.face_line, route.return_line, route.interaction_line);
        let admission = Admission {
            lease: carrier.lease,
            input_charge: required,
            canonical: carrier.canonical,
        };
        Self::prepare_with_host(
            super::preparation::MaskHostContext {
                basis,
                host: None,
                full: Some(admission),
            },
            carrier.face,
            route.planned_mask,
            Some(inspection),
            Some(selected),
            sequence,
            carrier.interactions_admitted,
        )
    }
}
