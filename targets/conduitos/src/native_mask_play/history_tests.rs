//! Actual checked native Mask kernel and compositor with retained CPU pixels.
//! Host offers, Base, placement and authority are explicitly CPU fixtures, not
//! an installed Host, Body join, physical scanout, durable recorder or human proof.
use super::{NativeMaskPlayError, PreparedNativeMaskPlay};
use crate::{
    display::{DisplayError, DisplayFormat, PixelTarget},
    native_compositor::{CompositionReceipt, CompositorAdmission, NativeCompositor},
};
use alloc::vec::Vec;
use conduit_conduitos_mask_offer::{Adapter, NativeMaskStorageProfile, prepare_stage};
use conduit_core::{HostBaseId, HostId};
use conduit_environment_history::{HISTORY_SUBJECT, HistoryGraph};
use conduit_face::{Face, FaceBasis, LayoutRect, ManifestationLifecycle};

mod common {
    use alloc::string::String;
    // Reuse the recorder's deterministic encoder fixture; no measurements are
    // reconstructed in this renderer proof. Path verified from this directory.
    include!("../../../../bodies/environment/history/tests/common/mod.rs");
}
const WIDTH: u32 = 640;
const HEIGHT: u32 = 360;
struct OwnedPixels {
    pixels: Vec<u32>,
    writes: u32,
    fail: bool,
}
impl OwnedPixels {
    fn prepare(fail: bool) -> Self {
        Self {
            pixels: vec![0; (WIDTH * HEIGHT) as usize],
            writes: 0,
            fail,
        }
    }
    fn at(&self, x: u16, y: u16) -> u32 {
        self.pixels[usize::from(y) * WIDTH as usize + usize::from(x)]
    }
    fn export_if_requested(&self, face: &Face) {
        let Some(output) = std::env::var_os("CONDUIT_HISTORY_NATIVE_PROOF_OUTPUT") else {
            return;
        };
        let output = std::path::PathBuf::from(output);
        std::fs::create_dir_all(&output).unwrap();
        let mut ppm = format!("P6\n{WIDTH} {HEIGHT}\n255\n").into_bytes();
        for pixel in &self.pixels {
            ppm.extend_from_slice(&[(pixel >> 16) as u8, (pixel >> 8) as u8, *pixel as u8]);
        }
        std::fs::write(output.join("native-history.ppm"), ppm).unwrap();
        std::fs::write(output.join("native-history.txt"), format!(
            "Synthetic fixture — not a live sensor connection\nCPU retained framebuffer from production native compositor.\nFace: {}\nRevision: {}\n{}x{}; {} successful pixel writes.\nNo physical display, Body membership, storage durability or human observation claimed.\n",
            face.identity.as_str(), face.revision, WIDTH, HEIGHT, self.writes)).unwrap();
    }
}
impl PixelTarget for OwnedPixels {
    fn format(&self) -> DisplayFormat {
        DisplayFormat {
            width: WIDTH,
            height: HEIGHT,
            pitch: WIDTH * 4,
            bits_per_pixel: 32,
            red_shift: 16,
            green_shift: 8,
            blue_shift: 0,
        }
    }
    fn write_pixel(&mut self, x: u32, y: u32, pixel: u32) -> Result<(), DisplayError> {
        if self.fail {
            return Err(DisplayError::Lost);
        }
        if x >= WIDTH || y >= HEIGHT {
            return Err(DisplayError::InvalidExtent);
        }
        self.pixels[(y * WIDTH + x) as usize] = pixel;
        self.writes += 1;
        Ok(())
    }
}
fn history_face(revision: u64) -> Face {
    let mut history = common::prepared(1);
    history
        .read_next(&common::chunk(&[
            (1, 1000, true),
            (2, 2000, false),
            (4, 11000, true),
        ]))
        .unwrap();
    let fragment = history.contribution(common::basis()).unwrap();
    Face::new_with_semantics(
        revision,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/native-history-cpu-fixture".into()),
            checked_plot_id: Some(common::basis().checked_plot_id),
            expanded_plot_id: Some("expanded/native-history-cpu-fixture".into()),
            plan_id: Some(common::basis().plan_id),
            active_play_id: Some(common::basis().active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        vec![],
        vec![],
        vec![],
    )
    .unwrap()
}
fn fixture(revision: u64) -> (Face, PreparedNativeMaskPlay, NativeCompositor) {
    // Existing canonical checked source/planner and Native Back definitions.
    // None is the existing explicit fixture Base, not discovered display custody.
    let stage = prepare_stage(
        Adapter::Native,
        &HostId::from("host/native-history-cpu-fixture"),
        &"boot/native-history-cpu-fixture".into(),
        1,
        None,
    )
    .unwrap();
    let face = history_face(revision);
    let pending = PreparedNativeMaskPlay::prepare_with_profile(
        &stage.planned_mask,
        &face,
        1,
        HISTORY_SUBJECT,
        "surface/native-history-cpu-fixture",
        HostBaseId::from("display/native-history-cpu-fixture"),
        NativeMaskStorageProfile::Standard,
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
            vec![show.placement_id.clone()],
            vec![show.target_subject.clone()],
        )
        .unwrap(),
    );
    compositor
        .admit_surface(
            &show.target_subject,
            LayoutRect {
                x: 0,
                y: 0,
                width: WIDTH as u16,
                height: HEIGHT as u16,
            },
            0,
        )
        .unwrap();
    (face, pending, compositor)
}
fn stage(
    pending: &PreparedNativeMaskPlay,
    compositor: &mut NativeCompositor,
) -> CompositionReceipt {
    let request = pending.renderer_request();
    let graph = HistoryGraph::prepare(request.face(), HISTORY_SUBJECT).unwrap();
    let scene = graph.native_scene().unwrap();
    compositor
        .update_pending_mask_surface(
            request.face(),
            request.prepared_show(),
            request.display_base_id(),
            &scene,
        )
        .unwrap()
        .clone()
}
#[test]
fn history_dots_reach_retained_pixels_before_exact_native_show_kernel_completion() {
    let (face, pending, mut compositor) = fixture(1);
    let original = face.clone();
    assert_eq!(
        pending.renderer_request().prepared_show().show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    let staged = stage(&pending, &mut compositor);
    assert!(
        compositor
            .scanout_acknowledgement(&staged, face.revision)
            .is_none()
    );
    assert!(
        compositor
            .focus_surface("surface/native-history-cpu-fixture")
            .is_err()
    );
    let graph = HistoryGraph::prepare(&face, HISTORY_SUBJECT).unwrap();
    let mut pixels = OwnedPixels::prepare(false);
    let frame = compositor.compose_frame(&mut pixels).unwrap();
    assert_eq!(pixels.writes, WIDTH * HEIGHT);
    assert_eq!(frame.pixels_written, WIDTH * HEIGHT);
    assert_eq!(frame.surfaces_composed, 1);
    let first = &graph.samples()[0];
    let failed = &graph.samples()[1];
    let last = &graph.samples()[2];
    let y = graph.y(first.temperature_milli_c().unwrap(), false) + 1;
    let point = pixels.at(graph.x(first) + 1, y);
    let blank = pixels.at(graph.x(failed) + 1, y);
    assert_ne!(
        point, blank,
        "failed acquisition must remain absent, not plotted as a reading"
    );
    assert_eq!(pixels.at(graph.x(last) + 1, y), point);
    assert_eq!(
        pixels.at(300, y),
        blank,
        "no connecting line crosses missing history"
    );
    let humidity_y = graph.y(first.humidity_milli_percent().unwrap(), true) + 1;
    assert_eq!(pixels.at(graph.x(first) + 1, humidity_y), point);
    assert_eq!(pixels.at(graph.x(failed) + 1, humidity_y), blank);
    assert_eq!(last.gap_before(), Some((3, 3)));
    assert!(
        compositor
            .scanout_acknowledgement(&staged, face.revision + 1)
            .is_none()
    );
    let ack = compositor
        .scanout_acknowledgement(&staged, face.revision)
        .unwrap();
    assert_eq!(ack.frame_sequence(), frame.frame_sequence);
    let session = pending.complete_render(&ack).unwrap();
    assert_eq!(
        session.show().show.lifecycle,
        ManifestationLifecycle::Available
    );
    assert_eq!(session.render_receipt().face_id, face.identity.as_str());
    assert_eq!(session.render_receipt().face_revision, face.revision);
    assert_eq!(
        session.render_receipt().scanout_pixels_written,
        pixels.writes
    );
    assert!(session.render_receipt().kernel_signs > 0);
    let receipt = session.close_without_input().unwrap();
    assert_eq!(receipt.face_id, staged.face_id.as_str());
    assert_eq!(receipt.mask_plan_id, staged.plan_id);
    assert_eq!(receipt.active_play_id, staged.active_play_id);
    assert_eq!(face, original);
    pixels.export_if_requested(&face);
}
#[test]
fn display_loss_has_no_ack_or_available_show_and_cancels_pending_execution() {
    let (face, pending, mut compositor) = fixture(1);
    let staged = stage(&pending, &mut compositor);
    let mut pixels = OwnedPixels::prepare(true);
    assert!(compositor.compose_frame(&mut pixels).is_err());
    assert_eq!(pixels.writes, 0);
    assert!(
        compositor
            .scanout_acknowledgement(&staged, face.revision)
            .is_none()
    );
    assert!(
        compositor
            .focus_surface("surface/native-history-cpu-fixture")
            .is_err()
    );
    assert_eq!(
        pending.renderer_request().prepared_show().show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    assert_eq!(pending.fail(), NativeMaskPlayError::RendererFailed);
}
#[test]
fn scanout_of_another_face_revision_cannot_complete_the_pending_history_show() {
    let (face, pending, mut compositor) = fixture(1);
    let staged = stage(&pending, &mut compositor);
    compositor
        .compose_frame(&mut OwnedPixels::prepare(false))
        .unwrap();
    let ack = compositor
        .scanout_acknowledgement(&staged, face.revision)
        .unwrap();
    let (_, wrong_revision, _) = fixture(2);
    assert!(matches!(
        wrong_revision.complete_render(&ack),
        Err(NativeMaskPlayError::RendererMismatch)
    ));
    pending
        .complete_render(&ack)
        .unwrap()
        .close_without_input()
        .unwrap();
}
