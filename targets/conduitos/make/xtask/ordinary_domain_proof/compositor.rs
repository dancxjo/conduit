//! Strict fixture receipt validation; no graphical product isolation claim.
use super::super::ConduitosError;
use serde_json::Value;

const PREFIX: &str = "CONDUIT_COMPOSITOR_GUEST_PROOF ";
const STRINGS: [(&str, &str); 5] = [
    ("schema", "conduit.conduitos/compositor-guest-fixture@2"),
    ("status", "completed"),
    ("proof_class", "freestanding-emulator"),
    ("architecture", "x86_64"),
    ("privilege", "ring3"),
];
const FLAGS: [(&str, bool); 11] = [
    ("fixture", true),
    ("production_compositor", false),
    ("physical_scanout", false),
    ("semantic_show_acknowledged", false),
    ("driver_isolation", false),
    ("dma_isolation", false),
    ("old_instance_quarantined", true),
    ("replacement_while_old_alive", true),
    ("stale_drop_preserves_replacement", true),
    ("fresh_instance_empty", true),
    ("old_handle_rejected", true),
];
const COUNTS: [(&str, u64); 5] = [
    ("pixel_comparisons", 832),
    ("initial_pixels", 768),
    ("removed_rectangle_pixels", 64),
    ("frame_steps", 136),
    ("fixture_acknowledgements", 2),
];

pub(super) fn validate(transcript: &str) -> Result<Value, ConduitosError> {
    let mut receipts = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let receipt = receipts
        .next()
        .ok_or_else(|| refusal("compositor-guest-fixture-absent"))?;
    if receipts.next().is_some() {
        return Err(refusal("compositor-guest-fixture-duplicated"));
    }
    let value: Value =
        serde_json::from_str(receipt).map_err(|_| refusal("compositor-guest-fixture-malformed"))?;
    let object = value
        .as_object()
        .ok_or_else(|| refusal("compositor-guest-fixture-malformed"))?;
    // Reject extra claims as well as missing fields: widening this fixture's
    // scope requires a reviewed schema and collector change.
    if object.len() != STRINGS.len() + FLAGS.len() + COUNTS.len() + 1
        || STRINGS
            .iter()
            .any(|(field, expected)| value[*field].as_str() != Some(*expected))
        || FLAGS
            .iter()
            .any(|(field, expected)| value[*field].as_bool() != Some(*expected))
        || COUNTS
            .iter()
            .any(|(field, expected)| value[*field].as_u64() != Some(*expected))
    {
        return Err(refusal("compositor-guest-fixture-mismatch"));
    }
    // This is the fixture's SHA-256 of its embedded guest ELF, not an independently
    // fetched artifact comparison. The tested Root embeds that exact image.
    let digest = value["image_sha256"]
        .as_str()
        .ok_or_else(|| refusal("compositor-guest-fixture-image-digest"))?;
    if digest.len() != 64
        || !digest
            .bytes()
            .all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte))
    {
        return Err(refusal("compositor-guest-fixture-image-digest"));
    }
    Ok(value)
}

fn refusal(reason: &'static str) -> ConduitosError {
    ConduitosError::refusal(
        reason,
        "requires the complete independent compositor guest fixture; no production compositor, physical scanout, semantic Show, driver or DMA isolation claim",
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Value {
        let mut value = serde_json::json!({"image_sha256": "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef"});
        for (field, expected) in STRINGS {
            value[field] = Value::from(expected);
        }
        for (field, expected) in FLAGS {
            value[field] = Value::from(expected);
        }
        for (field, expected) in COUNTS {
            value[field] = Value::from(expected);
        }
        value
    }
    fn transcript(value: &Value) -> String {
        format!("{PREFIX}{value}\n")
    }
    #[test]
    fn requires_exactly_one_complete_receipt() {
        let value = sample();
        let text = transcript(&value);
        assert_eq!(validate(&text).unwrap(), value);
        assert!(validate("").is_err());
        assert!(validate(&format!("{text}{text}")).is_err());
        assert!(validate(&format!("{PREFIX}{{invalid\n")).is_err());
        for field in value.as_object().unwrap().keys() {
            let mut changed = value.clone();
            changed.as_object_mut().unwrap().remove(field);
            assert!(validate(&transcript(&changed)).is_err(), "missing {field}");
        }
    }
    #[test]
    fn rejects_each_count_tamper() {
        for (field, count) in COUNTS {
            let mut changed = sample();
            changed[field] = Value::from(count + 1);
            assert!(validate(&transcript(&changed)).is_err(), "{field}");
        }
    }
    #[test]
    fn preserves_every_proof_boundary_and_rejects_extra_claims() {
        for (field, flag) in FLAGS {
            let mut changed = sample();
            changed[field] = Value::from(!flag);
            assert!(validate(&transcript(&changed)).is_err(), "{field}");
        }
        let mut changed = sample();
        changed["graphical_product_isolated"] = Value::Bool(true);
        assert!(validate(&transcript(&changed)).is_err());
    }
    #[test]
    fn rejects_noncanonical_image_hashes() {
        for digest in [
            "",
            "abc",
            &"a".repeat(63),
            &"a".repeat(65),
            &"A".repeat(64),
            &"g".repeat(64),
            &format!("{} ", "0".repeat(63)),
        ] {
            let mut changed = sample();
            changed["image_sha256"] = Value::from(digest);
            assert!(validate(&transcript(&changed)).is_err(), "{digest}");
        }
        let mut changed = sample();
        changed["image_sha256"] = Value::Null;
        assert!(validate(&transcript(&changed)).is_err());
    }
}
