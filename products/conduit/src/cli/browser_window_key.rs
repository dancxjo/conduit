use base64::{engine::general_purpose::STANDARD, Engine};

pub(crate) fn parse_browser_verifying_key(text: &str) -> Result<[u8; 32], String> {
    let text = text.trim();
    let invalid = || {
        "browser verifying key must encode exactly 32 bytes as Base64 or a JSON byte array"
            .to_owned()
    };
    if text.starts_with('[') {
        return serde_json::from_str(text).map_err(|_| invalid());
    }
    let mut decoded = [0; 33];
    if STANDARD
        .decode_slice(text, &mut decoded)
        .map_err(|_| invalid())?
        != 32
    {
        return Err(invalid());
    }
    let mut key = [0; 32];
    key.copy_from_slice(&decoded[..32]);
    Ok(key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn browser_key_base64_and_legacy_json_preserve_the_same_bytes() {
        let key = core::array::from_fn::<_, 32, _>(|index| index as u8);
        assert_eq!(
            parse_browser_verifying_key("AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh8=").unwrap(),
            key
        );
        assert_eq!(
            parse_browser_verifying_key(&serde_json::to_string(&key).unwrap()).unwrap(),
            key
        );
    }

    #[test]
    fn browser_key_base64_refuses_wrong_lengths_and_noncanonical_encodings() {
        for text in [
            "",
            "AA==",
            "[]",
            "[256]",
            "!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!!",
            "AAECAwQFBgcICQoLDA0ODxAREhMUFRYXGBkaGxwdHh9=",
        ] {
            assert!(parse_browser_verifying_key(text).is_err(), "{text}");
        }
        let mut oversized = [0; 44];
        STANDARD.encode_slice([1; 33], &mut oversized).unwrap();
        assert!(parse_browser_verifying_key(core::str::from_utf8(&oversized).unwrap()).is_err());
    }
}
