use conduit_speech::PreparedGreetingRenderer;
pub struct PcmEvidence {
    pub samples: Vec<i16>,
    pub frame_counts: Vec<usize>,
    pub source_programs: Vec<String>,
}
pub fn render(plans: &[PreparedGreetingRenderer<'_>]) -> PcmEvidence {
    let mut samples = Vec::new();
    let frame_path = std::env::var_os("CONDUIT_COMMON_GREETING_DSP_FRAMES");
    let mut frame_file =
        frame_path.map(|path| std::io::BufWriter::new(std::fs::File::create(path).unwrap()));
    use std::io::Write;
    let mut source_programs = Vec::<String>::new();
    let mut control_file = std::env::var_os("CONDUIT_COMMON_GREETING_CONTROLS")
        .map(|path| std::io::BufWriter::new(std::fs::File::create(path).unwrap()));
    let mut frame_counts = Vec::new();
    for plan in plans {
        let mut cursor = plan.cursor();
        let mut count = 0usize;
        while let Some(frame) = plan.next(&mut cursor).unwrap() {
            let rendered = frame.rendered();
            samples.push(i16::try_from(rendered.sample()).unwrap());
            if let Some(file) = frame_file.as_mut() {
                file.write_all(&u32::try_from(samples.len() - 1).unwrap().to_le_bytes())
                    .unwrap();
                file.write_all(&rendered.frame().to_le_bytes()).unwrap();
                for bytes in [
                    rendered.dsp_input_canonical(),
                    rendered.dsp_output_canonical(),
                ] {
                    file.write_all(&u32::try_from(bytes.len()).unwrap().to_le_bytes())
                        .unwrap();
                    file.write_all(bytes).unwrap();
                }
            }
            if let Some(file) = control_file.as_mut() {
                let executions = rendered
                    .executions()
                    .iter()
                    .chain(frame.target_selection_and_reset_executions());
                for execution in executions {
                    let program = execution.source_program_hex();
                    let index = source_programs
                        .iter()
                        .position(|p| p == program)
                        .unwrap_or_else(|| {
                            source_programs.push(program.to_string());
                            source_programs.len() - 1
                        });
                    file.write_all(&u32::try_from(samples.len() - 1).unwrap().to_le_bytes())
                        .unwrap();
                    file.write_all(&u32::try_from(index).unwrap().to_le_bytes())
                        .unwrap();
                    for bytes in [execution.input_canonical(), execution.output_canonical()] {
                        file.write_all(&u32::try_from(bytes.len()).unwrap().to_le_bytes())
                            .unwrap();
                        file.write_all(bytes).unwrap();
                    }
                }
            }
            count += 1;
        }
        frame_counts.push(count);
    }
    if let Some(mut file) = frame_file {
        file.flush().unwrap();
    }
    if let Some(mut file) = control_file {
        file.flush().unwrap();
    }
    assert_eq!(samples.len(), 32000);
    assert!(samples.iter().any(|s| *s != 0));
    if let Some(path) = std::env::var_os("CONDUIT_COMMON_GREETING_WAV") {
        let bytes = u32::try_from(samples.len() * 2).unwrap();
        let mut wav = Vec::new();
        wav.extend_from_slice(b"RIFF");
        wav.extend_from_slice(&(36 + bytes).to_le_bytes());
        wav.extend_from_slice(b"WAVEfmt ");
        wav.extend_from_slice(&16u32.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&1u16.to_le_bytes());
        wav.extend_from_slice(&16000u32.to_le_bytes());
        wav.extend_from_slice(&32000u32.to_le_bytes());
        wav.extend_from_slice(&2u16.to_le_bytes());
        wav.extend_from_slice(&16u16.to_le_bytes());
        wav.extend_from_slice(b"data");
        wav.extend_from_slice(&bytes.to_le_bytes());
        for sample in &samples {
            wav.extend_from_slice(&sample.to_le_bytes());
        }
        std::fs::write(path, wav).unwrap();
    }

    PcmEvidence {
        samples,
        frame_counts,
        source_programs,
    }
}
