//! Initial reading order for zero-Body and retained-Body nonvisual encounters.

use conduit_std_host::spoken_face_mask::ReaderCommand;

pub(super) fn opening_commands(spoken: bool) -> impl Iterator<Item = ReaderCommand> {
    // The Crèche starts at its current name control; a person can request
    // every clause without waiting through it before their first edit.
    spoken
        .then_some(ReaderCommand::Help)
        .into_iter()
        .chain(std::iter::once(if spoken {
            ReaderCommand::Repeat
        } else {
            ReaderCommand::ReadAll
        }))
        .chain(spoken.then_some(ReaderCommand::FocusAction("creche.name".into())))
}

/// A returning spoken user gets immediate orientation, then chooses whether
/// to read the whole view. Reading it automatically can outlast a live Play
/// and make its current Stop action unreachable through nonvisual input.
pub(super) fn opening_body_commands(spoken: bool) -> impl Iterator<Item = ReaderCommand> {
    std::iter::once(if spoken {
        ReaderCommand::Repeat
    } else {
        ReaderCommand::ReadAll
    })
}
