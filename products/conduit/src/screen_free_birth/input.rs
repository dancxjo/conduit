//! Finite screen-free command input over the current semantic Face.

use std::io::BufRead;

use conduit_core::kind_id;
use conduit_presentation::{FaceUtteranceProvenance, Presentation, PresentationRole};
use conduit_std_host::spoken_face_mask::{ReaderCommand, SpokenFaceSession};

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;

pub(super) const MAX_SCREEN_FREE_COMMAND_BYTES: usize = 4_096;
#[cfg(test)]
pub(super) const SCREEN_FREE_COMMANDS: &str = "Commands: help, summary, read current items, read all, review, next, previous, repeat, next/previous subject, next/previous main, article, or navigation, next/previous action, focus subject ID, focus ACTION, edit value TEXT, activate, stop, quit.";

/// Bound one command without allocating an arbitrarily long terminal line.
/// A rejected line is consumed completely so its tail cannot become an action.
pub(super) fn read_command_line(
    input: &mut impl BufRead,
) -> std::io::Result<Result<Option<String>, &'static str>> {
    let mut bytes = Vec::new();
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            if bytes.is_empty() {
                return Ok(Ok(None));
            }
            break;
        }
        let (take, ended) = match available.iter().position(|byte| *byte == b'\n') {
            Some(index) => (index + 1, true),
            None => (available.len(), false),
        };
        if bytes.len().saturating_add(take) > MAX_SCREEN_FREE_COMMAND_BYTES {
            input.consume(take);
            if !ended {
                discard_command_tail(input)?;
            }
            return Ok(Err("command is too long"));
        }
        bytes.extend_from_slice(&available[..take]);
        input.consume(take);
        if ended {
            break;
        }
    }
    match String::from_utf8(bytes) {
        Ok(line) => Ok(Ok(Some(line.trim_end_matches(['\r', '\n']).into()))),
        Err(_) => Ok(Err("command must be UTF-8")),
    }
}

fn discard_command_tail(input: &mut impl BufRead) -> std::io::Result<()> {
    loop {
        let available = input.fill_buf()?;
        if available.is_empty() {
            return Ok(());
        }
        let (take, ended) = match available.iter().position(|byte| *byte == b'\n') {
            Some(index) => (index + 1, true),
            None => (available.len(), false),
        };
        input.consume(take);
        if ended {
            return Ok(());
        }
    }
}

pub(super) fn parse_command(
    line: &str,
    reader: &SpokenFaceSession,
    face: &Presentation,
) -> Result<ReaderCommand, &'static str> {
    match line {
        "help" => Ok(ReaderCommand::Help),
        "summary" => Ok(ReaderCommand::Summary),
        "read all" => Ok(ReaderCommand::ReadAll),
        "read current items" => Ok(ReaderCommand::ReadCurrentItems),
        "review"
            if face
                .subjects
                .iter()
                .any(|subject| subject.name == "Review Birth choices") =>
        {
            Ok(ReaderCommand::ReadAll)
        }
        "next" => Ok(ReaderCommand::Next),
        "previous" => Ok(ReaderCommand::Previous),
        "repeat" => Ok(ReaderCommand::Repeat),
        "next subject" => Ok(ReaderCommand::NextSubject),
        "previous subject" => Ok(ReaderCommand::PreviousSubject),
        "next action" => Ok(ReaderCommand::NextAction),
        "previous action" => Ok(ReaderCommand::PreviousAction),
        "next main" => Ok(ReaderCommand::NextRole(PresentationRole::Semantic(
            kind_id("document/main"),
        ))),
        "previous main" => Ok(ReaderCommand::PreviousRole(PresentationRole::Semantic(
            kind_id("document/main"),
        ))),
        "next article" => Ok(ReaderCommand::NextRole(PresentationRole::Semantic(
            kind_id("document/article"),
        ))),
        "previous article" => Ok(ReaderCommand::PreviousRole(PresentationRole::Semantic(
            kind_id("document/article"),
        ))),
        "next navigation" => Ok(ReaderCommand::NextRole(PresentationRole::Semantic(
            kind_id("document/navigation"),
        ))),
        "previous navigation" => Ok(ReaderCommand::PreviousRole(PresentationRole::Semantic(
            kind_id("document/navigation"),
        ))),
        "activate" => Ok(ReaderCommand::Activate),
        "stop" => Ok(ReaderCommand::Stop),
        _ => {
            if let Some(subject) = line.strip_prefix("focus subject ") {
                if subject.is_empty() {
                    return Err("focus subject needs an exact subject ID");
                }
                return Ok(ReaderCommand::FocusSubject(subject.into()));
            }
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
                let action_id = match &reader.focused_clause().provenance {
                    FaceUtteranceProvenance::Action(provenance) => Some(provenance.identity()),
                    FaceUtteranceProvenance::ActionArgument(provenance) => {
                        Some(provenance.action_identity())
                    }
                    _ => None,
                };
                let boolean = action_id
                    .and_then(|identity| {
                        face.actions
                            .iter()
                            .find(|action| &action.identity == identity)
                    })
                    .and_then(|action| action.arguments.iter().find(|item| item.name == argument))
                    .is_some_and(|item| item.contract.value_kind.as_str() == "value/bool");
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
