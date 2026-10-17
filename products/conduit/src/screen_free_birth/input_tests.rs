use super::*;
use conduit_birth_plot::{BirthActionOutcome, HostOwnedBirthFaceBasis};
use conduit_patchbay_workbench::{PatchbayModel, ZeroBodyFrontDoor};
use conduit_std_host::terminal_face_mask::TerminalMaskExecution;
use conduit_std_host::terminal_mask_execution::HostedTerminalMaskExecution;
use conduit_std_host::StdHost;
use patchbay_hosted::HostedPatchbayAdapter;
use std::sync::Arc;

#[test]
fn read_current_items_is_a_generic_spoken_command() {
    let advertisement = StdHost::new().advertisement().clone();
    let door = ZeroBodyFrontDoor::from_model(
        Arc::new(HostedPatchbayAdapter),
        PatchbayModel::from_advertisement(advertisement.clone()),
    )
    .unwrap();
    let encounter = "00112233-4455-6677-8899-aabbccddeeff";
    let draft = door.creche_draft(encounter.into()).unwrap();
    let basis = HostOwnedBirthFaceBasis {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        encounter_id: encounter.into(),
    };
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).unwrap();
    let (face, show) =
        super::super::present(&draft, &basis, &mut execution, &mut Vec::new()).unwrap();
    let reader = SpokenFaceSession::new(face.clone(), show).unwrap();
    assert_eq!(
        parse_command("read remaining", &reader, &face),
        Ok(ReaderCommand::ReadItemPage)
    );
    assert_eq!(
        parse_command("more items", &reader, &face),
        Ok(ReaderCommand::MoreItems)
    );
    assert_eq!(
        parse_command("read current items", &reader, &face),
        Ok(ReaderCommand::ReadCurrentItems)
    );
    assert_eq!(
        parse_command("summary", &reader, &face),
        Ok(ReaderCommand::Summary)
    );
}

#[test]
fn selected_readout_keeps_multiple_bounded_segments_in_one_source_batch() {
    let advertisement = StdHost::new().advertisement().clone();
    let door = ZeroBodyFrontDoor::from_model(
        Arc::new(HostedPatchbayAdapter),
        PatchbayModel::from_advertisement(advertisement.clone()),
    )
    .unwrap();
    let encounter = "00112233-4455-6677-8899-aabbccddeeff";
    let draft = door.creche_draft(encounter.into()).unwrap();
    let basis = HostOwnedBirthFaceBasis {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        encounter_id: encounter.into(),
    };
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).unwrap();
    let (face, show) =
        super::super::present(&draft, &basis, &mut execution, &mut Vec::new()).unwrap();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(&face, &show, ReaderCommand::ReadAll, 1)
        .unwrap();
    let batch = reader.next_batch_with_limits(4, 64).unwrap().unwrap();
    assert!(batch.segments.len() > 1);
    let receipt_segments =
        super::super::selected_playback::verified_spoken_segments(&face, &show, &batch).unwrap();
    assert_eq!(receipt_segments.len(), batch.segments.len());
    assert_eq!(receipt_segments.last().unwrap()["reason"], "final-flush");
}

