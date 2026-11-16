//! Actual retained native presentation payloads, never wire-size estimates.
//!
//! These visitors fund a closed immutable clone, not the preceding planner,
//! Host advertisement, carrier buffers, wardrobe or a whole presentation slot.
//! Unsupported Plan owners refuse before IMAGE reservation or cloning.
use conduit_core::*;
use conduit_face::{FaceBasis, MaskShow, RemoteMaskHostBasis, RemoteOwnerMaskRouteSeal};
use conduit_std_host::{body_execution::ImagePreparedValue, StdHost};
use std::mem::size_of;

#[derive(Default)]
struct Bytes(usize);
impl Bytes {
    fn add(&mut self, bytes: usize) -> Option<()> {
        self.0 = self.0.checked_add(bytes)?;
        Some(())
    }
    fn strings(&mut self, values: &[&String]) -> Option<()> {
        for value in values {
            self.add(value.capacity())?;
        }
        Some(())
    }
    fn basis(&mut self, basis: &FaceBasis) -> Option<()> {
        let FaceBasis {
            body_id,
            wake_id,
            source_document_id,
            checked_plot_id,
            expanded_plot_id,
            plan_id,
            active_play_id,
            sign_ids,
        } = basis;
        self.add(body_id.as_ref().map_or(0, |v| v.retained_backing_bytes()))?;
        self.add(wake_id.as_ref().map_or(0, |v| v.retained_backing_bytes()))?;
        for value in [
            source_document_id.as_ref().map(|v| &v.0),
            checked_plot_id.as_ref().map(|v| &v.0),
            expanded_plot_id.as_ref().map(|v| &v.0),
            plan_id.as_ref().map(|v| &v.0),
            active_play_id.as_ref().map(|v| &v.0),
        ]
        .into_iter()
        .flatten()
        {
            self.add(value.capacity())?;
        }
        self.add(sign_ids.capacity().checked_mul(size_of::<SignId>())?)?;
        for value in sign_ids {
            self.add(value.0.capacity())?;
        }
        Some(())
    }
    fn host_basis(&mut self, basis: &RemoteMaskHostBasis) -> Option<()> {
        let RemoteMaskHostBasis {
            host_id,
            boot_id,
            offer_generation: _,
            offer_digest,
        } = basis;
        self.strings(&[&host_id.0, &boot_id.0, offer_digest])
    }
    fn endpoint(&mut self, endpoint: &LinkEndpoint) -> Option<()> {
        let LinkEndpoint {
            host_id,
            boot_id,
            endpoint_id,
        } = endpoint;
        self.strings(&[&host_id.0, &boot_id.0, &endpoint_id.0])
    }
    fn line(&mut self, line: &AdmittedLine) -> Option<()> {
        let AdmittedLine {
            line_id,
            binding,
            contract: _,
        } = line;
        let BoundLink {
            binding_id,
            source,
            sink,
            base,
            base_instance_id,
            credential,
            authority,
            limits: _,
        } = binding;
        self.strings(&[&line_id.0, &binding_id.0, &base.0, &base_instance_id.0])?;
        self.endpoint(source)?;
        self.endpoint(sink)?;
        if let LinkCredentialReference::Opaque(value) = credential {
            self.add(value.0.capacity())?;
        }
        if let LinkAuthorityReference::Grant(value) = authority {
            self.add(value.0.capacity())?;
        }
        Some(())
    }
    fn plot(&mut self, plot: &PlotIdentity) -> Option<()> {
        let PlotIdentity {
            source_document_id,
            checked_plot_id,
            expanded_plot_id,
        } = plot;
        self.strings(&[
            &source_document_id.0,
            &checked_plot_id.0,
            &expanded_plot_id.0,
        ])
    }
}

/// Nested owned allocations only, excluding inline Self and independently owned
/// inputs/allocator bookkeeping. Counts spare capacities, including empty Vecs.
pub(in crate::durable_host::owner::controller) fn seal_payload_bytes(
    seal: &RemoteOwnerMaskRouteSeal,
) -> Option<usize> {
    let RemoteOwnerMaskRouteSeal {
        route_plan_id,
        profile: _,
        body_id,
        workload_revision: _,
        face_id,
        face_revision: _,
        face_basis,
        owner_host,
        mask_host,
        planned_mask,
        face_line,
        return_line,
        interaction_line,
    } = seal;
    let mut n = Bytes::default();
    n.add(route_plan_id.0.capacity())?;
    n.add(body_id.retained_backing_bytes())?;
    n.add(face_id.retained_backing_bytes())?;
    n.basis(face_basis)?;
    n.host_basis(owner_host)?;
    n.host_basis(mask_host)?;
    n.add(conduit_std_host::native_mask_plan_backing_bytes(
        planned_mask,
    )?)?;
    n.line(face_line)?;
    n.line(return_line)?;
    if let Some(line) = interaction_line {
        n.line(line)?;
    }
    Some(n.0)
}

