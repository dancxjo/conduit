//! Installed terminal entrance for the zero-Body Birth encounter.
//!
//! Text is an inspectable readout of the Face. It is not synthesized speech or
//! playback evidence. Every application edit still crosses the ordinary Mask
//! interaction Fore and the current BirthDraft's revision check.

use std::io::{BufRead, Write};
use std::sync::Arc;

use conduit_birth_plot::{BirthActionOutcome, HostOwnedBirthFaceBasis};
use conduit_patchbay_workbench::{PatchbayModel, ZeroBodyFrontDoor};
use conduit_presentation::{FaceUtteranceProvenance, Presentation};
use conduit_std_host::spoken_face_mask::{ReaderCommand, SpokenFaceSession};
use conduit_std_host::terminal_face_mask::{TerminalFaceMask, TerminalMaskExecution};
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;
use conduit_std_host::StdHost;
use patchbay_hosted::HostedPatchbayAdapter;

mod audio;
use audio::BirthSpeechOutput;

pub(crate) fn run(input: &mut impl BufRead, output: &mut impl Write) -> Result<(), String> {
    run_with_output(input, output, None)
}

/// The installed speech Host is supplied by an explicit, separately admitted
/// caller. The public text entrance has no voice/output selection yet.
fn run_with_output(
    input: &mut impl BufRead,
    output: &mut impl Write,
    mut speech: Option<&mut BirthSpeechOutput>,
) -> Result<(), String> {
    let host = StdHost::new();
    let advertisement = host.advertisement().clone();
    let door = ZeroBodyFrontDoor::from_model(
        Arc::new(HostedPatchbayAdapter),
        PatchbayModel::from_advertisement(advertisement.clone()),
    )?;
    let encounter_id = random_uuid()?;
    let basis = HostOwnedBirthFaceBasis {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        encounter_id: encounter_id.clone(),
    };
    let mut draft = door.creche_draft(encounter_id)?;
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).map_err(debug_error)?;
    let (mut face, mut show) = present(&draft, &basis, &mut execution, output)?;
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).map_err(debug_error)?;
    if speech.is_none() {
        writeln!(output, "Text readout; no speech audio has been produced. Commands: help, read all, next, previous, repeat, focus ACTION, edit value TEXT, activate, stop, quit.").map_err(io_error)?;
    }
    let mut sequence = 1_u64;
    read(
        &mut reader,
        &face,
        &show,
        ReaderCommand::ReadAll,
        sequence,
        speech.as_deref_mut(),
        output,
    )?;

    loop {
        write!(output, "birth> ").map_err(io_error)?;
        output.flush().map_err(io_error)?;
        let mut line = String::new();
        if input.read_line(&mut line).map_err(io_error)? == 0 {
            execution.close_without_input().map_err(debug_error)?;
            return Ok(());
        }
        let line = line.trim_end_matches(['\r', '\n']);
        if line == "quit" {
            execution.close_without_input().map_err(debug_error)?;
            return Ok(());
        }
        let command = match parse_command(line, &reader, &face) {
            Ok(command) => command,
            Err(message) => {
                writeln!(output, "Refused input: {message}").map_err(io_error)?;
                continue;
            }
        };
        sequence = sequence.checked_add(1).ok_or("input sequence exhausted")?;
        let result = match reader.command(&face, &show, command, sequence) {
            Ok(result) => result,
            Err(refusal) => {
                writeln!(output, "Refused action: {refusal:?}").map_err(io_error)?;
                continue;
            }
        };
        emit_readout(&mut reader, &face, &show, speech.as_deref_mut(), output)?;
        let Some(interaction) = result.interaction else {
            continue;
        };
        writeln!(
            output,
            "Interaction: action={} face-revision={} show={}",
            interaction.action_id,
            face.revision,
            show.show_id.as_str()
        )
        .map_err(io_error)?;
        let correlated = execution.interact(interaction).map_err(debug_error)?;
        match draft
            .apply_host_owned_face_interaction(&basis, &show, &correlated.interaction)
            .map_err(debug_error)?
        {
            BirthActionOutcome::Changed => {
                writeln!(
                    output,
                    "Result: Changed; BirthDraft revision={}",
                    draft.revision()
                )
                .map_err(io_error)?;
                (face, show) = present(&draft, &basis, &mut execution, output)?;
                reader
                    .refresh(face.clone(), show.clone())
                    .map_err(debug_error)?;
                emit_readout(&mut reader, &face, &show, speech.as_deref_mut(), output)?;
            }
            BirthActionOutcome::Birth(selection) => {
                writeln!(
                    output,
                    "Result: Birth requested at BirthDraft revision={}",
                    selection.revision
                )
                .map_err(io_error)?;
                let revision = door.revision();
                let born = door.birth_from_creche(selection, revision)?;
                writeln!(
                    output,
                    "Body born in this encounter (not retained): {}",
                    born.body().body_id.as_str()
                )
                .map_err(io_error)?;
                return Ok(());
            }
        }
    }
}

