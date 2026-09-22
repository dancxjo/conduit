//! Native half of the deterministic One form, Two Fronts journey.

use crate::{png_capture::write_rgb_png, presentation::ordinary_front_door_lines, render};
use conduit_core::{BootId, HostId, SignId};
use conduit_presentation::ManifestationLifecycle;
use patchbay_model::{
    RendererAdapterIdentity, RendererAdapterKind, RendererExecution, ZeroBodyFrontDoor,
    ONE_FORM_TWO_FACES_BOOT_ID, ONE_FORM_TWO_FACES_HOST_ID,
};
use std::io::Write;
use std::path::Path;

const WIDTH: usize = 1_100;
const HEIGHT: usize = 720;

pub(super) fn run(root: &Path) -> Result<(), String> {
    let metadata = root
        .metadata()
        .map_err(|error| format!("inspect {}: {error}", root.display()))?;
    if !metadata.is_dir() {
        return Err("two-fronts evidence root must be an existing directory".into());
    }
    let mut session = ZeroBodyFrontDoor::with_identity(
        std::sync::Arc::new(patchbay_hosted::HostedPatchbayAdapter),
        HostId::from(ONE_FORM_TWO_FACES_HOST_ID),
        BootId::from(ONE_FORM_TWO_FACES_BOOT_ID),
    )?;
    let form = session
        .form_ids()
        .into_iter()
        .next()
        .ok_or("two-fronts entrance has no reviewed form")?;
    session.open_form(&form, session.revision())?;
    let projection = session.project()?;
    let lines = ordinary_front_door_lines(&projection.presentation, &projection.navigation, None)?;
    let mut execution = RendererExecution::prepare(
        projection.presentation.clone(),
        RendererAdapterKind::NativeWayland,
        RendererAdapterIdentity {
            host_id: HostId::from(ONE_FORM_TWO_FACES_HOST_ID),
            boot_id: BootId::from(ONE_FORM_TWO_FACES_BOOT_ID),
            target_subject: "journey/one-form-two-fronts/native-frame".into(),
        },
        SignId::from("journey/one-form-two-fronts/native-prepared"),
    )
    .map_err(|error| error.to_string())?;
    let mut pixels = vec![render::BACKGROUND; WIDTH * HEIGHT];
    render::draw_document(&mut pixels, WIDTH, HEIGHT, &lines);
    write_rgb_png(&root.join("native.png"), &pixels, WIDTH, HEIGHT)?;
    execution
        .mark_available(SignId::from("journey/one-form-two-fronts/native-retained"))
        .map_err(|error| error.to_string())?;
    if execution.manifestation.lifecycle != ManifestationLifecycle::Available {
        return Err("native manifestation did not become available".into());
    }
    let receipt = serde_json::json!({
        "schema": "conduit.journey/one-form-two-fronts-native@1",
        "presentation_id": projection.presentation.identity,
        "presentation_revision": projection.presentation.revision,
        "presentation_basis": projection.presentation.basis,
        "renderer_kind": "native-software-wayland",
        "renderer_implementation": "presentation/renderer-wayland@1",
        "manifestation_id": execution.manifestation.manifestation_id,
        "renderer_plan_id": execution.plan.plan_id,
        "renderer_play_id": execution.active_play_id,
        "lifecycle": "available",
        "width": WIDTH,
        "height": HEIGHT,
        "pixel_equality_claimed": false,
    });
    let mut file = std::fs::OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(root.join("native.json"))
        .map_err(|error| format!("create native receipt: {error}"))?;
    serde_json::to_writer_pretty(&mut file, &receipt)
        .map_err(|error| format!("encode native receipt: {error}"))?;
    file.write_all(b"\n")
        .map_err(|error| format!("finish native receipt: {error}"))
}
