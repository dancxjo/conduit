//! Deterministic Orbium seed construction for a bounded scalar field.

use alloc::{vec, vec::Vec};
use sha2::{Digest, Sha256};

use crate::{
    validate_lenia_dimensions, LeniaFieldId, LeniaFieldState, LeniaRefusal, LENIA_Q16_ONE,
};

const ORBIUM_SEED_DOMAIN: &[u8] = b"conduit.alife.orbium-seed.v1";

// Orbium unicaudatus specimen O2u from Chakazul/Lenia (MIT),
// https://github.com/Chakazul/Lenia, quantized by that source catalog to 255
// levels. Conduit decodes it into exact Q16.16 values.
const ORBIUM_RLE: &str = "7.MD6.qL$6.pKqEqFURpApBRAqQ$5.VqTrSsBrOpXpWpTpWpUpCrQ$4.CQrQsTsWsApITNPpGqGvL$3.IpIpWrOsGsBqXpJ4.LsFrL$A.DpKpSpJpDqOqUqSqE5.ExD$qL.pBpTT2.qCrGrVrWqM5.sTpP$.pGpWpD3.qUsMtItQtJ6.tL$.uFqGH3.pXtOuR2vFsK5.sM$.tUqL4.GuNwAwVxBwNpC4.qXpA$2.uH5.vBxGyEyMyHtW4.qIpL$2.wV5.tIyG3yOxQqW2.FqHpJ$2.tUS4.rM2yOyJyOyHtVpPMpFqNV$2.HsR4.pUxAyOxLxDxEuVrMqBqGqKJ$3.sLpE3.pEuNxHwRwGvUuLsHrCqTpR$3.TrMS2.pFsLvDvPvEuPtNsGrGqIP$4.pRqRpNpFpTrNtGtVtStGsMrNqNpF$5.pMqKqLqRrIsCsLsIrTrFqJpHE$6.RpSqJqPqVqWqRqKpRXE$8.OpBpIpJpFTK!";

pub fn orbium_seed(width: u16, height: u16, seed: u64) -> Result<LeniaFieldState, LeniaRefusal> {
    let count = validate_lenia_dimensions(width, height)?;
    let pattern = decode_pattern()?;
    let pattern_height = pattern.len();
    let pattern_width = pattern.iter().map(Vec::len).max().unwrap_or(0);
    if pattern_width == 0
        || pattern_height == 0
        || pattern_width > usize::from(width)
        || pattern_height > usize::from(height)
    {
        return Err(LeniaRefusal::InvalidSeed);
    }
    let mut cells = vec![0; count];
    let jitter_x = ((seed.wrapping_mul(17) % 9) as isize) - 4;
    let jitter_y = ((seed.wrapping_mul(29) % 9) as isize) - 4;
    let origin_x = (isize::try_from(usize::from(width) - pattern_width)
        .map_err(|_| LeniaRefusal::ArithmeticOverflow)?
        / 2)
        + jitter_x;
    let origin_y = (isize::try_from(usize::from(height) - pattern_height)
        .map_err(|_| LeniaRefusal::ArithmeticOverflow)?
        / 2)
        + jitter_y;
    let mirrored = seed & 1 == 0;
    for (y, row) in pattern.iter().enumerate() {
        for (x, value) in row.iter().copied().enumerate() {
            let pattern_x = if mirrored { pattern_width - 1 - x } else { x };
            let destination_x = (origin_x + pattern_x as isize).rem_euclid(width as isize) as usize;
            let destination_y = (origin_y + y as isize).rem_euclid(height as isize) as usize;
            cells[destination_y * usize::from(width) + destination_x] = value;
        }
    }
    let mut digest = Sha256::new();
    digest.update(ORBIUM_SEED_DOMAIN);
    digest.update(width.to_le_bytes());
    digest.update(height.to_le_bytes());
    digest.update(seed.to_le_bytes());
    let digest: [u8; 32] = digest.finalize().into();
    let mut field_id = [0; 16];
    field_id.copy_from_slice(&digest[..16]);
    LeniaFieldState::from_cells(LeniaFieldId(field_id), 0, width, height, cells)
}

fn decode_pattern() -> Result<Vec<Vec<u32>>, LeniaRefusal> {
    let mut rows = vec![Vec::new()];
    let mut count = 0_usize;
    let mut prefix = None;
    for character in ORBIUM_RLE.chars() {
        if character.is_ascii_digit() {
            count = count
                .checked_mul(10)
                .and_then(|value| value.checked_add(character.to_digit(10)? as usize))
                .ok_or(LeniaRefusal::ArithmeticOverflow)?;
            continue;
        }
        if matches!(character, 'p'..='y' | '@') {
            prefix = Some(character);
            continue;
        }
        if character == '$' || character == '!' {
            if prefix.is_some() || count != 0 {
                return Err(LeniaRefusal::InvalidSeed);
            }
            if character == '$' {
                rows.push(Vec::new());
            }
            continue;
        }
        let value = decode_value(prefix.take(), character)?;
        let repeats = if count == 0 { 1 } else { count };
        count = 0;
        let q16 = (u64::from(value) * u64::from(LENIA_Q16_ONE) + 127) / 255;
        rows.last_mut()
            .ok_or(LeniaRefusal::InvalidSeed)?
            .extend(core::iter::repeat_n(q16 as u32, repeats));
    }
    if rows.last().is_some_and(Vec::is_empty) {
        rows.pop();
    }
    Ok(rows)
}

fn decode_value(prefix: Option<char>, character: char) -> Result<u16, LeniaRefusal> {
    if character == '.' || character == 'b' {
        return prefix
            .is_none()
            .then_some(0)
            .ok_or(LeniaRefusal::InvalidSeed);
    }
    if character == 'o' {
        return prefix
            .is_none()
            .then_some(255)
            .ok_or(LeniaRefusal::InvalidSeed);
    }
    if !character.is_ascii_uppercase() {
        return Err(LeniaRefusal::InvalidSeed);
    }
    let suffix = character as u16 - 'A' as u16;
    let value = match prefix {
        None => suffix + 1,
        Some(prefix @ 'p'..='y') => (prefix as u16 - 'p' as u16) * 24 + suffix + 25,
        Some('@') => 10 * 24 + suffix + 25,
        _ => return Err(LeniaRefusal::InvalidSeed),
    };
    (value <= 255)
        .then_some(value)
        .ok_or(LeniaRefusal::InvalidSeed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn orbium_seed_is_finite_repeatable_and_seed_distinct() {
        let first = orbium_seed(128, 128, 1).unwrap();
        let repeated = orbium_seed(128, 128, 1).unwrap();
        let distinct = orbium_seed(128, 128, 2).unwrap();
        assert_eq!(first, repeated);
        assert_ne!(first.field_id, distinct.field_id);
        assert_eq!(first.cells().len(), 16_384);
        assert!(first.cells().iter().any(|value| *value != 0));
    }
}
