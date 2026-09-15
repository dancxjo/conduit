//! Bounded Orbium/Lenia execution retained as exact scalar-presentation checkpoints.

use crate::evidence::{
    EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance, EvidenceResult,
};
use crate::workspace::workspace_root;
use flate2::{write::ZlibEncoder, Compression};
use serde_json::Value;
use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::Command;

const CHECKPOINTS: [u64; 4] = [0, 1, 8, 32];
const COLUMNS: usize = 32;
const ROWS: usize = 16;
const SCALE: usize = 20;
const RAMP: &[u8] = b" .:-=+*#%@";

pub(super) fn run(output: PathBuf) -> Result<(), Box<dyn std::error::Error>> {
    let workspace = workspace_root()?;
    let output = absolute(&workspace, output);
    let parent = output.parent().ok_or("Little Life output has no parent")?;
    std::fs::create_dir_all(parent)?;
    std::fs::create_dir(&output)
        .map_err(|error| format!("create new Little Life evidence directory: {error}"))?;
    let mut guard = NewDirectory::new(output.clone());

    command(
        Command::new(cargo()).current_dir(&workspace).args([
            "build",
            "--locked",
            "--release",
            "-p",
            "conduit",
        ]),
        "build the Conduit product entrance",
    )?;
    let execution_path = output.join("execution.json");
    let run = Command::new(workspace.join("target/release/conduit"))
        .current_dir(&workspace)
        .args([
            "run",
            "forms/little-life/main.conduit",
            "--await-terminal",
            "--report",
        ])
        .arg(&execution_path)
        .output()
        .map_err(|error| format!("run Little Life through Conduit: {error}"))?;
    if !run.status.success() {
        return Err(format!(
            "Little Life Conduit run failed with {}: {}",
            run.status,
            String::from_utf8_lossy(&run.stderr)
        )
        .into());
    }
    let transcript = String::from_utf8(run.stdout)
        .map_err(|_| "Little Life presentation transcript is not UTF-8")?;
    create_new(&output.join("presentation.txt"), transcript.as_bytes())?;
    let fields = parse_fields(&transcript)?;
    if fields.len() != 32 || fields.iter().map(|field| field.generation).ne(1..=32) {
        return Err("Little Life did not present exactly generations 1 through 32".into());
    }

    let seed = conduit_alife::orbium_seed(32, 32, 1)
        .map_err(|error| format!("construct exact Orbium seed: {error:?}"))?;
    let seed_bitmap = conduit_alife::lenia_field_to_gray8(&seed)
        .map_err(|error| format!("lower exact Orbium seed: {error:?}"))?;
    write_gray_png(
        &output.join("t000.png"),
        seed_bitmap.pixels(),
        usize::from(seed_bitmap.width()),
        usize::from(seed_bitmap.height()),
    )?;
    for generation in [1, 8, 32] {
        let field = &fields[generation - 1];
        write_terminal_png(&output.join(format!("t{generation:03}.png")), &field.rows)?;
    }

    seal_manifest(&workspace, &output, &execution_path)?;
    guard.retain();
    println!("LITTLE LIFE COMPLETE: {}", output.display());
    Ok(())
}

