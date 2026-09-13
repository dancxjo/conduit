//! The complete transition sequence, lifecycle identity and bounded output window.
use super::{journey_records, ConduitosError, ALPHABET, CHARACTERS};
use serde_json::{json, Value};

pub(super) fn expected_text(characters: usize) -> String {
    ((characters.saturating_sub(128))..characters)
        .map(|index| char::from(ALPHABET[index % ALPHABET.len()].to_ascii_uppercase()))
        .collect()
}

pub(super) fn validate(serial: &str) -> Result<Value, ConduitosError> {
    let refusal = || {
        ConduitosError::refusal("keyboard-repeat-causality-invalid", "every repeated press/release must retain exact Body/Plan/Play and the bounded result through explicit Stop/Lull")
    };
    let records = journey_records::decode(serial)?;
    let quiescent = records
        .iter()
        .filter(|record| record["status"] == "quiescent-awaiting-input")
        .collect::<Vec<_>>();
    if quiescent.len() != CHARACTERS * 2 + 1 || serial.contains("CONDUIT_KERNEL_SIGN") {
        return Err(refusal());
    }
    for (count, record) in quiescent.iter().enumerate() {
        let characters = count.div_ceil(2);
        if record["input_count"] != count as u64
            || (count == 0 && record.get("result") != Some(&Value::Null))
            || (count != 0 && record["result"] != expected_text(characters))
            || record["result_omitted_bytes"] != characters.saturating_sub(128) as u64
        {
            return Err(refusal());
        }
        for field in [
            "host_id",
            "boot_id",
            "source_document_id",
            "checked_form_id",
            "expanded_form_id",
            "body_id",
            "wake_id",
            "plan_id",
            "active_play_id",
        ] {
            if quiescent[0][field].as_str().is_none_or(str::is_empty)
                || record[field] != quiescent[0][field]
            {
                return Err(refusal());
            }
        }
    }
    let final_play = quiescent.last().ok_or_else(refusal)?;
    for status in ["stopped", "lulled"] {
        let matching = records
            .iter()
            .filter(|record| record["status"] == status)
            .collect::<Vec<_>>();
        if matching.len() != 1 {
            return Err(refusal());
        }
        for field in [
            "body_id",
            "wake_id",
            "plan_id",
            "active_play_id",
            "input_count",
            "result",
            "result_omitted_bytes",
        ] {
            if matching[0][field] != final_play[field] {
                return Err(refusal());
            }
        }
    }
    let reports = serial
        .lines()
        .filter(|line| *line == "CONDUIT_BOOT_STAGE hid-release-report")
        .count();
    if reports < CHARACTERS * 2 {
        return Err(refusal());
    }
    Ok(
        json!({"first_play": quiescent[0], "last_play": final_play, "hid_followup_reports": reports,
        "result_retained_after_lull": true}),
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    fn records() -> Vec<Value> {
        let mut records = (0..=CHARACTERS * 2).map(|count| json!({
            "status":"quiescent-awaiting-input", "host_id":"host", "boot_id":"boot", "source_document_id":"source",
            "checked_form_id":"checked", "expanded_form_id":"expanded", "body_id":"body", "wake_id":"wake", "plan_id":"plan", "active_play_id":"play",
            "input_count":count, "result":if count == 0 { Value::Null } else { expected_text(count.div_ceil(2)).into() },
            "result_omitted_bytes":count.div_ceil(2).saturating_sub(128)
        })).collect::<Vec<_>>();
        for status in ["stopped", "lulled"] {
            let mut record = records.last().unwrap().clone();
            record["status"] = status.into();
            records.push(record);
        }
        records
    }
    fn serial(records: &[Value]) -> String {
        "CONDUIT_BOOT_STAGE hid-release-report\n".repeat(CHARACTERS * 2)
            + &records
                .iter()
                .map(|record| format!("CONDUIT_PRODUCT_JOURNEY {record}\n"))
                .collect::<String>()
    }
    #[test]
    fn repeated_keyboard_proof_rejects_restarts_input_loss_and_hidden_output_truncation() {
        let valid = records();
        assert!(validate(&serial(&valid)).is_ok());
        for field in [
            "body_id",
            "plan_id",
            "active_play_id",
            "checked_form_id",
            "input_count",
            "result",
            "result_omitted_bytes",
        ] {
            let mut changed = valid.clone();
            changed[300][field] = "substituted".into();
            assert!(validate(&serial(&changed)).is_err(), "{field}");
        }
        let mut lost = valid.clone();
        lost.remove(65);
        assert!(validate(&serial(&lost)).is_err());
        let mut duplicate = valid;
        duplicate.insert(65, duplicate[65].clone());
        assert!(validate(&serial(&duplicate)).is_err());
    }
}
