//! Actual owned payload of the ordinary retained CPU compositor.
//!
//! Expression identity storage is counted; configured Look owners remain unsupported.
//! This does not account allocator overhead, stack, GPU/SDL storage, or peak
//! preparation workspace. A caller must not add the fixed raster receipt twice.
use super::*;
use core::mem::size_of;

struct Bytes(usize);
impl Bytes {
    fn add(&mut self, bytes: usize) -> Option<()> {
        self.0 = self.0.checked_add(bytes)?;
        Some(())
    }
    fn vector<T>(&mut self, values: &Vec<T>) -> Option<()> {
        self.add(values.capacity().checked_mul(size_of::<T>())?)
    }
}

impl CompositionReceipt {
    /// Exact heap payload for a presentation receipt, including its expression.
    /// Inline fields are excluded; configured Look owners refuse explicitly.
    pub fn ordinary_retained_backing_bytes(&self) -> Option<usize> {
        if self.visual_look.is_some() {
            return None;
        }
        let Self {
            face_id,
            manifestation_id,
            plan_id,
            active_play_id,
            play_sequence: _,
            placement_id,
            host_id,
            boot_id,
            offer_generation: _,
            presenter_implementation_id,
            presenter_capability_id,
            presenter_artifact_id,
            front_subject,
            display_base_id,
            surface_id,
            display: _,
            visual_expression,
            visual_look: _,
        } = self;
        let mut bytes = Bytes(0);
        for capacity in [
            face_id.retained_backing_bytes(),
            manifestation_id.retained_backing_bytes(),
            plan_id.0.capacity(),
            active_play_id.0.capacity(),
            placement_id.0.capacity(),
            host_id.0.capacity(),
            boot_id.0.capacity(),
            presenter_implementation_id.0.capacity(),
            presenter_capability_id.0.capacity(),
            presenter_artifact_id.0.capacity(),
            front_subject.capacity(),
            display_base_id.0.capacity(),
            surface_id.capacity(),
        ] {
            bytes.add(capacity)?;
        }
        if let Some(expression) = visual_expression {
            bytes.add(expression.expression.identity.capacity())?;
        }
        Some(bytes.0)
    }
}

impl NativeCompositor {
    /// Exact retained heap payload of the ordinary CPU compositor, including
    /// its actual allocated raster buffers and Vec capacities. This is a scoped
    /// owner receipt, not whole-Root or preparation-peak evidence.
    pub fn ordinary_retained_backing_bytes(&self) -> Option<usize> {
        let Self {
            admission,
            surfaces,
            focused_surface,
            cursor: _,
            cursor_hover: _,
            frame_sequence: _,
            scanout_pixels: _,
            admitted_pixels: _,
            buffer_pool,
            damage: _,
        } = self;
        let CompositorAdmission {
            host_id,
            boot_id,
            offer_generation: _,
            presenter_implementation_id,
            display_base_id,
            placement_ids,
            surface_ids,
        } = admission;
        let mut bytes = Bytes(0);
        for capacity in [
            host_id.0.capacity(),
            boot_id.0.capacity(),
            presenter_implementation_id.0.capacity(),
            display_base_id.0.capacity(),
        ] {
            bytes.add(capacity)?;
        }
        bytes.vector(placement_ids)?;
        for id in placement_ids {
            bytes.add(id.0.capacity())?;
        }
        bytes.vector(surface_ids)?;
        for id in surface_ids {
            bytes.add(id.capacity())?;
        }
        bytes.vector(surfaces)?;
        for surface in surfaces {
            let CompositorSurface {
                surface_id,
                bounds: _,
                z: _,
                visible: _,
                buffer,
                raster_scene: _,
                visual_expression,
                visual_look,
                binding,
                receipt,
                input_ready: _,
            } = surface;
            if visual_look.is_some() {
                return None;
            }
            if let Some(expression) = visual_expression {
                bytes.add(expression.expression.identity.capacity())?;
            }
            bytes.add(surface_id.capacity())?;
            bytes.vector(&buffer.pixels)?;
            if let Some(binding) = binding {
                let SurfaceBinding {
                    manifestation_id,
                    plan_id,
                    placement_id,
                    front_subject,
                    last_revision: _,
                } = binding;
                for capacity in [
                    manifestation_id.retained_backing_bytes(),
                    plan_id.0.capacity(),
                    placement_id.0.capacity(),
                    front_subject.capacity(),
                ] {
                    bytes.add(capacity)?;
                }
            }
            if let Some(receipt) = receipt {
                bytes.add(receipt.ordinary_retained_backing_bytes()?)?;
            }
        }
        bytes.add(buffer_pool.retained_backing_bytes()?)?;
        if let Some(focused) = focused_surface {
            bytes.add(focused.capacity())?;
        }
        Some(bytes.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn actual_vector_capacities_and_identity_payload_survive_raster_binding() {
        let raster = PreparedCompositorBacking::prepare("surface", 8, 8, usize::MAX).unwrap();
        let fixed = raster.backing_bytes();
        let admission = CompositorAdmission::new(
            "host".into(),
            "boot".into(),
            conduit_core::OfferGeneration(1),
            "renderer".into(),
            "display".into(),
            alloc::vec!["placement".into()],
            alloc::vec!["surface".into()],
        )
        .unwrap();
        let mut compositor = raster.bind(admission).unwrap();
        let before = compositor.ordinary_retained_backing_bytes().unwrap();
        assert!(before > fixed);
        let previous = compositor.surfaces[0].surface_id.capacity();
        compositor.surfaces[0].surface_id.reserve_exact(300);
        let added = compositor.surfaces[0].surface_id.capacity() - previous;
        assert_eq!(
            compositor.ordinary_retained_backing_bytes(),
            Some(before + added)
        );
    }

    #[test]
    fn expression_identity_counts_allocated_capacity_on_each_surface() {
        let raster = PreparedCompositorBacking::prepare("surface", 8, 8, usize::MAX).unwrap();
        let admission = CompositorAdmission::new(
            "host".into(),
            "boot".into(),
            conduit_core::OfferGeneration(1),
            "renderer".into(),
            "display".into(),
            alloc::vec!["placement".into()],
            alloc::vec!["surface".into()],
        )
        .unwrap();
        let mut compositor = raster.bind(admission).unwrap();
        let before = compositor.ordinary_retained_backing_bytes().unwrap();
        let mut expression = conduit_face::admit_visual_expression(
            Ok(conduit_face::VisualExpression::phosphor()),
            conduit_face::VisualUserConstraints {
                forced_colors: false,
                reduced_motion: true,
                minimum_text_scale_percent: 150,
                maximum_supported_text_scale_percent: 200,
            },
        )
        .unwrap();
        expression.expression.identity.reserve_exact(257);
        let capacity = expression.expression.identity.capacity();
        compositor.surfaces[0].visual_expression = Some(expression);
        assert_eq!(
            compositor.ordinary_retained_backing_bytes(),
            Some(before + capacity)
        );
        compositor.surfaces[0].visual_expression = None;
        assert_eq!(compositor.ordinary_retained_backing_bytes(), Some(before));
    }
}
