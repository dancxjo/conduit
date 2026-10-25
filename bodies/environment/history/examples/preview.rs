//! CPU diagnostic only: immutable fixture recording -> Face -> two Mask materials.
use conduit_core::{ActivePlayId, BootId, HostId, PlanId};
use conduit_environment_history::{HistoryGraph, PreparedHistoryFace, HISTORY_SUBJECT};
use conduit_environment_runtime::{
    observation::{ObservationContract, PreparedObservationQuantityForms},
    recording::{PreparedRecordingChunk, RecordingContext, RecordingDisposition},
};
use conduit_face::{Face, FaceBasis, FaceContributionBasis};
use std::{fmt::Debug, fs, path::PathBuf};

fn checked<T, E: Debug>(result: Result<T, E>) -> Result<T, String> {
    result.map_err(|error| format!("{error:?}"))
}

fn main() -> Result<(), String> {
    let mut args = std::env::args_os().skip(1);
    let directory = PathBuf::from(args.next().ok_or("usage: preview <output-directory>")?);
    if args.next().is_some() {
        return Err("usage: preview <output-directory>".into());
    }
    let host: HostId = "host/synthetic-environment-preview".into();
    let boot: BootId = "boot/synthetic-environment-preview".into();
    let plan: PlanId = "plan/synthetic-environment-preview".into();
    let play: ActivePlayId = "play/synthetic-environment-preview".into();
    let context = || RecordingContext {
        host: &host,
        boot: &boot,
        plan: &plan,
        play: &play,
    };
    let contract = checked(ObservationContract::prepare())?;
    let mut forms = checked(PreparedObservationQuantityForms::prepare(&contract))?;
    let mut chunk = checked(PreparedRecordingChunk::prepare(context(), &contract))?;
    // Coordinates are tenths of Celsius and percent, encoded through the
    // existing observation Forms. Sequence 7 is absent; sequence 5 failed.
    let samples: [(u64, u64, u16, u16, bool); 12] = [
        (1, 1000, 198, 590, true),
        (2, 2100, 201, 582, true),
        (3, 3900, 205, 570, true),
        (4, 7000, 208, 561, true),
        (5, 8500, 0, 0, false),
        (6, 11000, 212, 548, true),
        (8, 19000, 218, 530, true),
        (9, 20500, 216, 535, true),
        (10, 24000, 213, 547, true),
        (11, 30000, 209, 562, true),
        (12, 32000, 205, 575, true),
        (13, 41000, 202, 588, true),
    ];
    for (sequence, ms, temperature, humidity, available) in samples {
        if available {
            let t = temperature.to_le_bytes();
            let h = humidity.to_le_bytes();
            let observation = checked(forms.unpack(&[t[0], t[1], h[0], h[1]]))?;
            checked(chunk.append(
                sequence,
                ms,
                RecordingDisposition::Available,
                Some(observation),
            ))?;
        } else {
            checked(chunk.append(sequence, ms, RecordingDisposition::Checksum, None))?;
        }
    }
    let recording = checked(chunk.seal())?.to_owned();
    let mut history = checked(PreparedHistoryFace::prepare(context(), &contract, 1))?;
    checked(history.read_next(&recording))?;
    let basis = FaceContributionBasis {
        checked_plot_id: "checked/synthetic-history-inspector".into(),
        plan_id: "plan/synthetic-history-inspector".into(),
        active_play_id: "play/synthetic-history-inspector".into(),
        required_interaction_context: None,
    };
    let fragment = checked(history.contribution(basis.clone()))?;
    let face = checked(Face::new_with_semantics(
        1,
        FaceBasis {
            body_id: None,
            wake_id: None,
            source_document_id: Some("source/synthetic-history-preview".into()),
            checked_plot_id: Some(basis.checked_plot_id),
            expanded_plot_id: Some("expanded/synthetic-history-inspector".into()),
            plan_id: Some(basis.plan_id),
            active_play_id: Some(basis.active_play_id),
            sign_ids: vec![],
        },
        fragment.subjects,
        fragment.relationships,
        fragment.properties,
        vec![],
        vec![],
        vec![],
    ))?;
    let graph = checked(HistoryGraph::prepare(&face, HISTORY_SUBJECT))?;
    let native = checked(graph.native_show())?;
    let html = format!(
        r#"<!doctype html>
<html lang="en"><meta charset="utf-8"><meta name="viewport" content="width=device-width,initial-scale=1">
<title>Synthetic room conditions history</title>
<style>
:root{{font-family:system-ui,sans-serif;color:#192b3c;background:#edf2f6;line-height:1.5}}
body{{max-width:900px;margin:2rem auto;padding:0 1rem}}main{{background:white;border-radius:14px;padding:1.5rem;box-shadow:0 3px 20px #172d4514}}
.fixture{{background:#fff1cc;border:2px solid #946200;border-radius:8px;padding:1rem;font-weight:700}}
h1{{font-size:1.4rem}}h2{{font-size:1.25rem}}svg{{display:block;width:100%;height:auto;border:1px solid #d8e2eb;border-radius:8px;background:#f8fbfd}}
svg text{{font:12px system-ui,sans-serif;fill:#263c51}}svg circle{{fill:#087c91}}details{{margin:1rem 0;overflow-wrap:anywhere}}summary{{cursor:pointer}}li{{margin:.35rem 0}}
table{{display:block;overflow-x:auto;max-width:100%;border-collapse:collapse;font-size:.9rem}}th,td{{text-align:left;padding:.4rem;border-bottom:1px solid #d8e2eb}}
</style><main><h1>Environmental history Mask preview</h1>
<p class="fixture">Synthetic fixture — not a live sensor connection</p>
{}
<p>This CPU diagnostic prepares browser HTML and native GraphicsScene material from the same Face. It does not show native pixels, execute a browser Host, establish membership, or prove recording durability.</p>
</main></html>"#,
        graph.browser_html()
    );
    let manifest = format!(
        "Synthetic fixture — not a live sensor connection\n\
         Source: deterministic twelve-sample CPU fixture; no sensor or external input.\n\
         Face: {}\nRevision: {}\nHistory subject: {}\n\
         Producer: host/synthetic-environment-preview; boot/synthetic-environment-preview\n\
         Time: producer Boot milliseconds; irregular intervals from 1000 to 41000.\n\
         Failure: sequence 5 checksum; gap: missing sequence 7.\n\
         recording-chunk.json: existing recorder encoding, not a durable commit receipt.\n\
         face.json: exact sealed Face shared by both Masks.\n\
         browser.html: HTML/SVG diagnostic material; browser execution is separate proof.\n\
         native.scene: canonical GraphicsScene encoding, not rendered native pixels.\n",
        native.content_id, native.revision, HISTORY_SUBJECT
    );
    let scene_bytes = native.scene.encode();
    let face_json = checked(serde_json::to_vec_pretty(&face))?;
    checked(fs::create_dir_all(&directory))?;
    for (name, bytes) in [
        ("browser.html", html.as_bytes()),
        ("native.scene", &scene_bytes[..native.scene.encoded_len()]),
        ("face.json", face_json.as_slice()),
        ("recording-chunk.json", recording.as_bytes()),
        ("manifest.txt", manifest.as_bytes()),
    ] {
        checked(fs::write(directory.join(name), bytes))?;
    }
    println!("Synthetic history Mask materials: {}", directory.display());
    Ok(())
}
