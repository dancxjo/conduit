//! Actual checked native Mask kernel and compositor with retained CPU pixels.
//! Host offers, Base, placement and authority are explicitly CPU fixtures, not
//! an installed Host, Body join, physical scanout, durable recorder or human proof.
use super::{NativeMaskPlayError, PreparedNativeMaskPlay};
use crate::{
    display::{DisplayError, DisplayFormat, PixelTarget},
    native_compositor::{CompositionReceipt, CompositorAdmission, NativeCompositor},
};
use alloc::vec::Vec;
use conduit_conduitos_mask_offer::{
    native_host_advertisement, prepare_history_component_stage_from_offer, NativeMaskStorageProfile,
};
use conduit_core::{HostBaseId, HostId};
use conduit_environment_history::{HistoryGraph, HISTORY_SUBJECT};
use conduit_face::{
    admit_visual_look, prepare_component_recipe, AdmittedVisualLook, Face, FaceBasis, LayoutRect,
    ManifestationLifecycle, VisualUserConstraints, HISTORY_COMPONENT_MASK_ROOT,
    HISTORY_COMPONENT_RECIPE_ID,
};

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
    if let Some(path) = std::env::var_os("CONDUIT_HISTORY_NATIVE_PROOF_FACE") {
        // Optional cross-Mask proof input is one explicitly selected sealed
        // CPU fixture Face. It confers no Body membership or storage authority.
        use std::io::Read;
        const MAXIMUM_FACE_BYTES: u64 = 64 * 1024;
        let file =
            std::fs::File::open(&path).expect("open explicitly selected cross-Mask Face fixture");
        let mut bytes = Vec::with_capacity(MAXIMUM_FACE_BYTES as usize + 1);
        file.take(MAXIMUM_FACE_BYTES + 1)
            .read_to_end(&mut bytes)
            .expect("read bounded cross-Mask Face fixture");
        assert!(
            bytes.len() <= MAXIMUM_FACE_BYTES as usize,
            "cross-Mask Face fixture exceeds the 64 KiB input bound"
        );
        let mut face: Face = serde_json::from_slice(&bytes)
            .expect("decode explicitly selected cross-Mask Face fixture");
        face.validate()
            .expect("cross-Mask Face fixture must retain its exact sealed identity");
        let offset = revision
            .checked_sub(1)
            .expect("native proof revision starts at one");
        if offset != 0 {
            face.revision = face
                .revision
                .checked_add(offset)
                .expect("cross-Mask refusal fixture revision must not overflow");
            face.reseal_prepared()
                .expect("seal changed-revision refusal fixture");
        }
        return face;
    }
    let mut history = common::prepared(1);
    history
        .read_next(&common::chunk(&[
            (1, 1000, true),
            (2, 2000, false),
            (4, 11000, true),
        ]))
        .unwrap();
    let fragment = history.contribution(common::basis()).unwrap();
    face_from_fragment(revision, fragment)
}
fn face_from_fragment(revision: u64, fragment: conduit_face::FaceFragment) -> Face {
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
fn fixture(
    revision: u64,
) -> (
    Face,
    PreparedNativeMaskPlay,
    NativeCompositor,
    AdmittedVisualLook,
) {
    fixture_face(history_face(revision))
}
fn fixture_face(
    face: Face,
) -> (
    Face,
    PreparedNativeMaskPlay,
    NativeCompositor,
    AdmittedVisualLook,
) {
    // Existing renderer-host definitions and explicit CPU surface offer. This
    // does not discover, initialize or establish custody of a physical Base.
    let provider = conduit_core::BaseProviderEntry {
        base_id: "display/native-history-cpu-fixture".into(),
        provider_instance_id: "display/native-history-cpu-fixture/provider".into(),
        provider_generation: 1,
        implementation_id: "conduitos/framebuffer@1".into(),
        mechanism_family: "conduitos.base/framebuffer@1".into(),
        enforcement_class: conduit_core::BaseEnforcementClass::ConduitOsKernelEnforced,
        lifecycle: conduit_core::BaseLifecycle::Ready,
        capabilities: Vec::new(),
        resources: vec![conduit_core::resource_offer(
            "surface/native-history-cpu-fixture",
            conduit_face::SHOW_RESOURCE_CLASS,
            1,
        )],
    };
    let host = native_host_advertisement(
        &HostId::from("host/native-history-cpu-fixture"),
        &"boot/native-history-cpu-fixture".into(),
        1,
        &provider,
    );
    let (stage, configuration) = prepare_history_component_stage_from_offer(&host).unwrap();
    assert_eq!(
        stage.planned_mask.mask.plot_name,
        HISTORY_COMPONENT_MASK_ROOT
    );
    let prepared = prepare_component_recipe(
        conduit_core::StructuredInfoValue::from_canonical_bytes(
            configuration.recipe.canonical_value(),
        )
        .unwrap(),
        &face,
    )
    .unwrap();
    assert_eq!(prepared.recipe.identity(), HISTORY_COMPONENT_RECIPE_ID);
    assert_eq!(prepared.subject_regions[0], Some(0));
    let look = admit_visual_look(
        &configuration.requested_look().unwrap(),
        VisualUserConstraints {
            forced_colors: false,
            reduced_motion: true,
            minimum_text_scale_percent: 100,
            maximum_supported_text_scale_percent: 200,
        },
    )
    .unwrap();
    assert_eq!(look.selected.identity(), "conduit.look/editorial-compact@1");
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
    (face, pending, compositor, look)
}
fn stage(
    pending: &PreparedNativeMaskPlay,
    compositor: &mut NativeCompositor,
    look: &AdmittedVisualLook,
) -> CompositionReceipt {
    let request = pending.renderer_request();
    let graph = HistoryGraph::prepare(request.face(), HISTORY_SUBJECT).unwrap();
    let scene = graph.native_scene_with_look(&look.selected).unwrap();
    compositor
        .update_pending_look_mask_surface(
            request.face(),
            request.prepared_show(),
            request.display_base_id(),
            &scene,
            look,
        )
        .unwrap()
        .clone()
}
#[test]
fn history_dots_reach_retained_pixels_before_exact_native_show_kernel_completion() {
    let (face, pending, mut compositor, look) = fixture(1);
    let original = face.clone();
    assert_eq!(pending.renderer_request().face(), &face);
    assert_eq!(
        pending.renderer_request().prepared_show().show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    let staged = stage(&pending, &mut compositor, &look);
    assert_eq!(
        staged.visual_look.as_ref().unwrap().identity(),
        look.selected.identity()
    );
    assert!(compositor
        .scanout_acknowledgement(&staged, face.revision)
        .is_none());
    assert!(compositor
        .focus_surface("surface/native-history-cpu-fixture")
        .is_err());
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
    let conduit_face::LookVisualFacetOption::Some(visual) = look.selected.visual() else {
        panic!("reviewed history Look has a visual facet")
    };
    let ink = visual.tokens().text_primary();
    let paper = visual.tokens().background();
    assert_eq!(
        point,
        pixels.format().pixel(ink.red(), ink.green(), ink.blue())
    );
    assert_eq!(
        blank,
        pixels
            .format()
            .pixel(paper.red(), paper.green(), paper.blue())
    );
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
    assert!(compositor
        .scanout_acknowledgement(&staged, face.revision + 1)
        .is_none());
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
    let (face, pending, mut compositor, look) = fixture(1);
    let staged = stage(&pending, &mut compositor, &look);
    let mut pixels = OwnedPixels::prepare(true);
    assert!(compositor.compose_frame(&mut pixels).is_err());
    assert_eq!(pixels.writes, 0);
    assert!(compositor
        .scanout_acknowledgement(&staged, face.revision)
        .is_none());
    assert!(compositor
        .focus_surface("surface/native-history-cpu-fixture")
        .is_err());
    assert_eq!(
        pending.renderer_request().prepared_show().show.lifecycle,
        ManifestationLifecycle::Prepared
    );
    assert_eq!(pending.fail(), NativeMaskPlayError::RendererFailed);
}
#[test]
fn scanout_of_another_face_revision_cannot_complete_the_pending_history_show() {
    let (face, pending, mut compositor, look) = fixture(1);
    let staged = stage(&pending, &mut compositor, &look);
    compositor
        .compose_frame(&mut OwnedPixels::prepare(false))
        .unwrap();
    let ack = compositor
        .scanout_acknowledgement(&staged, face.revision)
        .unwrap();
    let (_, wrong_revision, _, _) = fixture(2);
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

#[test]
fn full_recording_window_fits_native_admission_and_completes_only_after_scanout() {
    let face: Face = if let Some(path) = std::env::var_os("CONDUIT_HISTORY_NATIVE_WINDOW_FACE") {
        use std::io::Read;
        let mut bytes = Vec::new();
        std::fs::File::open(path)
            .unwrap()
            .take(64 * 1024 + 1)
            .read_to_end(&mut bytes)
            .unwrap();
        assert!(bytes.len() <= 64 * 1024);
        serde_json::from_slice(&bytes).unwrap()
    } else {
        let mut history = common::prepared(1);
        history
            .read_next(&common::chunk(
                &(1..=64).map(|n| (n, n * 1000, true)).collect::<Vec<_>>(),
            ))
            .unwrap();
        let window = conduit_environment_history::HistoryFaceWindow::prepare(8).unwrap();
        let fragment = history
            .contribution_window(common::basis(), window)
            .unwrap();
        assert_eq!(history.recovered().samples, 64);
        face_from_fragment(1, fragment)
    };
    face.validate().unwrap();
    let graph = HistoryGraph::prepare(&face, HISTORY_SUBJECT).unwrap();
    assert_eq!(graph.omitted(), 56);
    assert_eq!(graph.samples().len(), 8);
    let (original, pending, mut compositor, look) = fixture_face(face.clone());
    let staged = stage(&pending, &mut compositor, &look);
    assert!(compositor
        .scanout_acknowledgement(&staged, face.revision)
        .is_none());
    let frame = compositor
        .compose_frame(&mut OwnedPixels::prepare(false))
        .unwrap();
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
    session.close_without_input().unwrap();
    assert_eq!(original, face);
}
