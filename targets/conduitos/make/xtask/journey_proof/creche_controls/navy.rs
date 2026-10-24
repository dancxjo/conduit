//! Complete navy Look selection over one unchanged actual Crèche Face.
use super::*;
use conduit_face::{VisualDensity as D, VisualTypography as T, navy_look, navy_look_id};
use conduit_plot::rust_binding::NativeRustBinding;
use sha2::{Digest, Sha256};

#[allow(clippy::too_many_arguments)]
pub(super) fn exercise(
    run: &mut Run<'_>,
    initial: &Snapshot,
    destination: &Path,
    source: &str,
    image: &str,
    metadata: &Value,
    recipe: &Value,
    world: &Value,
    boot: &Value,
) -> Result<JourneyIdentity, ConduitosError> {
    let change = run.rename(
        "Café Llanfairpwllgwyngyll long Body name".as_bytes(),
        "creche-navy-long-name",
    )?;
    let baseline = snapshot(&text(run.serial)?)?;
    if retained_selection(&initial.face) != retained_selection(&baseline.face) {
        return Err(refusal("navy probe changed unrelated Source values"));
    }
    let mut transitions = Vec::new();
    for (input, density, typography, scale, forced, navy, capture) in [
        (
            "f11",
            D::Compact,
            T::Editorial,
            100,
            false,
            true,
            "creche-navy-compact",
        ),
        (
            "f6",
            D::Spacious,
            T::Editorial,
            125,
            false,
            true,
            "creche-navy-spacious",
        ),
        (
            "f8",
            D::Spacious,
            T::Engineering,
            125,
            false,
            true,
            "creche-navy-engineering-spacious",
        ),
        (
            "f6",
            D::Compact,
            T::Engineering,
            100,
            false,
            true,
            "creche-navy-engineering-compact",
        ),
        (
            "f9",
            D::Compact,
            T::Engineering,
            200,
            false,
            true,
            "creche-navy-large-text",
        ),
        (
            "f10",
            D::Compact,
            T::Engineering,
            200,
            true,
            true,
            "creche-navy-forced-colors",
        ),
        ("f10", D::Compact, T::Engineering, 200, false, true, ""),
        ("f9", D::Compact, T::Engineering, 100, false, true, ""),
        ("f8", D::Compact, T::Editorial, 100, false, true, ""),
        (
            "f11",
            D::Compact,
            T::Editorial,
            100,
            false,
            false,
            "creche-phosphor-restored",
        ),
    ] {
        let record = super::accessibility::select(run, &baseline, input, scale, forced)?;
        let expression = &record["expression"]["expression"];
        let density_name = if density == D::Compact {
            "compact"
        } else {
            "spacious"
        };
        let type_name = if typography == T::Editorial {
            "editorial"
        } else {
            "engineering"
        };
        if expression["density"] != density_name
            || expression["typography"] != type_name
            || expression["palette"] != "phosphor"
        {
            return Err(refusal(
                "navy transition changed the requested appearance axes",
            ));
        }
        if navy {
            let bytes = navy_look(density, typography)
                .and_then(|look| look.encode())
                .map_err(|error| refusal(format!("canonical navy Look: {error:?}")))?;
            if expression["identity"] != navy_look_id(density, typography)
                || record["configuration"]["look_canonical_bytes"] != bytes.len()
                || record["configuration"]["look_canonical_sha256"]
                    != format!("{:x}", Sha256::digest(&bytes))
            {
                return Err(refusal(
                    "selected navy Look differs from exact canonical resource",
                ));
            }
        } else if expression["identity"] != conduit_face::PHOSPHOR_LOOK_ID {
            return Err(refusal("F11 did not restore the ordinary dark Look"));
        }
        transitions.push(record);
        if !capture.is_empty() {
            run.capture(capture)?;
        }
    }
    let serial = text(run.serial)?;
    if rows(&serial, CHANGE)?.len() != 1
        || !rows(&serial, "CONDUIT_NATIVE_BIRTH ")?.is_empty()
        || rows(&serial, "CONDUIT_BOOT_SIGN ")? != vec![boot.clone()]
    {
        return Err(refusal(
            "navy Look selection dispatched an unexpected Source effect",
        ));
    }
    for checkpoint in super::super::journey_records::decode(&serial)? {
        custody::validate_world(metadata, boot, &checkpoint, source, image)?;
    }
    if run.child.try_wait().map_err(io_error)?.is_some() {
        return Err(refusal("guest exited during navy observations"));
    }
    let proof = json!({"schema":"conduit.conduitos/creche-navy-look-proof@1", "status":"passed",
        "proof_class":"freestanding-emulator", "source_commit":source,"image_sha256":image,
        "canonical_build_metadata":metadata,"boot_sign":boot,"initial_world":world,"selected_recipe":recipe,
        "checked_name_change":change,"baseline_acknowledged_face":baseline.record,
        "transitions":transitions,"final_acknowledged_face":snapshot(&serial)?.record,
        "prepared_frame_budget":frame_budget::summarize(&serial)?,"observations":run.observations,
        "captures":run.captures,"capture_limit":27,"screenshots":"journey-frames/manifest.json",
        "scope":"four navy density/type Looks and user minimum/forced colors with exact Look bytes, unchanged complete Face/structure and fresh local Shows; no Body startup",
        "complete_control_page_traversal":false,"physical_evidence":false,"human_enactment":false});
    fs::write(
        destination.join("creche-navy-look-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(|error| refusal(error.to_string()))?,
    )
    .map_err(io_error)?;
    identity(world)
}
