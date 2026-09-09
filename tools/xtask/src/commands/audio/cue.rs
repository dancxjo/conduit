//! File rendering is a DSP audition, not a lifecycle or playback receipt.
use crate::{cli::GlobalOpts, output::RepositoryOutput};
use conduit_synth::{StartupChime, STARTUP_CHIME_FRAMES, STARTUP_CHIME_SCORE_ID};
use serde::Serialize;
use std::{fs::OpenOptions, io::Write, path::Path};

#[derive(Serialize)]
struct CueReport<'a> {
    schema: &'static str,
    proof_class: &'static str,
    score_id: &'static str,
    dry_run: bool,
    effects_performed: bool,
    file_written: bool,
    audio_device_opened: bool,
    sample_rate_hz: u32,
    channels: u16,
    frames: u32,
    output: &'a Path,
}

pub fn render(opts: &GlobalOpts, path: &Path) -> Result<(), Box<dyn std::error::Error>> {
    let output = RepositoryOutput::from_opts(opts);
    if !output.dry_run() {
        let mut file = OpenOptions::new().write(true).create_new(true).open(path)?;
        write_wave(&mut file)?;
        file.sync_all()?;
    }
    output.emit_json(&CueReport {
        schema: "conduit.tools/xtask/startup-cue-render@1",
        proof_class: "deterministic-dsp-render",
        score_id: STARTUP_CHIME_SCORE_ID,
        dry_run: output.dry_run(),
        effects_performed: !output.dry_run(),
        file_written: !output.dry_run(),
        audio_device_opened: false,
        sample_rate_hz: conduit_synth::REFERENCE_SAMPLE_RATE_HZ,
        channels: 1,
        frames: STARTUP_CHIME_FRAMES,
        output: path,
    })?;
    output.emit_human(|writer| {
        writeln!(
            writer,
            "{}startup cue: {} (DSP audition; no audio device opened)",
            if output.dry_run() {
                "dry-run "
            } else {
                "Rendered "
            },
            path.display()
        )
    })?;
    Ok(())
}

fn write_wave(writer: &mut impl Write) -> std::io::Result<()> {
    let data_bytes = STARTUP_CHIME_FRAMES * 2;
    let rate = conduit_synth::REFERENCE_SAMPLE_RATE_HZ;
    writer.write_all(b"RIFF")?;
    writer.write_all(&(36 + data_bytes).to_le_bytes())?;
    writer.write_all(b"WAVEfmt ")?;
    writer.write_all(&16_u32.to_le_bytes())?;
    writer.write_all(&1_u16.to_le_bytes())?; // PCM
    writer.write_all(&1_u16.to_le_bytes())?; // mono
    writer.write_all(&rate.to_le_bytes())?;
    writer.write_all(&(rate * 2).to_le_bytes())?;
    writer.write_all(&2_u16.to_le_bytes())?;
    writer.write_all(&16_u16.to_le_bytes())?;
    writer.write_all(b"data")?;
    writer.write_all(&data_bytes.to_le_bytes())?;
    let mut cue = StartupChime::new();
    let mut block = [0_i16; 256];
    let mut encoded = [0_u8; 512];
    loop {
        let count = cue.render(&mut block).expect("admitted render block");
        if count == 0 {
            return Ok(());
        }
        for (sample, bytes) in block[..count].iter().zip(encoded.as_chunks_mut::<2>().0) {
            bytes.copy_from_slice(&sample.to_le_bytes());
        }
        writer.write_all(&encoded[..count * 2])?;
    }
}