fn seal_manifest(workspace: &Path, root: &Path, execution_path: &Path) -> Result<(), String> {
    let report: Value = serde_json::from_slice(
        &std::fs::read(execution_path)
            .map_err(|error| format!("read Little Life execution report: {error}"))?,
    )
    .map_err(|error| format!("decode Little Life execution report: {error}"))?;
    if report.get("schema").and_then(Value::as_str) != Some("conduit.observatory.snapshot/v2") {
        return Err("Little Life execution report has the wrong schema".into());
    }
    let plans = report
        .get("plans")
        .and_then(Value::as_array)
        .filter(|plans| plans.len() == 1)
        .ok_or("Little Life execution must retain exactly one Plan")?;
    let plan_id = text(&plans[0], "plan_id")?;
    let observations = report
        .get("observations")
        .and_then(Value::as_array)
        .ok_or("Little Life execution report lacks observations")?;
    let terminal = observations
        .iter()
        .find(|observation| {
            observation
                .get("kind")
                .and_then(|kind| kind.get("PlanTerminal"))
                .is_some()
        })
        .ok_or("Little Life execution report lacks a Plan terminal")?;
    let active_play_id = text(terminal, "active_play_id")?;
    if terminal
        .pointer("/kind/PlanTerminal/disposition")
        .and_then(Value::as_str)
        != Some("Completed")
    {
        return Err("Little Life Play did not complete".into());
    }
    let mut manifest =
        EvidenceManifest::new(root, workspace, "journey-little-life", "journey-gallery")?;
    for generation in CHECKPOINTS {
        manifest.declare(output(
            format!("little-life.t{generation}"),
            EvidenceKind::Screenshot,
            format!("t{generation:03}.png"),
            "image/png",
            generation,
            plan_id,
            active_play_id,
        ))?;
    }
    manifest.declare(output(
        "little-life.presentation".into(),
        EvidenceKind::ConsoleTranscript,
        "presentation.txt".into(),
        "text/plain; charset=utf-8",
        32,
        plan_id,
        active_play_id,
    ))?;
    let mut execution = output(
        "little-life.execution".into(),
        EvidenceKind::MachineReadableManifest,
        "execution.json".into(),
        "application/json",
        32,
        plan_id,
        active_play_id,
    );
    execution.provenance.step_id = Some("plan-terminal".into());
    execution.provenance.renderer_id = None;
    execution.provenance.proof_class = Some("ordinary-plan-play-execution-report".into());
    manifest.declare(execution)?;
    manifest.finish(EvidenceResult::Complete)
}

fn output(
    id: String,
    kind: EvidenceKind,
    path: String,
    media_type: &str,
    generation: u64,
    plan_id: &str,
    active_play_id: &str,
) -> EvidenceOutput {
    EvidenceOutput {
        id,
        kind,
        path: path.into(),
        media_type: media_type.into(),
        required: true,
        provenance: EvidenceProvenance {
            scenario_id: "little-life.orbium-lenia@1".into(),
            step_id: Some(format!("generation-{generation}")),
            plan_id: Some(plan_id.into()),
            active_play_id: Some(active_play_id.into()),
            renderer_id: Some(if generation == 0 {
                "presentation/gray8-bitmap@1".into()
            } else {
                "std/kernel-present-scalar-field@1".into()
            }),
            asserted_semantic_disposition: Some("completed".into()),
            proof_class: Some(if generation == 0 {
                "deterministic-orbium-seed-bitmap".into()
            } else {
                "std-scalar-field-terminal-presentation".into()
            }),
            image_width: (kind == EvidenceKind::Screenshot).then_some(if generation == 0 {
                32
            } else {
                (COLUMNS * SCALE) as u32
            }),
            image_height: (kind == EvidenceKind::Screenshot).then_some(if generation == 0 {
                32
            } else {
                (ROWS * SCALE) as u32
            }),
            physical_evidence: Some(false),
            ..EvidenceProvenance::default()
        },
    }
}

struct PresentedField {
    generation: u64,
    rows: Vec<Vec<u8>>,
}

fn parse_fields(transcript: &str) -> Result<Vec<PresentedField>, String> {
    let mut lines = transcript.lines();
    let mut fields = Vec::new();
    while let Some(line) = lines.next() {
        if !line.starts_with("SCALAR-FIELD title=\"Orbium evolution\"") {
            continue;
        }
        let generation = line
            .split_whitespace()
            .find_map(|part| part.strip_prefix("generation="))
            .ok_or("scalar-field header lacks generation")?
            .parse::<u64>()
            .map_err(|_| "scalar-field generation is invalid")?;
        let mut rows = Vec::with_capacity(ROWS);
        for _ in 0..ROWS {
            let row = lines
                .next()
                .ok_or("scalar-field presentation is truncated")?;
            if row.len() != COLUMNS || !row.bytes().all(|value| RAMP.contains(&value)) {
                return Err("scalar-field presentation has invalid bounded cells".into());
            }
            rows.push(row.as_bytes().to_vec());
        }
        fields.push(PresentedField { generation, rows });
    }
    Ok(fields)
}

