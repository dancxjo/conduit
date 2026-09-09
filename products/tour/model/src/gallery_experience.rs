//! Editorial guidance for the reviewed Forms. This describes their current
//! local experience; execution, output and availability remain runtime truth.
pub fn gallery_experience(name: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match name {
        "firefly-choir" => (
            "Light & rhythm",
            "A tiny chorus of light, learning to keep time.",
            "Press Run once and watch the chorus keep pulsing. Press Stop when you are done.",
            "Pulse five and later stay in the same Play. This local composition does not connect peers or play sound.",
        ),
        "morse_network" => (
            "Messages in light",
            "Turn a distress call into a constellation of dots and dashes.",
            "Press Run once, focus the keyboard control, and type several characters to send each one in light.",
            "The same Play keeps listening between characters. Press Stop when you are done.",
        ),
        "memory_lantern" => (
            "A first small program",
            "Give a thought a place to glow.",
            "Press Run once, focus the keyboard control, and keep typing or editing the current text.",
            "Each edit updates bounded retained text in the same Play. Press Stop when you are done.",
        ),
        "desk_telegraph" => (
            "Inside a message",
            "Pack a message, pass it along, and unwrap it intact.",
            "Press Run once, type a message, and press Enter. Send another before pressing Stop.",
            "Each submission crosses the typed-record and finite-queue path. This local desk is not connected remotely.",
        ),
        "night-radio" => (
            "A miniature broadcast",
            "A late-night dispatch, carried from one end of a Form to the other.",
            "Press Run once, type a report, and press Enter. Dispatch another in the same Play, then press Stop.",
            "Each report crosses framing, a finite queue, and decoding. This station does not claim radio or audio service.",
        ),
        "secret-knock-demo" => (
            "Patterns in time",
            "Can your fingers remember a rhythm?",
            "Press Run, then knock three times: a short gap followed by a gap three times as long. Finish within one second.",
            "The bounded attempt resets after its result. Try another rhythm in the same Play, then press Stop.",
        ),
        "pocket-theremin" => (
            "Motion into numbers",
            "Explore the space between a gesture and a frequency.",
            "Press Run once, then choose several horizontal positions to map them from 20 to 20,000 Hz.",
            "Later positions stay in the same Play. This Form displays frequency but does not produce sound; press Stop when done.",
        ),
        "button_across_room" => (
            "Touch becomes light",
            "Each touch becomes light through a connection you can see.",
            "Press Run once, then press and release the control repeatedly. Press Stop when you are done.",
            "The indicator is here in your browser. A physical light across the room requires a connected Host.",
        ),
        _ => (
            "Explore a Form",
            "A reviewed composition.",
            "Open the source to explore this Form.",
            "",
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use alloc::format;

    #[test]
    fn every_reviewed_experience_teaches_run_once_interact_until_stop() {
        for name in [
            "firefly-choir",
            "morse_network",
            "memory_lantern",
            "desk_telegraph",
            "night-radio",
            "secret-knock-demo",
            "pocket-theremin",
            "button_across_room",
        ] {
            let (_, _, instruction, note) = gallery_experience(name);
            let guidance = format!("{instruction} {note}");
            assert!(guidance.contains("Press Run"), "{name}: {guidance}");
            assert!(guidance.contains("Stop"), "{name}: {guidance}");
            for stale in ["run again", "four timed pulses", "one pointer position"] {
                assert!(
                    !guidance.to_ascii_lowercase().contains(stale),
                    "{name}: {guidance}"
                );
            }
        }
    }
}
