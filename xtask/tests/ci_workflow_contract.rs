use std::fs;
use std::path::PathBuf;

#[test]
fn required_check_waits_for_every_selectable_proof_aggregate() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("..");
    let workflow =
        fs::read_to_string(root.join(".github/workflows/check.yml")).expect("read check workflow");
    let required_gate = workflow
        .split("\n  check:\n")
        .nth(1)
        .and_then(|tail| tail.split("\n  browser-tools:\n").next())
        .expect("locate the stable required check job");

    let declared_join = required_gate
        .lines()
        .find(|line| line.trim_start().starts_with("needs:"))
        .expect("required check declares its proof join");
    for aggregate in [
        "classify",
        "workspace-check",
        "esp32-firmware",
        "browser-host",
        "conduitos-boot",
    ] {
        assert!(
            declared_join.contains(aggregate),
            "required check can finish without `{aggregate}`: {declared_join}"
        );
    }

    for result in ["BROWSER_HOST_RESULT", "CONDUITOS_RESULT"] {
        assert!(
            required_gate.contains(result),
            "required check joins the job but does not inspect `{result}`"
        );
    }
}
