//! Complete cold per-invocation managed metadata, separate from fixed stores.
use super::*;
use conduit_face::MaskShow;

fn add(total: &mut usize, value: usize) -> Option<()> {
    *total = total.checked_add(value)?;
    Some(())
}
fn plot_identity_bytes(identity: &conduit_core::PlotIdentity) -> Option<usize> {
    let conduit_core::PlotIdentity {
        source_document_id,
        checked_plot_id,
        expanded_plot_id,
    } = identity;
    source_document_id
        .0
        .capacity()
        .checked_add(checked_plot_id.0.capacity())?
        .checked_add(expanded_plot_id.0.capacity())
}
fn show_bytes(show: &MaskShow) -> Option<usize> {
    let MaskShow {
        show_id,
        mask_plot,
        face_plot,
        face_plan_id,
        face_id,
        face_revision: _,
        planned_mask,
        show,
    } = show;
    let mut total = show_id.retained_backing_bytes();
    add(&mut total, plot_identity_bytes(mask_plot)?)?;
    if let Some(identity) = face_plot {
        add(&mut total, plot_identity_bytes(identity)?)?;
    }
    if let Some(plan) = face_plan_id {
        add(&mut total, plan.0.capacity())?;
    }
    add(&mut total, face_id.retained_backing_bytes())?;
    add(
        &mut total,
        super::owned_backing::native_mask_plan_backing_bytes(planned_mask)?,
    )?;
    add(&mut total, show.retained_backing_bytes()?)?;
    Some(total)
}
impl NativeMaskPlayReceipt {
    /// All owned heap payload of this receipt; fixed Sign evidence is inline.
    pub fn retained_backing_bytes(&self) -> Option<usize> {
        let Self {
            mask_plan_id,
            active_play_id,
            face_id,
            face_revision: _,
            renderer_input_bytes: _,
            renderer_show_value_bytes: _,
            show_value_id,
            scanout_frame_sequence: _,
            scanout_pixels_written: _,
            display_base_id,
            surface_id,
            kernel_signs: _,
            fore_endpoints: _,
            collection_storage: _,
        } = self;
        let mut total = 0;
        for capacity in [
            mask_plan_id.0.capacity(),
            active_play_id.0.capacity(),
            face_id.capacity(),
            show_value_id.capacity(),
            display_base_id.0.capacity(),
            surface_id.capacity(),
        ] {
            add(&mut total, capacity)?;
        }
        Some(total)
    }
}
impl ColdNativeMaskInvocation {
    /// Actual cold metadata highwater before the first kernel Step. Excludes
    /// fixed scheduler/value/Sign stores already owned by the selected Base.
    /// Unsupported Plan owners refuse rather than provide a partial receipt.
    pub fn retained_metadata_bytes(&self) -> Option<usize> {
        let parts = self.parts.as_ref()?;
        let mut total = parts.renderer.face().retained_backing_bytes()?;
        add(&mut total, show_bytes(parts.renderer.prepared_show())?)?;
        add(&mut total, parts.renderer.display_base_id().0.capacity())?;
        add(&mut total, parts.face_bytes.capacity())?;
        add(&mut total, parts.show_bytes.capacity())?;
        for port in [&parts.face_fore, &parts.show_fore, &parts.interaction_fore] {
            add(
                &mut total,
                super::owned_backing::lowered_fore_backing_bytes(port)?,
            )?;
        }
        if let Some(show) = &parts.available_show {
            add(&mut total, show_bytes(show)?)?;
        }
        if let Some(receipt) = &parts.receipt {
            add(&mut total, receipt.retained_backing_bytes()?)?;
        }
        add(&mut total, parts.scheduler.retained_metadata_bytes()?)?;
        Some(total)
    }
}
