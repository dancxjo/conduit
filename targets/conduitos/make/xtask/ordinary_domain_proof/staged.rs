//! Strict separate staged-window matrix; no assembly or production IPC claim.
use super::super::ConduitosError;
use serde_json::{json, Value};
const HEADER: &str = "CONDUIT_SERVICE_STAGED_HEADER ";
const SAMPLE: &str = "CONDUIT_SERVICE_STAGED_SAMPLE ";
const REFUSAL: &str = "CONDUIT_SERVICE_STAGED_REFUSAL ";
const LENGTHS: [u64; 5] = [0, 256, 4096, 16384, 65536];
fn refused() -> ConduitosError {
    ConduitosError::refusal("service-staged-invalid","requires exact30-row full-span matrix and8correlated refusals; no independent assembly attribution or product protocol claim")
}
fn complete_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split_inclusive('\n').filter_map(|line| {
        line.strip_suffix('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
    })
}
fn records(text: &str, prefix: &str) -> Result<Vec<Value>, ConduitosError> {
    complete_lines(text)
        .filter_map(|line| line.strip_prefix(prefix))
        .map(|line| serde_json::from_str(line).map_err(|_| refused()))
        .collect()
}
pub(super) fn validate(text: &str) -> Result<Value, ConduitosError> {
    let sequence = complete_lines(text)
        .filter_map(|line| {
            [HEADER, SAMPLE, REFUSAL]
                .into_iter()
                .find(|prefix| line.starts_with(*prefix))
        })
        .collect::<Vec<_>>();
    if sequence.len() != 39
        || sequence.iter().enumerate().any(|(index, prefix)| {
            *prefix
                != if index == 0 {
                    HEADER
                } else if index <= 30 {
                    SAMPLE
                } else {
                    REFUSAL
                }
        })
    {
        return Err(refused());
    }
    let headers = records(text, HEADER)?;
    if headers.len() != 1 {
        return Err(refused());
    }
    let header = &headers[0];
    for (key, expected) in [
        ("schema", json!("conduit.conduitos/service-staged-ipc@1")),
        ("fixture", json!(true)),
        ("privilege", json!("ring3")),
        ("units", json!("raw-tsc")),
        ("calibrated", json!(false)),
        ("counter_overhead_included", json!(true)),
        ("component_attribution", json!(false)),
        ("records", json!(30)),
        ("refusals", json!(8)),
        ("lengths", json!(LENGTHS)),
        ("root_workspace_bytes", json!(131072)),
        ("domain_reserved_bytes", json!(19349504)),
        ("entry_work_grant", json!(1)),
        ("tail_check_outside_interval", json!(true)),
        ("preconditioned", json!(true)),
        ("total_transaction", json!(false)),
        ("guest_independent_identity_authority", json!(false)),
        ("production_protocol", json!(false)),
    ] {
        if header[key] != expected {
            return Err(refused());
        }
    }
    let rows = records(text, SAMPLE)?;
    if rows.len() != 30 {
        return Err(refused());
    }
    let mut previous = 0;
    for (index, row) in rows.iter().enumerate() {
        let case = index / 6;
        let mode = (index % 6) / 3;
        let trial = index % 3;
        let length = LENGTHS[case];
        if row["case"] != case
            || row["mode"] != mode
            || row["trial"] != trial
            || row["length"] != length
            || row["validated_bytes"] != length
            || row["request"] != index + 1
            || row["tail_unchanged"] != true
        {
            return Err(refused());
        }
        let instance = row["instance"]
            .as_u64()
            .filter(|value| *value > 0)
            .ok_or_else(refused)?;
        if mode == 1 && trial > 0 {
            if instance != previous {
                return Err(refused());
            }
        } else if instance <= previous {
            return Err(refused());
        }
        for key in [
            "stage_admission_copy",
            "entry_and_guest",
            "return_copy_validation",
        ] {
            if row[key].as_u64().is_none() {
                return Err(refused());
            }
        }
        previous = instance;
    }
    let refusals = records(text, REFUSAL)?;
    if refusals.len() != 8 {
        return Err(refused());
    }
    let mut current = 0;
    for (case, row) in refusals.iter().enumerate() {
        let instance = row["instance"]
            .as_u64()
            .filter(|value| *value > previous)
            .ok_or_else(refused)?;
        if case == 0 {
            current = instance;
        } else if instance != current {
            return Err(refused());
        }
        let reason = if case < 6 {
            case + 1
        } else if case == 6 {
            4
        } else {
            7
        };
        if row["case"] != case
            || row["request"] != 31
            || row["descriptor_request"] != if case == 5 { 30 } else { 31 }
            || row["descriptor_instance"] != if case == 4 { previous } else { current }
            || row["reason"] != reason
            || row["guest_entered"] != (case == 6)
            || row["successful_output"] != false
        {
            return Err(refused());
        }
    }
    Ok(json!({"header":header,"samples":rows,"refusals":refusals,
        "scope":"actual stopped transfer-window stage admission/copy; whole protected entry plus full-span guest XOR; return copy plus full requested-span validation and exact Root current-instance/request correlation; untouched-tail check outside timing; full64KiBwindow and Rootoutput prefilled outside intervals warming memory; no cold-cache or totaltransaction claim; root_workspace_bytes counts only the two prepared payloadbuffers, excluding fixed row/descriptor metadata and allocator overhead; guest only echoes supplied identity",
        "not_established":["calibrated latency","independent trap/CR3/register/interrupt costs","production staged protocol","parent production confinement","whole-process heap accounting","normal responsive-journey speedup"]}))
}
#[cfg(test)]
mod tests {
    use super::*;
    fn transcript() -> String {
        let header = json!({"schema":"conduit.conduitos/service-staged-ipc@1","fixture":true,"privilege":"ring3","units":"raw-tsc","calibrated":false,"counter_overhead_included":true,"component_attribution":false,"records":30,"refusals":8,"lengths":LENGTHS,"root_workspace_bytes":131072,"domain_reserved_bytes":19349504,"entry_work_grant":1,"tail_check_outside_interval":true,"preconditioned":true,"total_transaction":false,"guest_independent_identity_authority":false,"production_protocol":false});
        let mut text = format!("{HEADER}{header}\n");
        let mut instance = 0;
        for index in 0..30 {
            let case = index / 6;
            let mode = (index % 6) / 3;
            let trial = index % 3;
            if mode == 0 || trial == 0 {
                instance += 1;
            }
            text.push_str(&format!("{SAMPLE}{}\n",json!({"case":case,"mode":mode,"trial":trial,"length":LENGTHS[case],"validated_bytes":LENGTHS[case],"instance":instance,"request":index+1,"stage_admission_copy":10,"entry_and_guest":20,"return_copy_validation":30,"tail_unchanged":true})));
        }
        for case in 0..8 {
            text.push_str(&format!("{REFUSAL}{}\n",json!({"case":case,"instance":instance+1,"request":31,"descriptor_request":if case==5{30}else{31},"descriptor_instance":if case==4{instance}else{instance+1},"reason":if case<6{case+1}else if case==6{4}else{7},"guest_entered":case==6,"successful_output":false})));
        }
        text
    }
    #[test]
    fn exact_full_matrix_and_refusal_correspondence() {
        let text = transcript();
        let receipt = validate(&text).unwrap();
        assert_eq!(receipt["samples"].as_array().unwrap().len(), 30);
        for (from, to) in [
            ("\"request\":1,", "\"request\":2,"),
            ("\"validated_bytes\":0", "\"validated_bytes\":1"),
            ("\"length\":65536", "\"length\":65537"),
            ("\"successful_output\":false", "\"successful_output\":true"),
            ("\"guest_entered\":true", "\"guest_entered\":false"),
            ("\"tail_unchanged\":true", "\"tail_unchanged\":false"),
            ("\"descriptor_request\":30", "\"descriptor_request\":31"),
        ] {
            let changed = text.replace(from, to);
            assert_ne!(changed, text, "{from}");
            assert!(validate(&changed).is_err(), "{from}");
        }
    }
    #[test]
    fn broadened_profile_duplicate_missing_and_partial_records_refuse() {
        let text = transcript();
        for (from, to) in [
            ("service-staged-ipc@1", "service-staged-ipc@2"),
            (
                "\"root_workspace_bytes\":131072",
                "\"root_workspace_bytes\":131073",
            ),
            ("\"entry_work_grant\":1", "\"entry_work_grant\":2"),
            (
                "\"guest_independent_identity_authority\":false",
                "\"guest_independent_identity_authority\":true",
            ),
        ] {
            let changed = text.replace(from, to);
            assert_ne!(changed, text);
            assert!(validate(&changed).is_err());
        }
        assert!(validate("").is_err());
        assert!(validate(&format!("{text}{text}")).is_err());
        assert!(validate(text.strip_suffix('\n').unwrap()).is_err());
        let row = text.lines().find(|line| line.starts_with(SAMPLE)).unwrap();
        assert!(validate(&format!("{text}{row}\n")).is_err());
        let partial = format!("{text}{SAMPLE}{{");
        assert!(validate(&partial).is_ok());
        assert!(validate(&format!("{partial}\n")).is_err());
        assert!(validate(&text.replace('\n', "\r\n")).is_ok());
        // Within-prefix order/counts remain intact; cross-kind reordering must refuse.
        let mut moved = text.lines().collect::<Vec<_>>();
        let first_refusal = moved.remove(31);
        moved.insert(1, first_refusal);
        let moved = moved.join("\n") + "\n";
        assert_ne!(moved, text);
        assert!(validate(&moved).is_err());
        let mut late_header = text.lines().collect::<Vec<_>>();
        let header = late_header.remove(0);
        late_header.insert(1, header);
        let late_header = late_header.join("\n") + "\n";
        assert_ne!(late_header, text);
        assert!(validate(&late_header).is_err());
        assert!(
            validate(&text.replacen(HEADER, &format!("unrelated boot line\n{HEADER}"), 1)).is_ok()
        );
    }
}
