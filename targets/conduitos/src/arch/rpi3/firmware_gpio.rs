//! Firmware expander responses carry a status in place of the requested pin.
//! SET may return only that status; GET independently verifies the output.
//! The pinned firmware returns length 8 without a response bit on this tag.
//! As in Linux gpio-raspberrypi-exp, message success and the overwritten pin
//! status establish completion; an untouched/ignored request retains pin 131.
const SET_STATE: u32 = 0x38041;
const GET_STATE: u32 = 0x30041;
pub(super) const fn request(pin: u8, set: bool) -> [u32; 8] {
    [
        32,
        0,
        if set { SET_STATE } else { GET_STATE },
        8,
        8,
        128 + pin as u32,
        if set { 1 } else { 0 },
        0,
    ]
}
pub(super) fn accepted(words: &[u32; 8], set: bool) -> bool {
    let length = words[4] & 0x7fff_ffff;
    words[0] == 32
        && words[1] == 0x8000_0000
        && words[2] == if set { SET_STATE } else { GET_STATE }
        && words[3] == 8
        && (length == 8 || (set && length == 4))
        && words[5] == 0
        && words[7] == 0
        && (set || words[6] == 1)
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn set_status_is_not_an_echo_and_readback_must_be_high() {
        let mut set = request(3, true);
        assert_eq!(set[5], 131);
        assert!(!accepted(&set, true));
        set[1] = 0x8000_0000;
        set[4] = 0x8000_0004;
        set[5] = 0;
        set[6] = 0; // SET status alone does not establish physical state.
        assert!(accepted(&set, true));
        set[4] = 0x8000_0008;
        assert!(accepted(&set, true));
        set[4] = 8; // Observed physical reply: status 0, unflagged length.
        assert!(accepted(&set, true));
        set[5] = 131; // Ignored tag leaves the requested pin unchanged.
        assert!(!accepted(&set, true));
        set[5] = u32::MAX;
        assert!(!accepted(&set, true));
        let mut get = request(3, false);
        get[1] = 0x8000_0000;
        get[4] = 0x8000_0008;
        get[5] = 0;
        assert!(!accepted(&get, false));
        get[6] = 1;
        assert!(accepted(&get, false));
        get[4] = 8;
        assert!(accepted(&get, false));
        for index in [0, 1, 2, 3, 4, 5, 6, 7] {
            let mut bad = get;
            bad[index] ^= 1;
            assert!(!accepted(&bad, false), "word {index}");
        }
        get[4] = 0x8000_0004;
        assert!(!accepted(&get, false));
    }
}