/// Acknowledged Show payload, including its separately owned Plan/manifestation.
/// Neither the Show nor its backing conveys execution or interaction authority.
#[allow(dead_code)]
pub(in crate::durable_host::owner::controller) fn show_payload_bytes(
    shown: &MaskShow,
) -> Option<usize> {
    let MaskShow {
        show_id,
        mask_plot,
        face_plot,
        face_plan_id,
        face_id,
        face_revision: _,
        planned_mask,
        show,
    } = shown;
    let mut n = Bytes::default();
    n.add(show_id.retained_backing_bytes())?;
    n.plot(mask_plot)?;
    if let Some(plot) = face_plot {
        n.plot(plot)?;
    }
    if let Some(plan) = face_plan_id {
        n.add(plan.0.capacity())?;
    }
    n.add(face_id.retained_backing_bytes())?;
    n.add(conduit_std_host::native_mask_plan_backing_bytes(
        planned_mask,
    )?)?;
    n.add(show.retained_backing_bytes()?)?;
    Some(n.0)
}

/// Clone one already prepared native seal on the SAME original IMAGE counter.
/// This is only immutable clone custody; initial planning and the borrowed input
/// remain caller-owned. No route is published or membership/Show accepted here.
#[allow(dead_code)]
pub(in crate::durable_host::owner::controller) fn prepare_seal_clone(
    host: &mut StdHost,
    seal: &RemoteOwnerMaskRouteSeal,
) -> Result<ImagePreparedValue<RemoteOwnerMaskRouteSeal>, String> {
    let bytes = size_of::<RemoteOwnerMaskRouteSeal>()
        .checked_add(seal_payload_bytes(seal).ok_or("unsupported native seal backing")?)
        .ok_or("native seal backing overflow")?;
    // SAFETY: the sole factory operation is derived Clone. Every supported
    // descendant's owned payload/spare capacity is visited above or by the closed
    // native Plan visitor. Clone allocates its result only, no planner/codec or
    // async effects. Source remains borrowed and independently funded. The
    // immutable result exposes no mutation that can grow this retained envelope.
    unsafe { host.prepare_selected_image_value(bytes, bytes, || Ok(seal.clone())) }
}

/// Closed owner-Plan payload. Local/speech child Plans are deliberately not
/// part of the native-viewer factory and must obtain their own qualification.
pub(in crate::durable_host::owner::controller) fn owner_plan_payload_bytes(
    plan: &conduit_face::OwnerFacePlan,
) -> Option<usize> {
    let conduit_face::OwnerFacePlan {
        plan_id,
        body_id,
        workload_revision: _,
        face_id,
        face_revision: _,
        face_basis,
        routes,
    } = plan;
    let mut n = Bytes::default();
    n.add(plan_id.0.capacity())?;
    n.add(body_id.retained_backing_bytes())?;
    n.add(face_id.retained_backing_bytes())?;
    n.basis(face_basis)?;
    n.add(
        routes
            .capacity()
            .checked_mul(size_of::<conduit_face::OwnerFaceChildRoute>())?,
    )?;
    for route in routes {
        match route {
            conduit_face::OwnerFaceChildRoute::Remote { seal } => {
                n.add(size_of::<RemoteOwnerMaskRouteSeal>())?;
                n.add(seal_payload_bytes(seal)?)?;
            }
            conduit_face::OwnerFaceChildRoute::Local { .. } => return None,
        }
    }
    Some(n.0)
}

pub(in crate::durable_host::owner::controller) fn selected_route_payload_bytes(
    route: &conduit_face::SelectedMaskPlotRoute,
) -> Option<usize> {
    let conduit_face::SelectedMaskPlotRoute {
        route_id,
        mask_plot,
        plan_id,
    } = route;
    let mut n = Bytes::default();
    n.add(route_id.capacity())?;
    n.plot(mask_plot)?;
    n.add(plan_id.0.capacity())?;
    Some(n.0)
}

pub(in crate::durable_host::owner::controller) fn wardrobe_control_payload_bytes(
    control: &conduit_face::MaskWardrobeControl,
) -> Option<usize> {
    let conduit_face::MaskWardrobeControl {
        scoped_wardrobe,
        active_plan_id,
        selected,
    } = control;
    let conduit_face::BodyMaskWardrobe {
        body_id,
        wake_id,
        wardrobe,
    } = scoped_wardrobe;
    let conduit_face::MaskWardrobe {
        revision: _,
        lifetime: _,
        worn,
        preference,
    } = wardrobe;
    let mut n = Bytes::default();
    n.add(body_id.retained_backing_bytes())?;
    if let Some(wake) = wake_id {
        n.add(wake.retained_backing_bytes())?;
    }
    for plots in [worn, preference] {
        n.add(plots.capacity().checked_mul(size_of::<PlotIdentity>())?)?;
        for plot in plots {
            n.plot(plot)?;
        }
    }
    n.add(active_plan_id.0.capacity())?;
    if let Some(selected) = selected {
        n.add(selected_route_payload_bytes(selected)?)?;
    }
    Some(n.0)
}

