//! Exact decimal fractions for verified fixed-width values. Formatting never
//! narrows a value to i128 or substitutes a floating approximation.
use conduit_core::{ExactRational, EXACT_RATIONAL_MAX_ENCODED_BYTES};
pub(super) fn format(value: ExactRational) -> Result<String, String> {
    let mut bytes = [0; EXACT_RATIONAL_MAX_ENCODED_BYTES];
    let n = value
        .encode(&mut bytes)
        .map_err(|e| format!("encode verified fraction: {e:?}"))?;
    let numerator_length = usize::from(u16::from_le_bytes(bytes[2..4].try_into().unwrap()));
    let numerator = decimal(&bytes[6..6 + numerator_length]);
    let denominator = decimal(&bytes[6 + numerator_length..n]);
    let sign = if bytes[1] == 0 { "" } else { "-" };
    Ok(if denominator == "1" {
        format!("{sign}{numerator}")
    } else {
        format!("{sign}{numerator}/{denominator}")
    })
}
fn decimal(bytes: &[u8]) -> String {
    // Inputs come only from ExactRational::encode: each magnitude is at most
    // 512 bytes / 1234 decimal digits. This post-Play renderer has finite work.
    let mut digits = vec![0_u8];
    for byte in bytes.iter().rev() {
        let mut carry = u16::from(*byte);
        for digit in &mut digits {
            let value = u16::from(*digit) * 256 + carry;
            *digit = (value % 10) as u8;
            carry = value / 10;
        }
        while carry != 0 {
            digits.push((carry % 10) as u8);
            carry /= 10;
        }
    }
    digits
        .into_iter()
        .rev()
        .map(|d| char::from(b'0' + d))
        .collect()
}
#[cfg(test)]
mod tests {
    use super::*;
    use conduit_core::ExactNumericWork;
    #[test]
    fn exact_signed_fractions_and_values_wider_than_u128_render_without_narrowing() {
        let mut work = ExactNumericWork::new(10_000_000);
        assert_eq!(format(ExactRational::ZERO).unwrap(), "0");
        assert_eq!(format(ExactRational::ONE).unwrap(), "1");
        assert_eq!(
            format(ExactRational::ratio(-7, 3, &mut work).unwrap()).unwrap(),
            "-7/3"
        );
        let wide = ExactRational::from_u128(u128::MAX)
            .checked_multiply(ExactRational::from_u128(u128::MAX), &mut work)
            .unwrap();
        assert_eq!(
            format(wide).unwrap(),
            "115792089237316195423570985008687907852589419931798687112530834793049593217025"
        );
        let high = ExactRational::from_i128(2)
            .checked_power(4095, &mut work)
            .unwrap();
        let maximum = high
            .checked_add(
                high.checked_subtract(ExactRational::ONE, &mut work)
                    .unwrap(),
                &mut work,
            )
            .unwrap();
        let text = format(maximum).unwrap();
        assert_eq!(text.len(), 1234);
        use sha2::{Digest, Sha256};
        assert_eq!(
            format!("{:x}", Sha256::digest(text.as_bytes())),
            "c8ff76dbc0c71335c48a7b764297d5f5724ec7c764a243b8f7c558046b1129be"
        );
        let inverse = ExactRational::ONE
            .checked_divide(maximum, &mut work)
            .unwrap();
        assert_eq!(format(inverse).unwrap(), format!("1/{text}"));
    }
}
