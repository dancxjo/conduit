//! Editorial guidance for the reviewed Forms. This describes their current
//! local experience; execution, output and availability remain runtime truth.
pub fn gallery_experience(name: &str) -> (&'static str, &'static str, &'static str, &'static str) {
    match name {
        "firefly-choir" => (
            "Light & rhythm",
            "A tiny chorus of light, learning to keep time.",
            "Press Run to watch four timed pulses and see a rhythm follow them.",
            "This local composition shows light and rhythm. It does not connect peers or play sound.",
        ),
        "morse_network" => (
            "Messages in light",
            "Turn a distress call into a constellation of dots and dashes.",
            "Press Run. Watch SOS become three short flashes, three long flashes, then three short flashes.",
            "Change the message below to send your own words through the same Form.",
        ),
        "memory_lantern" => (
            "A first small program",
            "Give a thought a place to glow.",
            "Press Run to display READY, or type your own message below and run it again.",
            "The current Form displays one message. It does not yet save a collection of memories.",
        ),
        "desk_telegraph" => (
            "Inside a message",
            "Pack a message, pass it along, and unwrap it intact.",
            "Press Run to send CALLING through a typed record and a finite queue. Inspect the wiring to follow its journey.",
            "This run stays in your browser. A remote desk is not connected.",
        ),
        "night-radio" => (
            "A miniature broadcast",
            "A late-night dispatch, carried from one end of a Form to the other.",
            "Press Run to carry NIGHT REPORT through framing, a queue, and decoding, then read the result.",
            "This is a local text station. It does not stream audio or connect to a radio service.",
        ),
        "secret-knock-demo" => (
            "Patterns in time",
            "Can your fingers remember a rhythm?",
            "Press Run, then knock three times: a short gap followed by a gap three times as long. Finish within one second.",
            "The result compares your timing with the stored pattern. Run again to try another rhythm.",
        ),
        "pocket-theremin" => (
            "Motion into numbers",
            "Explore the space between a gesture and a frequency.",
            "Press Run, then click a horizontal position on the control to map one pointer position to a frequency from 20 to 20,000 Hz.",
            "This Form displays a frequency; it does not produce sound. Run again to sample another position.",
        ),
        "button_across_room" => (
            "Touch becomes light",
            "One press. One light. A connection you can see.",
            "Press Run, then hold the control button to light the indicator. Release to turn it off.",
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