fn write_terminal_png(path: &Path, rows: &[Vec<u8>]) -> Result<(), String> {
    let mut pixels = vec![0; COLUMNS * SCALE * ROWS * SCALE];
    for (y, row) in rows.iter().enumerate() {
        for (x, value) in row.iter().enumerate() {
            let level = RAMP
                .iter()
                .position(|candidate| candidate == value)
                .unwrap();
            let intensity = (level * 255 / (RAMP.len() - 1)) as u8;
            for py in y * SCALE..(y + 1) * SCALE {
                for px in x * SCALE..(x + 1) * SCALE {
                    pixels[py * COLUMNS * SCALE + px] = intensity;
                }
            }
        }
    }
    write_gray_png(path, &pixels, COLUMNS * SCALE, ROWS * SCALE)
}

fn write_gray_png(path: &Path, pixels: &[u8], width: usize, height: usize) -> Result<(), String> {
    if pixels.len() != width * height || width == 0 || height == 0 {
        return Err("Little Life PNG dimensions are invalid".into());
    }
    let mut filtered = Vec::with_capacity((width + 1) * height);
    for row in pixels.chunks_exact(width) {
        filtered.push(0);
        filtered.extend_from_slice(row);
    }
    let mut encoder = ZlibEncoder::new(Vec::new(), Compression::best());
    encoder
        .write_all(&filtered)
        .map_err(|error| format!("compress Little Life PNG: {error}"))?;
    let compressed = encoder
        .finish()
        .map_err(|error| format!("finish Little Life PNG: {error}"))?;
    let mut png = b"\x89PNG\r\n\x1a\n".to_vec();
    let mut ihdr = Vec::with_capacity(13);
    ihdr.extend_from_slice(&(width as u32).to_be_bytes());
    ihdr.extend_from_slice(&(height as u32).to_be_bytes());
    ihdr.extend_from_slice(&[8, 0, 0, 0, 0]);
    chunk(&mut png, b"IHDR", &ihdr);
    chunk(&mut png, b"IDAT", &compressed);
    chunk(&mut png, b"IEND", &[]);
    create_new(path, &png)
}

fn chunk(output: &mut Vec<u8>, kind: &[u8; 4], bytes: &[u8]) {
    output.extend_from_slice(&(bytes.len() as u32).to_be_bytes());
    output.extend_from_slice(kind);
    output.extend_from_slice(bytes);
    let mut crc = 0xffff_ffff_u32;
    for byte in kind.iter().chain(bytes) {
        crc ^= u32::from(*byte);
        for _ in 0..8 {
            crc = (crc >> 1) ^ (0xedb8_8320_u32 & (0_u32.wrapping_sub(crc & 1)));
        }
    }
    output.extend_from_slice(&(!crc).to_be_bytes());
}

fn create_new(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let mut file = OpenOptions::new()
        .write(true)
        .create_new(true)
        .open(path)
        .map_err(|error| format!("create {}: {error}", path.display()))?;
    file.write_all(bytes)
        .map_err(|error| format!("write {}: {error}", path.display()))
}

fn text<'a>(value: &'a Value, field: &str) -> Result<&'a str, String> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| format!("Little Life receipt lacks {field}"))
}

fn command(command: &mut Command, description: &str) -> Result<(), String> {
    let status = command
        .status()
        .map_err(|error| format!("{description}: {error}"))?;
    status
        .success()
        .then_some(())
        .ok_or_else(|| format!("{description} failed with {status}"))
}

fn cargo() -> std::ffi::OsString {
    std::env::var_os("CARGO").unwrap_or_else(|| "cargo".into())
}

fn absolute(workspace: &Path, path: PathBuf) -> PathBuf {
    if path.is_absolute() {
        path
    } else {
        workspace.join(path)
    }
}

struct NewDirectory {
    path: PathBuf,
    retain: bool,
}

impl NewDirectory {
    fn new(path: PathBuf) -> Self {
        Self {
            path,
            retain: false,
        }
    }

    fn retain(&mut self) {
        self.retain = true;
    }
}

impl Drop for NewDirectory {
    fn drop(&mut self) {
        if !self.retain {
            let _ = std::fs::remove_dir_all(&self.path);
        }
    }
}
