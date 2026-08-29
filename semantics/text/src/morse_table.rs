//! One shared International Morse lookup table for both directions.

pub(crate) fn symbols(value: u8) -> Option<&'static [u8]> {
    Some(match value.to_ascii_uppercase() {
        b'A' => b".-",
        b'B' => b"-...",
        b'C' => b"-.-.",
        b'D' => b"-..",
        b'E' => b".",
        b'F' => b"..-.",
        b'G' => b"--.",
        b'H' => b"....",
        b'I' => b"..",
        b'J' => b".---",
        b'K' => b"-.-",
        b'L' => b".-..",
        b'M' => b"--",
        b'N' => b"-.",
        b'O' => b"---",
        b'P' => b".--.",
        b'Q' => b"--.-",
        b'R' => b".-.",
        b'S' => b"...",
        b'T' => b"-",
        b'U' => b"..-",
        b'V' => b"...-",
        b'W' => b".--",
        b'X' => b"-..-",
        b'Y' => b"-.--",
        b'Z' => b"--..",
        b'0' => b"-----",
        b'1' => b".----",
        b'2' => b"..---",
        b'3' => b"...--",
        b'4' => b"....-",
        b'5' => b".....",
        b'6' => b"-....",
        b'7' => b"--...",
        b'8' => b"---..",
        b'9' => b"----.",
        _ => return None,
    })
}

pub(crate) fn character(value: &[u8]) -> Option<u8> {
    (b'A'..=b'Z')
        .chain(b'0'..=b'9')
        .find(|candidate| symbols(*candidate) == Some(value))
}
