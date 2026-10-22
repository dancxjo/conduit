//! Exact hostile-artifact broker receipt. No product guest diagnostic branch.
use super::super::ConduitosError;
use serde_json::Value;
const PREFIX: &str = "CONDUIT_COMPOSITOR_HOSTILE_PROOF ";
fn expected() -> Value {
    let cases = [
        ("envelope", "protocol", 0),
        ("shape", "protocol", 0),
        ("fault", "terminated", 0),
        ("loop", "terminated", 1),
    ];
    serde_json::json!({"schema":"conduit.conduitos/compositor-hostile-broker-fixture@1",
        "status":"completed","proof_class":"freestanding-emulator","fixture":true,
        "production_guest_modified":false,"direct_memory_attack":false,
        "semantic_show_acknowledged":false,"physical_scanout":false,
        "cases":cases.into_iter().map(|(name,error,preemptions)|serde_json::json!({
            "case":name,"error":error,"entries":2,"preemptions":preemptions,
            "root_sentinel_preserved":true,"sibling_sentinel_preserved":true,
            "fresh_production_guest":true,"image_sha256":""})).collect::<Vec<_>>()})
}
pub(super) fn validate(transcript: &str) -> Result<Value, ConduitosError> {
    let refusal = || {
        ConduitosError::refusal(
            "compositor-hostile-broker-fixture-mismatch",
            "requires exact separate hostile-artifact broker outcomes; sentinel preservation is not a direct memory-attack test or semantic Show/physical display receipt",
        )
    };
    let mut lines = transcript
        .lines()
        .filter_map(|line| line.strip_prefix(PREFIX));
    let mut actual: Value =
        serde_json::from_str(lines.next().ok_or_else(refusal)?).map_err(|_| refusal())?;
    let original = actual.clone();
    let cases = actual["cases"].as_array_mut().ok_or_else(refusal)?;
    if cases.len() != 4 || lines.next().is_some() {
        return Err(refusal());
    }
    for case in cases {
        let digest = case["image_sha256"].as_str().ok_or_else(refusal)?;
        if digest.len() != 64
            || !digest
                .bytes()
                .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
        {
            return Err(refusal());
        }
        case["image_sha256"] = Value::from("");
    }
    if actual != expected() {
        return Err(refusal());
    }
    // Digests bind the fixture's embedded proof ELFs. They are not independently
    // fetched remote artifacts or hashes of the unchanged production guest.
    Ok(original)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn sample() -> Value {
        let mut value = expected();
        for case in value["cases"].as_array_mut().unwrap() {
            case["image_sha256"] = Value::from("ab".repeat(32));
        }
        value
    }
    fn text(value: &Value) -> String {
        format!("{PREFIX}{value}\n")
    }
    #[test]
    fn exact_separate_cases_and_nonclaims_are_required() {
        let value = sample();
        let transcript = text(&value);
        assert_eq!(validate(&transcript).unwrap(), value);
        for invalid in [
            String::new(),
            format!("{transcript}{transcript}"),
            format!("{PREFIX}{{\n"),
        ] {
            assert!(validate(&invalid).is_err());
        }
        for field in value.as_object().unwrap().keys() {
            let mut bad = value.clone();
            bad[field] = Value::Null;
            assert!(validate(&text(&bad)).is_err());
        }
        for i in 0..4 {
            for field in value["cases"][i].as_object().unwrap().keys() {
                let mut bad = value.clone();
                bad["cases"][i][field] = Value::Null;
                assert!(validate(&text(&bad)).is_err(), "case{i}:{field}");
            }
        }
        let mut bad = value.clone();
        bad["direct_memory_attack"] = Value::Bool(true);
        assert!(validate(&text(&bad)).is_err());
        let mut bad = value.clone();
        bad["cases"][3]["preemptions"] = Value::from(0);
        assert!(validate(&text(&bad)).is_err());
        let mut bad = value.clone();
        bad["extra_claim"] = Value::Bool(true);
        assert!(validate(&text(&bad)).is_err());
    }
}
