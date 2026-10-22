//! Strict fixed-matrix reader; raw observations are retained without speed claims.
use super::super::ConduitosError;
use serde_json::{Value, json};
const HEADER: &str = "CONDUIT_SERVICE_BASELINE ";
const SAMPLE: &str = "CONDUIT_SERVICE_SAMPLE ";
const COMPONENT: &str = "CONDUIT_SERVICE_SETUP_COMPONENT ";
fn refusal() -> ConduitosError {
    ConduitosError::refusal(
        "service-baseline-invalid",
        "requires one exact raw-TSC fixture matrix and any declared setup observations; no calibrated or assembly-component claim",
    )
}
fn complete_lines(text: &str) -> impl Iterator<Item = &str> {
    text.split_inclusive('\n').filter_map(|line| {
        line.strip_suffix('\n')
            .map(|line| line.strip_suffix('\r').unwrap_or(line))
    })
}
pub(super) fn validate(text: &str) -> Result<Value, ConduitosError> {
    let headers = complete_lines(text)
        .filter_map(|line| line.strip_prefix(HEADER))
        .collect::<Vec<_>>();
    if headers.len() != 1 {
        return Err(refusal());
    }
    let header: Value = serde_json::from_str(headers[0]).map_err(|_| refusal())?;
    if header["schema"] != "conduit.conduitos/service-baseline@1"
        || header["units"] != "raw-tsc"
        || header["calibrated"] != false
        || header["component_attribution"] != false
        || header["fixture_capacity"] != 256
        || header["records"] != 20
        || header["invariant_tsc"].as_bool().is_none()
    {
        return Err(refusal());
    }
    for key in ["cpuid1_eax", "cpuid1_ecx", "cpuid1_edx"] {
        if header[key].as_u64().is_none_or(|n| n > u32::MAX.into()) {
            return Err(refusal());
        }
    }
    if header["counter_pair_ticks"]
        .as_array()
        .is_none_or(|a| a.len() != 3 || a.iter().any(|v| v.as_u64().is_none()))
    {
        return Err(refusal());
    }
    let rows = complete_lines(text)
        .filter_map(|line| line.strip_prefix(SAMPLE))
        .map(|line| serde_json::from_str::<Value>(line).map_err(|_| refusal()))
        .collect::<Result<Vec<_>, _>>()?;
    if rows.len() != 20 {
        return Err(refusal());
    }
    let mut previous_instance = 0;
    for (index, row) in rows.iter().enumerate() {
        let (case, mode, trial) = if index < 18 {
            (index / 6, (index % 6) / 3, index % 3)
        } else {
            (index - 15, 0, 0)
        };
        if row["case"] != case || row["mode"] != mode || row["trial"] != trial {
            return Err(refusal());
        }
        for key in [
            "setup",
            "request",
            "entry",
            "response",
            "teardown",
            "interrupts",
            "preemptions",
        ] {
            if row[key].as_u64().is_none() {
                return Err(refusal());
            }
        }
        let instance = row["instance"]
            .as_u64()
            .filter(|n| *n > 0)
            .ok_or_else(refusal)?;
        if mode == 1 && trial > 0 {
            if instance != previous_instance || row["setup"] != 0 {
                return Err(refusal());
            }
        } else if instance <= previous_instance {
            return Err(refusal());
        }
        if mode == 1 && trial < 2 && row["teardown"] != 0 {
            return Err(refusal());
        }
        if row["preemptions"] != u64::from(case == 3) || (case >= 3 && row["response"] != 0) {
            return Err(refusal());
        }
        previous_instance = instance;
    }
    let components = validate_components(text, &header, &rows)?;
    Ok(
        json!({"header":header,"samples":rows,"setup_components":components,
        "setup_component_scope":"@1 measures slot clearing including counter reads; @2 records an omitted clear with zero sentinel and a full zero-state scan included in outer setup; exact instance/setup correspondence; no independent trap/CR3/register/interrupt attribution",
        "scope":"whole ServiceDomain entry including budget/context setup, guest work, interrupts and automatic fault quarantine; request includes frame clearing; response includes length/status check and copied-byte observation; 256B is only this fixture's maximum",
        "remaining":"larger transfer-window/staged IPC sweep, assembly component attribution, demonstrated optimization, before/after isolation equivalence and responsive journey comparison"}),
    )
}
fn validate_components(
    text: &str,
    header: &Value,
    rows: &[Value],
) -> Result<Vec<Value>, ConduitosError> {
    let declared = match header.get("setup_component_records") {
        None => 0,
        Some(value) if value.as_u64() == Some(14) => 14,
        Some(_) => return Err(refusal()),
    };
    let components = complete_lines(text)
        .filter_map(|line| line.strip_prefix(COMPONENT))
        .map(|line| serde_json::from_str::<Value>(line).map_err(|_| refusal()))
        .collect::<Result<Vec<_>, _>>()?;
    if components.len() != declared {
        return Err(refusal());
    }
    if declared == 0 {
        return Ok(components);
    }
    let schema = &components[0]["schema"];
    if components
        .iter()
        .any(|component| &component["schema"] != schema)
    {
        return Err(refusal());
    }
    let setups = rows
        .iter()
        .filter(|row| row["mode"] == 0 || row["trial"] == 0);
    for (component, row) in components.iter().zip(setups) {
        let valid_profile = match component["schema"].as_str() {
            Some("conduit.conduitos/service-setup-component@1") => {
                component["counter_overhead_included"] == true
                    && component.get("clearing_performed").is_none()
                    && component.get("zero_state_scan_in_setup").is_none()
            }
            Some("conduit.conduitos/service-setup-component@2") => {
                component["clearing"].as_u64() == Some(0)
                    && component["clearing_performed"] == false
                    && component["counter_overhead_included"] == false
                    && component["zero_state_scan_in_setup"] == true
            }
            _ => false,
        };
        if !valid_profile
            || component["units"] != "raw-tsc"
            || component["component"] != "slot-zeroing"
            || component["instance"] != row["instance"]
            || component["setup"] != row["setup"]
            || component["clearing"]
                .as_u64()
                .is_none_or(|ticks| row["setup"].as_u64().is_none_or(|setup| ticks > setup))
        {
            return Err(refusal());
        }
    }
    Ok(components)
}
#[cfg(test)]
mod tests {
    use super::*;
    fn transcript() -> String {
        let mut text = format!(
            "{HEADER}{}\n",
            json!({"schema":"conduit.conduitos/service-baseline@1","units":"raw-tsc","calibrated":false,"component_attribution":false,"fixture_capacity":256,"records":20,"cpuid1_eax":0,"cpuid1_ecx":0,"cpuid1_edx":0,"invariant_tsc":false,"counter_pair_ticks":[1,2,3]})
        );
        let mut instance = 0;
        for index in 0..20 {
            let (case, mode, trial) = if index < 18 {
                (index / 6, (index % 6) / 3, index % 3)
            } else {
                (index - 15, 0, 0)
            };
            if mode == 0 || trial == 0 {
                instance += 1;
            }
            text.push_str(&format!("{SAMPLE}{}\n",json!({"case":case,"mode":mode,"trial":trial,"instance":instance,"setup":0,"request":1,"entry":2,"response":0,"teardown":0,"interrupts":u64::from(case==3),"preemptions":u64::from(case==3)})));
        }
        text
    }
    #[test]
    fn exact_matrix_and_evidence_boundaries() {
        let text = transcript();
        assert!(validate(&text).is_ok());
        assert!(validate("").is_err());
        assert!(validate(&format!("{text}{text}")).is_err());
        for (from, to) in [
            ("\"calibrated\":false", "\"calibrated\":true"),
            ("\"case\":4", "\"case\":3"),
            ("\"instance\":1,", "\"instance\":0,"),
            ("\"preemptions\":1", "\"preemptions\":0"),
            ("\"mode\":1", "\"mode\":0"),
        ] {
            assert_ne!(text.replace(from, to), text);
            assert!(validate(&text.replace(from, to)).is_err(), "{from}");
        }
        assert!(validate(&text.lines().take(20).collect::<Vec<_>>().join("\n")).is_err());
    }
    #[test]
    fn only_newline_terminated_records_are_admitted() {
        let text = transcript();
        assert!(validate(text.strip_suffix('\n').unwrap()).is_err());
        assert!(validate(&text.replace('\n', "\r\n")).is_ok());
        // Retain raw partial serial tails, but never parse them as records.
        let partial = format!("{text}{SAMPLE}{{\"case\":4");
        assert!(validate(&partial).is_ok());
        assert!(validate(&format!("{partial}\n")).is_err());
    }
    fn component_transcript() -> String {
        let mut text = transcript().replace(
            "\"records\":20",
            "\"records\":20,\"setup_component_records\":14",
        );
        let rows = validate(&transcript()).unwrap()["samples"]
            .as_array()
            .unwrap()
            .clone();
        for row in rows
            .iter()
            .filter(|row| row["mode"] == 0 || row["trial"] == 0)
        {
            text.push_str(&format!(
                "{COMPONENT}{}\n",
                json!({
                    "schema":"conduit.conduitos/service-setup-component@1",
                    "units":"raw-tsc","component":"slot-zeroing",
                    "instance":row["instance"],"setup":row["setup"],
                    "clearing":0,"counter_overhead_included":true
                })
            ));
        }
        text
    }
    #[test]
    fn elided_clear_requires_exact_versioned_profile() {
        let baseline = component_transcript();
        let mut text = String::new();
        for line in baseline.lines() {
            if let Some(raw) = line.strip_prefix(COMPONENT) {
                let mut value: Value = serde_json::from_str(raw).unwrap();
                value["schema"] = json!("conduit.conduitos/service-setup-component@2");
                value["setup"] = json!(10);
                value["clearing_performed"] = json!(false);
                value["counter_overhead_included"] = json!(false);
                value["zero_state_scan_in_setup"] = json!(true);
                text.push_str(&format!("{COMPONENT}{value}\n"));
            } else if let Some(raw) = line.strip_prefix(SAMPLE) {
                let mut value: Value = serde_json::from_str(raw).unwrap();
                if value["mode"] == 0 || value["trial"] == 0 {
                    value["setup"] = json!(10);
                }
                text.push_str(&format!("{SAMPLE}{value}\n"));
            } else {
                text.push_str(line);
                text.push('\n');
            }
        }
        assert!(validate(&baseline).is_ok());
        assert!(validate(&text).is_ok());
        let mut mixed = String::new();
        let mut changed = false;
        for line in text.lines() {
            if !changed && let Some(raw) = line.strip_prefix(COMPONENT) {
                let mut value: Value = serde_json::from_str(raw).unwrap();
                value["schema"] = json!("conduit.conduitos/service-setup-component@1");
                value["counter_overhead_included"] = json!(true);
                value.as_object_mut().unwrap().remove("clearing_performed");
                value
                    .as_object_mut()
                    .unwrap()
                    .remove("zero_state_scan_in_setup");
                mixed.push_str(&format!("{COMPONENT}{value}\n"));
                changed = true;
            } else {
                mixed.push_str(line);
                mixed.push('\n');
            }
        }
        assert!(changed);
        assert!(validate(&mixed).is_err());
        for (from, to) in [
            ("\"clearing\":0", "\"clearing\":1"),
            (
                "\"clearing_performed\":false",
                "\"clearing_performed\":true",
            ),
            (
                "\"counter_overhead_included\":false",
                "\"counter_overhead_included\":true",
            ),
            (
                "\"zero_state_scan_in_setup\":true",
                "\"zero_state_scan_in_setup\":false",
            ),
            ("service-setup-component@2", "service-setup-component@3"),
            ("\"clearing_performed\":false,", ""),
        ] {
            assert_ne!(text.replace(from, to), text, "mutation must apply: {from}");
            assert!(validate(&text.replace(from, to)).is_err(), "{from}");
        }
    }
    #[test]
    fn declared_components_require_exact_complete_instance_correspondence() {
        let text = component_transcript();
        assert_eq!(
            validate(&text).unwrap()["setup_components"]
                .as_array()
                .unwrap()
                .len(),
            14
        );
        for (from, to) in [
            ("\"clearing\":0", "\"clearing\":1"),
            (
                "\"counter_overhead_included\":true",
                "\"counter_overhead_included\":false",
            ),
            (
                "\"setup_component_records\":14",
                "\"setup_component_records\":13",
            ),
        ] {
            assert!(validate(&text.replace(from, to)).is_err(), "{from}");
        }
        let foreign = text
            .lines()
            .map(|line| {
                if line.starts_with(COMPONENT) {
                    line.replace("\"instance\":14", "\"instance\":13")
                } else {
                    line.to_owned()
                }
            })
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert_ne!(foreign, text);
        assert!(validate(&foreign).is_err());
        assert!(validate(text.strip_suffix('\n').unwrap()).is_err());
        let missing = text
            .lines()
            .filter(|line| !line.starts_with(COMPONENT))
            .collect::<Vec<_>>()
            .join("\n")
            + "\n";
        assert!(validate(&missing).is_err());
        let extra = text
            .lines()
            .find(|line| line.starts_with(COMPONENT))
            .unwrap();
        assert!(validate(&format!("{text}{extra}\n")).is_err());
        assert!(validate(&text.replace(",\"setup_component_records\":14", "")).is_err());
    }
}