fn present(
    draft: &conduit_birth_plot::BirthDraft,
    basis: &HostOwnedBirthFaceBasis,
    execution: &mut HostedTerminalMaskExecution,
    output: &mut impl Write,
) -> Result<
    (
        conduit_presentation::Presentation,
        conduit_presentation::MaskShow,
    ),
    String,
> {
    let face = draft.host_owned_face(basis).map_err(debug_error)?;
    let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24).map_err(debug_error)?;
    mask.present(execution, output).map_err(debug_error)?;
    writeln!(output).map_err(io_error)?;
    let show = mask
        .show()
        .ok_or("terminal did not acknowledge a Show")?
        .clone();
    Ok((face, show))
}

fn read(
    reader: &mut SpokenFaceSession,
    face: &conduit_presentation::Presentation,
    show: &conduit_presentation::MaskShow,
    command: ReaderCommand,
    sequence: u64,
    speech: Option<&mut BirthSpeechOutput>,
    output: &mut impl Write,
) -> Result<(), String> {
    reader
        .command(face, show, command, sequence)
        .map_err(debug_error)?;
    emit_readout(reader, face, show, speech, output)
}

fn emit_readout(
    reader: &mut SpokenFaceSession,
    face: &Presentation,
    show: &conduit_presentation::MaskShow,
    speech: Option<&mut BirthSpeechOutput>,
    output: &mut impl Write,
) -> Result<(), String> {
    if let Some(speech) = speech {
        while let Some(step) = speech.advance(reader, face, show)? {
            writeln!(
                output,
                "Produced audio: {} ({} PCM bytes, {} ordered segments, Face revision={}, Show={}; no speaker playback)",
                step.execution.wav_path.display(),
                step.execution.receipt.pcm_bytes,
                step.batch.segments.len(),
                face.revision,
                show.show_id.as_str()
            )
            .map_err(io_error)?;
            if let Some(terminal) = step.terminal {
                writeln!(output, "Spoken turn: {:?}", terminal.outcome).map_err(io_error)?;
                break;
            }
        }
        return Ok(());
    }
    if let Some(readout) = reader.take_text_readout().map_err(debug_error)? {
        writeln!(
            output,
            "Text Face revision={} Show={}",
            readout.face_revision, readout.show_id
        )
        .map_err(io_error)?;
        for clause in readout.clauses {
            writeln!(output, "{clause}").map_err(io_error)?;
        }
    }
    Ok(())
}

fn parse_command(
    line: &str,
    reader: &SpokenFaceSession,
    face: &Presentation,
) -> Result<ReaderCommand, &'static str> {
    match line {
        "help" => Ok(ReaderCommand::Help),
        "read all" => Ok(ReaderCommand::ReadAll),
        "next" => Ok(ReaderCommand::Next),
        "previous" => Ok(ReaderCommand::Previous),
        "repeat" => Ok(ReaderCommand::Repeat),
        "activate" => Ok(ReaderCommand::Activate),
        "stop" => Ok(ReaderCommand::Stop),
        _ => {
            if let Some(action) = line.strip_prefix("focus ") {
                if action.is_empty() {
                    return Err("focus needs an exact action ID");
                }
                return Ok(ReaderCommand::FocusAction(action.into()));
            }
            if let Some(rest) = line.strip_prefix("edit ") {
                let (argument, value) = rest
                    .split_once(' ')
                    .ok_or("edit needs an argument and value")?;
                if argument.is_empty() {
                    return Err("edit needs an argument");
                }
                let boolean = match &reader.focused_clause().provenance {
                    FaceUtteranceProvenance::Action(provenance) => face
                        .actions
                        .iter()
                        .find(|action| &action.identity == provenance.identity())
                        .and_then(|action| {
                            action.arguments.iter().find(|item| item.name == argument)
                        })
                        .is_some_and(|item| item.contract.value_kind.as_str() == "value/bool"),
                    _ => false,
                };
                let value = match (boolean, value) {
                    (true, "true") => vec![1],
                    (true, "false") => vec![0],
                    _ => value.as_bytes().to_vec(),
                };
                return Ok(ReaderCommand::Edit {
                    argument: argument.into(),
                    value,
                });
            }
            Err("unknown command; enter help")
        }
    }
}

