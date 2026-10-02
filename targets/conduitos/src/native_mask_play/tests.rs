use super::*;
use crate::display::{DisplayError, DisplayFormat, PixelTarget};
use crate::native_compositor::{CompositorAdmission, NativeCompositor};
use conduit_core::{HostBaseId, HostId, SignId};
use conduit_presentation::{
    GraphicsScene, LayoutRect, PresentationBasis, PresentationRole, PresentationSubject,
};

struct Display {
    writes: u32,
    fail: bool,
}
impl PixelTarget for Display {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: 16,
            height: 16,
            pitch: 64,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, _: u32, _: u32, _: u32) -> Result<(), DisplayError> {
        if self.fail {
            return Err(DisplayError::Lost);
        }
        self.writes += 1;
        Ok(())
    }
}

fn fixture(sequence: u64) -> (PreparedNativeMaskPlay, NativeCompositor) {
    let stage = crate::mask_control::prepare_stage(
        crate::mask_control::Adapter::Native,
        &HostId::from("host/native-mask"),
        &"boot/native-mask".into(),
        1,
        "surface/native-mask",
        None,
    )
    .unwrap();
    let plan = &stage.planned_mask.plan;
    let body = conduit_body::Body::born(
        plan.source_document_id.clone(),
        plan.checked_plot_id.clone(),
        1,
        SignId::from("born/native-mask"),
    )
    .unwrap();
    let (body, wake) = body.wake(1, SignId::from("wake/native-mask")).unwrap();
    let presentation = Presentation::new(
        1,
        PresentationBasis {
            body_id: Some(body.body_id),
            wake_id: Some(wake.wake_id),
            source_document_id: Some(plan.source_document_id.clone()),
            checked_plot_id: Some(plan.checked_plot_id.clone()),
            expanded_plot_id: Some(plan.expanded_plot_id.clone()),
            plan_id: Some(plan.plan_id.clone()),
            active_play_id: None,
            sign_ids: alloc::vec![SignId::from("face/native-mask")],
        },
        alloc::vec![PresentationSubject {
            identity: "front/native-mask".into(),
            role: PresentationRole::Document,
            name: "Native Mask".into()
        }],
        Vec::new(),
        Vec::new(),
        Vec::new(),
    )
    .unwrap();
    let pending = PreparedNativeMaskPlay::prepare(
        &stage.planned_mask,
        &presentation,
        sequence,
        "front/native-mask",
        "surface/native-mask",
        HostBaseId::from("display/native-mask"),
    )
    .unwrap();
    let request = pending.renderer_request();
    let show = &request.prepared_show().show;
    let mut compositor = NativeCompositor::admitted(
        CompositorAdmission::new(
            show.host_id.clone(),
            show.boot_id.clone(),
            show.offer_generation,
            show.presenter_implementation_id.clone(),
            request.display_base_id().clone(),
            alloc::vec![show.placement_id.clone()],
            alloc::vec![show.target_subject.clone()],
        )
        .unwrap(),
    );
    compositor
        .admit_surface(
            &show.target_subject,
            LayoutRect {
                x: 0,
                y: 0,
                width: 16,
                height: 16,
            },
            0,
        )
        .unwrap();
    (pending, compositor)
}

fn stage(
    pending: &PreparedNativeMaskPlay,
    compositor: &mut NativeCompositor,
) -> crate::native_compositor::CompositionReceipt {
    let request = pending.renderer_request();
    compositor
        .update_pending_mask_surface(
            request.presentation(),
            request.prepared_show(),
            request.display_base_id(),
            &GraphicsScene::empty(),
        )
        .unwrap()
        .clone()
}

#[test]
fn renderer_waits_for_exact_scanout_before_delivering_its_show_fore() {
    let (pending, mut compositor) = fixture(1);
    let request = pending.renderer_request();
    assert_eq!(
        run(
            &request.prepared_show().planned_mask,
            request.presentation(),
            1
        ),
        Err(NativeMaskPlayError::PendingRenderer)
    );
    let staged = stage(&pending, &mut compositor);
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
    assert!(compositor.focus_surface("surface/native-mask").is_err());
    let mut display = Display {
        writes: 0,
        fail: false,
    };
    let frame = compositor.compose_frame(&mut display).unwrap();
    assert_eq!(display.writes, 256);
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    assert_eq!(ack.frame_sequence(), frame.frame_sequence);
    let receipt = pending.complete(&ack).unwrap();
    assert_eq!(receipt.mask_plan_id, staged.plan_id);
    assert_eq!(receipt.active_play_id, staged.active_play_id);
    assert_eq!(receipt.presentation_id, staged.presentation_id.as_str());
    assert!(receipt.kernel_signs > 0);
    compositor.focus_surface("surface/native-mask").unwrap();
}

#[test]
fn other_play_scanout_cannot_complete_pending_renderer() {
    let (other, mut compositor) = fixture(2);
    let staged = stage(&other, &mut compositor);
    compositor
        .compose_frame(&mut Display {
            writes: 0,
            fail: false,
        })
        .unwrap();
    let ack = compositor.scanout_acknowledgement(&staged, 1).unwrap();
    let (pending, _) = fixture(1);
    assert_eq!(
        pending.complete(&ack),
        Err(NativeMaskPlayError::RendererMismatch)
    );
    other.cancel().unwrap();
}

#[test]
fn display_failure_has_no_ack_and_failed_or_cancelled_execution_returns_no_show() {
    let (pending, mut compositor) = fixture(1);
    let staged = stage(&pending, &mut compositor);
    assert!(
        compositor
            .compose_frame(&mut Display {
                writes: 0,
                fail: true
            })
            .is_err()
    );
    assert!(compositor.scanout_acknowledgement(&staged, 1).is_none());
    assert!(compositor.focus_surface("surface/native-mask").is_err());
    assert_eq!(pending.fail(), NativeMaskPlayError::RendererFailed);
    let (pending, _) = fixture(1);
    pending.cancel().unwrap();
}