#[test]
fn plot_argument_reached_by_reading_accepts_boolean_edits_on_current_birth_face() {
    let advertisement = StdHost::new().advertisement().clone();
    let door = ZeroBodyFrontDoor::from_model(
        Arc::new(HostedPatchbayAdapter),
        PatchbayModel::from_advertisement(advertisement.clone()),
    )
    .unwrap();
    let encounter = "00112233-4455-6677-8899-aabbccddeeff";
    let basis = HostOwnedBirthFaceBasis {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        encounter_id: encounter.into(),
    };
    let mut draft = door.creche_draft(encounter.into()).unwrap();
    let mut sequence = 0;
    for (word, expected) in [("true", 1), ("false", 0)] {
        let mut execution = HostedTerminalMaskExecution::new(&advertisement).unwrap();
        let (face, show) =
            super::super::present(&draft, &basis, &mut execution, &mut Vec::new()).unwrap();
        let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
        // Discover a supported Plot control using only the offered reading order.
        for _ in 0..reader.clause_count() {
            sequence += 1;
            let command = parse_command("next action", &reader, &face).unwrap();
            reader.command(&face, &show, command, sequence).unwrap();
            reader.take_text_readout().unwrap();
            let FaceUtteranceProvenance::Action(source) = &reader.focused_clause().provenance
            else {
                continue;
            };
            if face.actions.iter().any(|action| {
                &action.identity == source.identity()
                    && action
                        .arguments
                        .iter()
                        .any(|argument| argument.contract.value_kind.as_str() == "value/bool")
            }) {
                break;
            }
        }
        sequence += 1;
        let next = parse_command("next", &reader, &face).unwrap();
        reader.command(&face, &show, next, sequence).unwrap();
        let instructions = reader.take_text_readout().unwrap().unwrap();
        assert!(instructions
            .clauses
            .iter()
            .any(|text| text.contains("Choose true or false")));
        assert!(matches!(
            reader.focused_clause().provenance,
            FaceUtteranceProvenance::ActionArgument(_)
        ));
        let command = parse_command(&format!("edit value {word}"), &reader, &face).unwrap();
        assert_eq!(
            command,
            ReaderCommand::Edit {
                argument: "value".into(),
                value: vec![expected]
            }
        );
        sequence += 1;
        reader.command(&face, &show, command, sequence).unwrap();
        reader.take_text_readout().unwrap();
        sequence += 1;
        let activate = parse_command("activate", &reader, &face).unwrap();
        let interaction = reader
            .command(&face, &show, activate, sequence)
            .unwrap()
            .interaction
            .unwrap();
        assert_eq!(interaction.face_id, face.identity.as_str());
        assert_eq!(interaction.face_revision, face.revision);
        assert_eq!(interaction.show_id, show.show_id.as_str());
        assert_eq!(interaction.arguments[0].value, vec![expected]);
        let correlated = execution.interact(interaction).unwrap();
        assert_eq!(
            draft
                .apply_host_owned_face_interaction(&basis, &show, &correlated.interaction)
                .unwrap(),
            BirthActionOutcome::Changed
        );
        assert_eq!(
            draft
                .choices()
                .iter()
                .filter(|choice| choice.selected)
                .count(),
            usize::from(expected)
        );
    }
}

#[test]
fn exact_id_commands_preserve_boolean_and_literal_text_values() {
    let mut output = Vec::new();
    super::super::run(
        &mut b"focus creche.name\nedit value true\nactivate\nfocus creche.plot.0\nedit value true\nactivate\nfocus creche.plot.0\nedit value false\nactivate\nread all\nquit\n".as_slice(),
        &mut output,
    ).unwrap();
    let output = String::from_utf8(output).unwrap();
    assert!(!output.contains("Refused"), "{output}");
    assert!(output.contains("Current value: true."));
    assert_eq!(
        output.matches("Interaction: action=creche.plot.0").count(),
        2
    );
}

#[test]
fn review_command_reads_current_birth_choices_from_the_face() {
    let advertisement = StdHost::new().advertisement().clone();
    let door = ZeroBodyFrontDoor::from_model(
        Arc::new(HostedPatchbayAdapter),
        PatchbayModel::from_advertisement(advertisement.clone()),
    )
    .unwrap();
    let draft = door
        .creche_draft("00112233-4455-6677-8899-aabbccddeeff".into())
        .unwrap();
    let basis = HostOwnedBirthFaceBasis {
        host_id: advertisement.host_id.clone(),
        boot_id: advertisement.boot_id.clone(),
        encounter_id: "00112233-4455-6677-8899-aabbccddeeff".into(),
    };
    let mut execution = HostedTerminalMaskExecution::new(&advertisement).unwrap();
    let (face, show) =
        super::super::present(&draft, &basis, &mut execution, &mut Vec::new()).unwrap();
    let mut reader = SpokenFaceSession::new(face.clone(), show.clone()).unwrap();
    reader
        .command(
            &face,
            &show,
            parse_command("review", &reader, &face).unwrap(),
            1,
        )
        .unwrap();
    let readout = reader.take_text_readout().unwrap().unwrap();
    assert!(readout
        .clauses
        .iter()
        .any(|clause| clause.contains("Review Birth choices")));
    assert!(readout
        .clauses
        .iter()
        .any(|clause| clause.contains("Starting Plots selected:")));
    assert!(readout
        .clauses
        .iter()
        .any(|clause| clause.contains("Birth:")));
}
