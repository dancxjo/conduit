use super::super::{result, BrowserOracle, FormProofResult, InventoryForm};
use super::bounded_reason;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::Instant;

const EVIDENCE_MARKER: &str = "CONDUIT_FORM_EVIDENCE=";
const MAX_RUNTIME_IDENTITY_BYTES: usize = 256;

pub(super) struct BatchRequest<'a> {
    pub(super) form: &'a InventoryForm,
    pub(super) path: String,
    pub(super) identities: Option<(String, String)>,
    pub(super) oracle: &'a BrowserOracle,
}

#[derive(Debug, Deserialize)]
struct BrowserEvidence {
    slug: String,
    status: String,
    plan_id: Option<String>,
    play_id: Option<String>,
    reason: Option<String>,
}

pub(super) fn local_playwright(root: &Path) -> PathBuf {
    let name = if cfg!(windows) {
        "playwright.cmd"
    } else {
        "playwright"
    };
    root.join("proof/browser/node_modules")
        .join(".bin")
        .join(name)
}

pub(super) fn execute(
    root: &Path,
    playwright: &Path,
    requests: Vec<BatchRequest<'_>>,
) -> Vec<FormProofResult> {
    if requests.is_empty() {
        return Vec::new();
    }
    let specs: BTreeSet<_> = requests
        .iter()
        .map(|request| request.oracle.spec.as_str())
        .collect();
    let cases: Vec<_> = requests
        .iter()
        .map(|request| request.oracle.case.as_str())
        .collect();
    let mut command = Command::new(playwright);
    command
        .current_dir(root)
        .args(["test", "--config", "proof/browser/playwright.config.mjs"])
        .args(specs)
        .args(["--project", "chromium", "--workers", "1", "--retries", "0"])
        .env(
            "CONDUIT_FORM_CASES_JSON",
            serde_json::to_string(&cases).expect("bounded case identities serialize"),
        );
    let started = Instant::now();
    let output = command.output();
    let duration = started.elapsed().as_millis();
    match output {
        Ok(output) => materialize(&requests, &output, duration),
        Err(error) => requests
            .into_iter()
            .map(|request| {
                proof(
                    request,
                    duration,
                    "unavailable",
                    &format!("cannot start admitted Playwright binary: {error}"),
                )
            })
            .collect(),
    }
}