fn random_uuid() -> Result<String, String> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| format!("Birth encounter identity: {error}"))?;
    bytes[6] = (bytes[6] & 0x0f) | 0x40;
    bytes[8] = (bytes[8] & 0x3f) | 0x80;
    Ok(format!(
        "{:08x}-{:04x}-{:04x}-{:04x}-{:012x}",
        u32::from_be_bytes(bytes[0..4].try_into().unwrap()),
        u16::from_be_bytes(bytes[4..6].try_into().unwrap()),
        u16::from_be_bytes(bytes[6..8].try_into().unwrap()),
        u16::from_be_bytes(bytes[8..10].try_into().unwrap()),
        u64::from_be_bytes([
            0, 0, bytes[10], bytes[11], bytes[12], bytes[13], bytes[14], bytes[15]
        ])
    ))
}

fn debug_error(error: impl std::fmt::Debug) -> String {
    format!("{error:?}")
}
fn io_error(error: std::io::Error) -> String {
    format!("terminal I/O: {error}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn zero_body_commands_edit_select_read_and_birth_through_mask() {
        let input = b"help\nfocus creche.name\nedit value true\nactivate\nfocus creche.name\nedit value Ada\nactivate\nread all\nfocus creche.plot.0\nedit value true\nactivate\nfocus creche.birth\nactivate\n";
        let mut output = Vec::new();
        run(&mut &input[..], &mut output).unwrap();
        let output = String::from_utf8(output).unwrap();
        assert!(output.contains("Text Face revision="));
        assert!(output.contains("Interaction: action=creche.name"));
        assert_eq!(output.matches("Interaction: action=creche.name").count(), 2);
        assert!(output.contains("Interaction: action=creche.plot.0"));
        assert!(output.contains("Interaction: action=creche.birth"));
        assert!(output.contains("Result: Changed; BirthDraft revision="));
        assert!(output.contains("Body born in this encounter (not retained):"));
        assert!(output.contains("Text readout; no speech audio has been produced."));
    }

    #[test]
    fn eof_and_quit_do_not_birth() {
        for mut input in [b"".as_slice(), b"quit\n".as_slice()] {
            let mut output = Vec::new();
            run(&mut input, &mut output).unwrap();
            assert!(!String::from_utf8(output)
                .unwrap()
                .contains("Body born in this encounter"));
        }
    }

    /// Explicit local-provider proof. This uses the actual zero-Body draft,
    /// current Face, acknowledged terminal Show, planned speech Fore, and WAV
    /// effect. It does not establish audible playback or a retained Body.
    #[test]
    #[ignore = "requires installed eSpeak NG and retains a real WAV"]
    fn zero_body_face_produces_real_ordered_speech() {
        use conduit_std_host::hosted_speech_synthesis::EspeakDiscovery;

        let host = StdHost::new();
        let advertisement = host.advertisement().clone();
        let door = ZeroBodyFrontDoor::from_model(
            Arc::new(HostedPatchbayAdapter),
            PatchbayModel::from_advertisement(advertisement.clone()),
        )
        .unwrap();
        let basis = HostOwnedBirthFaceBasis {
            host_id: advertisement.host_id.clone(),
            boot_id: advertisement.boot_id.clone(),
            encounter_id: random_uuid().unwrap(),
        };
        let draft = door.creche_draft(basis.encounter_id.clone()).unwrap();
        let mut execution = HostedTerminalMaskExecution::new(&advertisement).unwrap();
        let (face, show) = present(&draft, &basis, &mut execution, &mut Vec::new()).unwrap();
        assert!(face.basis.body_id.is_none());
        let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
        reader
            .command(&face, &show, ReaderCommand::ReadAll, 1)
            .unwrap();
        let mut reference = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
        reference
            .command(&face, &show, ReaderCommand::ReadAll, 1)
            .unwrap();
        let expected = reference
            .take_text_readout()
            .unwrap()
            .unwrap()
            .clauses
            .join("");

        let provider = EspeakDiscovery::inspect(
            std::path::Path::new("/usr/bin/espeak-ng"),
            std::path::Path::new("/usr/lib/x86_64-linux-gnu/espeak-ng-data"),
            "en-us",
            &[std::path::PathBuf::from(
                "/usr/lib/x86_64-linux-gnu/libespeak-ng.so.1.1.51",
            )],
        )
        .unwrap();
        let directory = std::env::temp_dir().join(format!(
            "conduit-zero-body-spoken-{}",
            random_uuid().unwrap()
        ));
        std::fs::create_dir(&directory).unwrap();
        let mut speech = BirthSpeechOutput::new(provider, &directory).unwrap();
        let mut actual = String::new();
        let mut produced = Vec::new();
        let mut terminal = None;
        while let Some(step) = speech.advance(&mut reader, &face, &show).unwrap() {
            assert_eq!(step.batch.face_id, face.identity.as_str());
            assert_eq!(step.batch.source_show_id, show.show_id.as_str());
            assert_eq!(step.batch.segments.len(), 1);
            for (index, segment) in step.batch.segments.iter().enumerate() {
                assert_eq!(segment.segment.sequence as usize, index);
                assert!(segment.segment.text.len() <= 64);
                actual.push_str(&segment.segment.text);
            }
            assert_eq!(
                step.execution.receipt.source_segments_sha256,
                step.batch.source_segments_sha256
            );
            assert_eq!(
                step.execution.receipt.wav_bytes,
                std::fs::metadata(&step.execution.wav_path).unwrap().len()
            );
            let wav = std::fs::read(&step.execution.wav_path).unwrap();
            assert_eq!(&wav[..4], b"RIFF");
            assert!(wav[44..].iter().any(|sample| *sample != 0));
            produced.push(serde_json::json!({
                "wav": step.execution.wav_path,
                "segments": step.batch.segments.iter().map(|segment| serde_json::json!({
                    "sequence": segment.segment.sequence,
                    "text": segment.segment.text,
                    "reason": format!("{:?}", segment.segment.reason),
                })).collect::<Vec<_>>(),
                "sourceSegmentsSha256": step.execution.receipt.source_segments_sha256,
                "providerSha256": step.execution.receipt.provider_sha256,
                "speechPlanId": step.execution.receipt.speech_plan_id,
                "speechPlayId": step.execution.receipt.speech_play_id,
                "wavSha256": step.execution.receipt.wav_sha256,
                "pcmBytes": step.execution.receipt.pcm_bytes,
                "pcmBlocks": step.execution.receipt.pcm_blocks,
            }));
            if let Some(receipt) = step.terminal {
                terminal = Some(receipt);
                break;
            }
        }
        assert_eq!(actual, expected);
        assert!(produced.len() > 1);
        assert!(produced
            .iter()
            .any(|item| item["pcmBlocks"].as_u64().unwrap() > 1));
        assert_eq!(
            terminal.unwrap().outcome,
            conduit_std_host::spoken_face_mask::SpokenTurnOutcome::Completed
        );
        std::fs::write(
            directory.join("receipt.json"),
            serde_json::to_vec_pretty(&serde_json::json!({
                "faceId": face.identity.as_str(),
                "faceRevision": face.revision,
                "showId": show.show_id.as_str(),
                "bodyId": serde_json::Value::Null,
                "audioProduced": true,
                "speakerPlayback": false,
                "batches": produced,
            }))
            .unwrap(),
        )
        .unwrap();
        eprintln!("zero-Body spoken audio: {}", directory.display());
    }
}
