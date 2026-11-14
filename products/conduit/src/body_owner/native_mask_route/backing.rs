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
