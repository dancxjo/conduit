//! Strict fixed-matrix reader; raw observations are retained without speed claims.
use super::super::ConduitosError;
use serde_json::{Value, json};
const HEADER: &str = "CONDUIT_SERVICE_BASELINE ";
const SAMPLE: &str = "CONDUIT_SERVICE_SAMPLE ";
fn refusal() -> ConduitosError {
    ConduitosError::refusal(
        "service-baseline-invalid",
        "requires one exact raw-TSC fixture matrix, not calibrated or component timing",
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
    Ok(json!({"header":header,"samples":rows,
        "scope":"whole ServiceDomain entry including budget/context setup, guest work, interrupts and automatic fault quarantine; request includes frame clearing; response includes length/status check and copied-byte observation; 256B is only this fixture's maximum",
        "remaining":"larger transfer-window/staged IPC sweep, assembly component attribution, demonstrated optimization, before/after isolation equivalence and responsive journey comparison"}))
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
}
