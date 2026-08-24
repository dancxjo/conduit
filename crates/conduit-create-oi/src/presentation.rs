//! Exact bounded Create 1 OI v2 music and indicator presentation.
//!
//! This mechanism program is intentionally motion-free even if UART command
//! alignment is lost: neither Create drive opcode occurs anywhere in any
//! command, including song-event and LED payload bytes.

pub const PRESENTATION_LIGHT_STEPS: u8 = 8;
pub const PRESENTATION_SONG_PLAYS: u8 = 4;
pub const PRESENTATION_LIGHT_STEP_MS: u64 = 800;

pub const PRESENTATION_START: [u8; 1] = [128];
pub const PRESENTATION_FULL: [u8; 1] = [132];
pub const PRESENTATION_SAFE: [u8; 2] = [128, 131];
pub const PRESENTATION_LIGHTS_OFF: [u8; 4] = [139, 0, 0, 0];
pub const PRESENTATION_PLAY_SONG: [u8; 2] = [141, 3];

/// One original 16-event syncopated riff in Create song slot 3.
pub const PRESENTATION_DEFINE_SONG: [u8; 35] = [
    140, 3, 16, 52, 4, 0, 2, 52, 4, 55, 4, 57, 6, 0, 2, 59, 4, 57, 4, 55, 6, 0, 2, 52, 4, 0, 2, 64,
    4, 62, 4, 59, 4, 57, 8,
];

/// Match Netherwick's healthy Brainstem presentation: alternate PLAY and
/// ADVANCE while keeping the POWER LED at color 128 and full intensity.
pub const fn presentation_lights(step: u8) -> [u8; 4] {
    // These are Create 1 LED-command bits. The Create 1 button sensor packet
    // uses different bit positions and must not be used as an LED reference.
    let led_bits = if step & 1 == 0 {
        crate::CREATE_1_PLAY_LED_MASK
    } else {
        crate::CREATE_1_ADVANCE_LED_MASK
    };
    [139, led_bits, 128, 255]
}

pub const fn presentation_bytes_are_motion_free(bytes: &[u8]) -> bool {
    let mut index = 0;
    while index < bytes.len() {
        if bytes[index] == 137 || bytes[index] == 145 {
            return false;
        }
        index += 1;
    }
    true
}

const _: () = {
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_START));
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_FULL));
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_SAFE));
    assert!(presentation_bytes_are_motion_free(
        &PRESENTATION_DEFINE_SONG
    ));
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_PLAY_SONG));
    assert!(presentation_bytes_are_motion_free(&PRESENTATION_LIGHTS_OFF));
    let mut step = 0;
    while step < PRESENTATION_LIGHT_STEPS {
        assert!(presentation_bytes_are_motion_free(&presentation_lights(
            step
        )));
        step += 1;
    }
};

#[cfg(test)]
mod tests {
    use super::*;
    use std::vec::Vec;

    #[test]
    fn exact_program_contains_no_drive_opcode_at_any_alignment() {
        let mut bytes = Vec::new();
        bytes.extend_from_slice(&PRESENTATION_START);
        bytes.extend_from_slice(&PRESENTATION_FULL);
        bytes.extend_from_slice(&PRESENTATION_DEFINE_SONG);
        for step in 0..PRESENTATION_LIGHT_STEPS {
            if step & 1 == 0 {
                bytes.extend_from_slice(&PRESENTATION_PLAY_SONG);
            }
            bytes.extend_from_slice(&presentation_lights(step));
        }
        bytes.extend_from_slice(&PRESENTATION_LIGHTS_OFF);
        bytes.extend_from_slice(&PRESENTATION_SAFE);

        assert!(presentation_bytes_are_motion_free(&bytes));
        assert!(!bytes.contains(&137));
        assert!(!bytes.contains(&145));
    }

    #[test]
    fn light_pattern_matches_netherwick_healthy_supervision() {
        assert_eq!(presentation_lights(0), [139, 2, 128, 255]);
        assert_eq!(presentation_lights(1), [139, 8, 128, 255]);
        assert_eq!(presentation_lights(6), [139, 2, 128, 255]);
        assert_eq!(presentation_lights(7), [139, 8, 128, 255]);
        assert_eq!(PRESENTATION_LIGHT_STEP_MS, 800);
    }

    #[test]
    fn program_is_pinned_to_create_1_oi_v2() {
        assert_eq!(crate::CREATE_1_OI_PROTOCOL_VERSION, 2);
        assert_eq!(crate::CREATE_OI_BAUD, 57_600);
        assert_eq!(crate::CREATE_1_PLAY_LED_MASK, 0x02);
        assert_eq!(crate::CREATE_1_ADVANCE_LED_MASK, 0x08);
    }

    #[test]
    fn riff_fits_one_create_song_slot() {
        assert_eq!(PRESENTATION_DEFINE_SONG[0..3], [140, 3, 16]);
        assert_eq!(PRESENTATION_DEFINE_SONG.len(), 3 + 2 * 16);
        assert_eq!(PRESENTATION_PLAY_SONG, [141, 3]);
        assert_eq!(PRESENTATION_SONG_PLAYS, 4);
    }
}
