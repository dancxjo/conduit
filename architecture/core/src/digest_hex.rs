//! Canonical hexadecimal digest output; no identity or authority admission.
use alloc::string::String;
pub(crate) fn hex_digest(digest: &[u8]) -> String {
    const DIGITS: &[u8; 16] = b"0123456789abcdef";
    let mut encoded = alloc::vec![0; digest.len() * 2];
    for (byte, pair) in digest.iter().zip(encoded.as_chunks_mut::<2>().0) {
        pair[0] = DIGITS[(byte >> 4) as usize];
        pair[1] = DIGITS[(byte & 15) as usize];
    }
    // Every initialized byte comes from the ASCII table above. Keep the safe
    // UTF-8 check while avoiding sixty-four String::push calls per digest.
    String::from_utf8(encoded).expect("hex digits are ASCII")
}