/// Closed ordinary core verifier scratch for the single-fragment four-Gear
/// native graph. Empty State/regions/fusions/pools/activations allocate no
/// descendants. The normal verifier is still invoked after this admission.
pub(in crate::durable_host::owner::controller) fn native_verification_workspace(
    mask: &conduit_face::PlannedMaskPlot,
) -> Option<usize> {
    conduit_face::native_mask_plan_backing_bytes(mask)?;
    let [fragment] = mask.plan.fragments.as_slice() else {
        return None;
    };
    if fragment.placements.len() != 4
        || fragment.connections.len() != 4
        || fragment.fore_ports.len() != 3
        || fragment.plan_fragments.len() != 1
    {
        return None;
    }
    let mut n = Bytes::default();
    // verify_plan clones the Plot identity and top commitment; fragment
    // verification owns a second commitment clone. Digest outputs are64B each.
    n.plot(&mask.mask.plot_identity)?;
    for _ in 0..2 {
        n.add(size_of::<FragmentCommitment>())?;
        n.strings(&[&fragment.host_id.0, &fragment.fragment_id.0])?;
        n.add(64)?;
    }
    // Shared-pool placement references, all Cords, then the one-Cord occurrence,
    // source and sink filter Vecs. Nonexact collect starts at capacity4; these
    // closed sets never exceed4 and hence never reallocate.
    for _ in [
        "shared placements",
        "all Cords",
        "Cord occurrences",
        "source",
        "sink",
    ] {
        n.add(4usize.checked_mul(size_of::<&PlannedGear>())?)?;
    }
    Some(n.0)
}

pub(in crate::durable_host::owner::controller) fn basis_payload_bytes(
    basis: &FaceBasis,
) -> Option<usize> {
    let mut n = Bytes::default();
    n.basis(basis)?;
    Some(n.0)
}
pub(in crate::durable_host::owner::controller) fn plot_payload_bytes(
    plot: &PlotIdentity,
) -> Option<usize> {
    let mut n = Bytes::default();
    n.plot(plot)?;
    Some(n.0)
}
pub(in crate::durable_host::owner::controller) fn line_offer_payload_bytes(
    line: &LineOffer,
) -> Option<usize> {
    let mut n = Bytes::default();
    n.strings(&[
        &line.line_id.0,
        &line.availability.line_id.0,
        &line.availability.binding_id.0,
        &line.availability.sign_id.0,
    ])?;
    let binding = &line.binding;
    n.strings(&[
        &binding.binding_id.0,
        &binding.base.0,
        &binding.base_instance_id.0,
    ])?;
    n.endpoint(&binding.source)?;
    n.endpoint(&binding.sink)?;
    if let LinkCredentialReference::Opaque(value) = &binding.credential {
        n.add(value.0.capacity())?;
    }
    if let LinkAuthorityReference::Grant(value) = &binding.authority {
        n.add(value.0.capacity())?;
    }
    Some(n.0)
}
pub(in crate::durable_host::owner::controller) fn admitted_line_payload_bytes(
    line: &LineOffer,
) -> Option<usize> {
    let total = line_offer_payload_bytes(line)?;
    total
        .checked_sub(line.availability.line_id.0.capacity())?
        .checked_sub(line.availability.binding_id.0.capacity())?
        .checked_sub(line.availability.sign_id.0.capacity())
}

impl super::super::face_wardrobe::OwnerFaceWardrobe {
    pub(super) fn retain_native_memory(
        &mut self,
        memory: conduit_std_host::body_execution::ImageMemoryRetention,
    ) {
        self._memory = Some(memory);
    }
    pub(super) fn retain_native_show_memory(
        &mut self,
        memory: conduit_std_host::body_execution::ImageMemoryRetention,
    ) {
        self._show_memory = Some(memory);
    }

    /// Actual closed native-view payload, including each independently owned
    /// Plan and Show. Rejects unsupported local/speech child owners; this
    /// allocation-free receipt grants neither selection nor IMAGE capacity.
    pub(super) fn retained_backing_bytes(&self) -> Option<usize> {
        let Self {
            plan,
            control,
            acknowledged,
            _show_memory: _,
            _memory: _,
        } = self;
        let mut bytes = owner_plan_payload_bytes(plan)?
            .checked_add(wardrobe_control_payload_bytes(control)?)?;
        if let Some(acknowledged) = acknowledged {
            let super::super::face_wardrobe::AcknowledgedShow {
                selected,
                child_route_plan_id,
                show,
            } = acknowledged;
            bytes = bytes
                .checked_add(selected_route_payload_bytes(selected)?)?
                .checked_add(child_route_plan_id.0.capacity())?
                .checked_add(show_payload_bytes(show)?)?;
        }
        Some(bytes)
    }
}