fn materialize(
    requests: &[BatchRequest<'_>],
    output: &Output,
    duration: u128,
) -> Vec<FormProofResult> {
    let expected: BTreeSet<_> = requests
        .iter()
        .map(|request| request.form.slug.as_str())
        .collect();
    let records = evidence_records(output, &expected);
    let diagnostic = bounded_reason(&format!(
        "{}{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    ));
    match records {
        Err(reason) => requests
            .iter()
            .map(|request| proof_ref(request, duration, "failed", &reason))
            .collect(),
        Ok(records) => {
            let all_passed = records.values().all(|record| record.status == "passed");
            requests
                .iter()
                .map(|request| match records.get(&request.form.slug) {
                    None => proof_ref(
                        request,
                        duration,
                        "failed",
                        "browser-safe batch omitted this Form's exact evidence",
                    ),
                    Some(_) if !output.status.success() && all_passed => proof_ref(
                        request,
                        duration,
                        "failed",
                        &format!("Playwright batch failed after passing evidence: {diagnostic}"),
                    ),
                    Some(record) => from_record(request, record, duration),
                })
                .collect()
        }
    }
}

fn from_record(
    request: &BatchRequest<'_>,
    record: &BrowserEvidence,
    duration: u128,
) -> FormProofResult {
    match record.status.as_str() {
        "passed" => {
            let Some(plan_id) = record
                .plan_id
                .as_deref()
                .filter(|value| valid_identity(value))
            else {
                return proof_ref(
                    request,
                    duration,
                    "failed",
                    "browser-safe evidence has an invalid or empty Plan identity",
                );
            };
            let Some(play_id) = record
                .play_id
                .as_deref()
                .filter(|value| valid_identity(value))
            else {
                return proof_ref(
                    request,
                    duration,
                    "failed",
                    "browser-safe evidence has an invalid or empty Play identity",
                );
            };
            let mut proof = proof_ref(
                request,
                duration,
                "passed",
                "declared oracle passed in one admitted Playwright batch with fresh test state",
            );
            proof.plan_id = Some(plan_id.to_owned());
            proof.play_id = Some(play_id.to_owned());
            proof
        }
        status @ ("refused" | "failed") => {
            let reason = record.reason.as_deref().unwrap_or("");
            if reason.is_empty() {
                proof_ref(
                    request,
                    duration,
                    "failed",
                    "browser-safe failure evidence omitted its bounded reason",
                )
            } else {
                proof_ref(request, duration, status, &bounded_reason(reason))
            }
        }
        _ => proof_ref(
            request,
            duration,
            "failed",
            "browser-safe evidence contains an unknown status",
        ),
    }
}

fn evidence_records(
    output: &Output,
    expected: &BTreeSet<&str>,
) -> Result<BTreeMap<String, BrowserEvidence>, String> {
    let mut records = BTreeMap::new();
    for encoded in String::from_utf8_lossy(&output.stdout)
        .lines()
        .filter_map(|line| line.split_once(EVIDENCE_MARKER).map(|(_, value)| value))
    {
        let record: BrowserEvidence = serde_json::from_str(encoded)
            .map_err(|error| format!("browser-safe batch emitted malformed evidence: {error}"))?;
        if !expected.contains(record.slug.as_str()) {
            return Err(format!(
                "browser-safe batch emitted evidence for undeclared Form '{}'",
                record.slug
            ));
        }
        let slug = record.slug.clone();
        if records.insert(slug.clone(), record).is_some() {
            return Err(format!(
                "browser-safe batch emitted duplicate evidence for Form '{slug}'"
            ));
        }
    }
    Ok(records)
}

fn valid_identity(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= MAX_RUNTIME_IDENTITY_BYTES
        && !value.chars().any(char::is_control)
}

fn proof(request: BatchRequest<'_>, duration: u128, status: &str, reason: &str) -> FormProofResult {
    proof_ref(&request, duration, status, reason)
}

fn proof_ref(
    request: &BatchRequest<'_>,
    duration: u128,
    status: &str,
    reason: &str,
) -> FormProofResult {
    let mut proof = result(
        request.form,
        &request.path,
        duration,
        status,
        reason,
        request.identities.clone(),
        "browser-safe",
    );
    proof.environment_profile = "playwright/chromium-1.62.0-worker1-retry0";
    proof.evidence_artifacts.push(request.oracle.spec.clone());
    proof
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn local_tooling_is_exact_and_never_uses_npx() {
        let path = local_playwright(Path::new("/fixture"));
        assert!(path.ends_with(Path::new("proof/browser/node_modules/.bin/playwright")));
    }

    #[test]
    fn parses_multiple_records_and_preserves_individual_failure() {
        let output = output(
            false,
            concat!(
                "CONDUIT_FORM_EVIDENCE={\"slug\":\"one\",\"status\":\"passed\",\"plan_id\":\"plan/1\",\"play_id\":\"play/1\"}\n",
                "CONDUIT_FORM_EVIDENCE={\"slug\":\"two\",\"status\":\"refused\",\"reason\":\"source interaction refused\"}\n"
            ),
        );
        let expected = BTreeSet::from(["one", "two"]);
        let records = evidence_records(&output, &expected).unwrap();
        assert_eq!(records.len(), 2);
        assert_eq!(records["one"].status, "passed");
        assert_eq!(records["two"].status, "refused");

        let forms = [form("one"), form("two")];
        let oracles = [oracle("one"), oracle("two")];
        let requests: Vec<_> = forms
            .iter()
            .zip(&oracles)
            .map(|(form, oracle)| BatchRequest {
                form,
                path: format!("forms/{}/main.conduit", form.slug),
                identities: None,
                oracle,
            })
            .collect();
        let results = materialize(&requests, &output, 7);
        assert_eq!(results[0].status, "passed");
        assert_eq!(results[1].status, "refused");
    }

    #[test]
    fn malformed_duplicate_and_unknown_evidence_fail_closed() {
        let expected = BTreeSet::from(["one"]);
        assert!(
            evidence_records(&output(true, "CONDUIT_FORM_EVIDENCE={bad}\n"), &expected).is_err()
        );
        assert!(evidence_records(
            &output(
                true,
                concat!(
                    "CONDUIT_FORM_EVIDENCE={\"slug\":\"one\",\"status\":\"failed\"}\n",
                    "CONDUIT_FORM_EVIDENCE={\"slug\":\"one\",\"status\":\"failed\"}\n"
                )
            ),
            &expected,
        )
        .is_err());
        assert!(evidence_records(
            &output(
                true,
                "CONDUIT_FORM_EVIDENCE={\"slug\":\"other\",\"status\":\"passed\"}\n"
            ),
            &expected,
        )
        .is_err());
    }

    #[test]
    fn plan_and_play_identities_are_nonempty_bounded_and_printable() {
        assert!(valid_identity("plan/1"));
        assert!(!valid_identity(""));
        assert!(!valid_identity("bad\nidentity"));
        assert!(!valid_identity(&"x".repeat(MAX_RUNTIME_IDENTITY_BYTES + 1)));
    }

    fn output(success: bool, stdout: &str) -> Output {
        Output {
            status: status(success),
            stdout: stdout.as_bytes().to_vec(),
            stderr: Vec::new(),
        }
    }

    fn form(slug: &str) -> InventoryForm {
        InventoryForm {
            slug: slug.to_owned(),
            title: slug.to_owned(),
            entry: slug.to_owned(),
            reusable_entries: Vec::new(),
            initial_body_order: None,
            deterministic: None,
            deterministic_not_applicable: None,
            browser_safe: None,
            browser_safe_not_applicable: None,
        }
    }

    fn oracle(case: &str) -> BrowserOracle {
        BrowserOracle {
            spec: "proof/browser/reviewed-form-conformance.spec.mjs".to_owned(),
            case: case.to_owned(),
        }
    }

    #[cfg(unix)]
    fn status(success: bool) -> std::process::ExitStatus {
        use std::os::unix::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(if success { 0 } else { 1 << 8 })
    }

    #[cfg(windows)]
    fn status(success: bool) -> std::process::ExitStatus {
        use std::os::windows::process::ExitStatusExt;
        std::process::ExitStatus::from_raw(if success { 0 } else { 1 })
    }
}
