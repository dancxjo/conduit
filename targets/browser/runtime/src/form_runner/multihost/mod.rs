//! Shared executable-tour runner extended across two exact browser fragments.

mod abi;
mod plan;
mod protocol;
mod session;

#[cfg(test)]
mod night_radio_tests;
#[cfg(test)]
mod tests;

#[cfg(test)]
mod button_tests;

#[cfg(test)]
fn submit_ascii_line(
    session: &mut session::Session,
    mut output: protocol::Output,
    text: &str,
) -> protocol::Output {
    for byte in text.bytes().chain(std::iter::once(b'\n')) {
        let (usage, modifiers) = match byte {
            b'a'..=b'z' => (byte - b'a' + 4, 0),
            b'A'..=b'Z' => (
                byte - b'A' + 4,
                conduit_human::KeyModifiers::LEFT_SHIFT.bits(),
            ),
            b' ' => (44, 0),
            b'\n' => (40, 0),
            _ => panic!("unsupported test input byte {byte}"),
        };
        for transition in [0, 1] {
            let protocol::Output::Input { input, .. } = output else {
                if transition == 1 && byte == b'\n' {
                    break;
                }
                panic!("standing keyboard did not request the next key event")
            };
            assert_eq!(input.effect_kind, "key-event");
            let play = input.active_play_id.clone();
            output = session
                .complete_input(
                    &play,
                    input.request_sequence,
                    &[usage, transition, modifiers],
                )
                .unwrap();
        }
    }
    output
}
