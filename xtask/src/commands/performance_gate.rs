use std::{
    collections::{BTreeMap, BTreeSet},
    fs,
    path::{Path, PathBuf},
    process::Command,
    time::Instant,
};

use serde_json::{Value, json};

fn run_cmd(cmd: &[&str], cwd: &Path, capture: bool) -> Result<String, String> {
    let output = Command::new(cmd[0])
        .args(&cmd[1..])
        .current_dir(cwd)
        .output()
        .map_err(|e| format!("failed to exec {}: {e}", cmd[0]))?;

    if !output.status.success() {
        let stderr = String::from_utf8_lossy(&output.stderr);
        return Err(format!("command {:?} failed: {stderr}", cmd));
    }
    if capture {
        Ok(String::from_utf8_lossy(&output.stdout).trim().to_string())
    } else {
        Ok(String::new())
    }
}

fn one_artifact(pattern_dir: &Path, prefix: &str, suffix: &str) -> Result<PathBuf, String> {
    let mut matches = Vec::new();
    if let Ok(entries) = fs::read_dir(pattern_dir) {
        for entry in entries.flatten() {
            let name = entry.file_name();
            let s = name.to_string_lossy();
            if s.starts_with(prefix) && s.ends_with(suffix) {
                if let Ok(meta) = entry.metadata() {
                    matches.push((entry.path(), meta.modified().ok()));
                }
            }
        }
    }
    matches.sort_by_key(|m| m.1);
    matches.last().map(|m| m.0.clone()).ok_or_else(|| {
        format!(
            "missing built artifact matching {prefix}*{suffix} in {}",
            pattern_dir.display()
        )
    })
}

fn build_artifacts(
    workspace_root: &Path,
) -> Result<std::collections::BTreeMap<String, PathBuf>, String> {
    run_cmd(
        &[
            "cargo",
            "build",
            "--release",
            "-p",
            "conduct",
            "-p",
            "conduit-core",
        ],
        workspace_root,
        false,
    )?;
    run_cmd(
        &[
            "cargo",
            "build",
            "--release",
            "-p",
            "conduit-core",
            "--no-default-features",
            "--target",
            "thumbv6m-none-eabi",
        ],
        workspace_root,
        false,
    )?;

    let conduct_bin = workspace_root.join("target/release/conduct");
    let core_rlib = one_artifact(
        &workspace_root.join("target/release/deps"),
        "libconduit_core-",
        ".rlib",
    )?;
    let thumb_rlib = one_artifact(
        &workspace_root.join("target/thumbv6m-none-eabi/release/deps"),
        "libconduit_core-",
        ".rlib",
    )?;

    let mut res = std::collections::BTreeMap::new();
    res.insert("conduct-release".to_string(), conduct_bin);
    res.insert("conduit-core-release".to_string(), core_rlib);
    res.insert("conduit-core-thumbv6m-release".to_string(), thumb_rlib);
    Ok(res)
}

fn host_metadata(workspace_root: &Path, baseline: &Value) -> Value {
    let commit = run_cmd(&["git", "rev-parse", "HEAD"], workspace_root, true).unwrap_or_default();
    let rustc_vv = run_cmd(&["rustc", "-Vv"], workspace_root, true).unwrap_or_default();
    let release = rustc_vv
        .lines()
        .find_map(|l| l.strip_prefix("release: "))
        .unwrap_or("unknown")
        .trim()
        .to_string();
    let host = rustc_vv
        .lines()
        .find_map(|l| l.strip_prefix("host: "))
        .unwrap_or("unknown")
        .trim()
        .to_string();

    let os = std::env::consts::OS.to_string();
    let machine = std::env::consts::ARCH.to_string();
    let cpu = match fs::read_to_string("/proc/cpuinfo") {
        Ok(text) => text
            .lines()
            .find_map(|l| {
                if l.to_lowercase().starts_with("model name") {
                    l.split(':').nth(1).map(|s| s.trim().to_string())
                } else {
                    None
                }
            })
            .unwrap_or_else(|| "unknown".to_string()),
        Err(_) => "unknown".to_string(),
    };

    let fixture_rev = baseline
        .get("fixture_revision")
        .and_then(Value::as_u64)
        .unwrap_or(1);

    json!({
        "commit": commit,
        "rustc": release,
        "host_target": host,
        "os": os,
        "machine": machine,
        "cpu": cpu,
        "fixture_revision": fixture_rev,
    })
}

fn workload_test_targets(workloads: &[Value]) -> BTreeMap<String, BTreeSet<String>> {
    let mut targets = BTreeMap::<String, BTreeSet<String>>::new();

    for workload in workloads {
        let Some(command) = workload.get("command").and_then(Value::as_array) else {
            continue;
        };
        let args: Vec<&str> = command.iter().filter_map(Value::as_str).collect();
        if args.get(..2) != Some(&["cargo", "test"]) {
            continue;
        }

        let package = args
            .windows(2)
            .find(|pair| pair[0] == "-p" || pair[0] == "--package")
            .map(|pair| pair[1]);
        let test = args
            .windows(2)
            .find(|pair| pair[0] == "--test")
            .map(|pair| pair[1]);

        if let (Some(package), Some(test)) = (package, test) {
            targets
                .entry(package.to_owned())
                .or_default()
                .insert(test.to_owned());
        }
    }

    targets
}

fn prebuild_workloads(workspace_root: &Path, workloads: &[Value]) -> Result<(), String> {
    for (package, tests) in workload_test_targets(workloads) {
        let mut args = vec![
            "test".to_owned(),
            "--release".to_owned(),
            "-p".to_owned(),
            package,
            "--no-run".to_owned(),
        ];
        for test in tests {
            args.push("--test".to_owned());
            args.push(test);
        }

        let output = Command::new("cargo")
            .args(&args)
            .current_dir(workspace_root)
            .output()
            .map_err(|error| format!("failed to prebuild performance workloads: {error}"))?;
        if !output.status.success() {
            return Err(format!(
                "performance workload prebuild failed: {}",
                String::from_utf8_lossy(&output.stderr)
            ));
        }
    }

    Ok(())
}

fn workload_report(workspace_root: &Path, workloads: &[Value]) -> Result<Vec<Value>, String> {
    prebuild_workloads(workspace_root, workloads)?;

    let mut report = Vec::new();
    for wl in workloads {
        let id = wl.get("id").and_then(Value::as_str).unwrap_or("");
        let timing_gate = wl.get("timing");
        let cmd_arr = wl.get("command").and_then(Value::as_array);

        let elapsed_ns = if let Some(arr) = cmd_arr {
            let str_args: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
            let start = Instant::now();
            run_cmd(&str_args, workspace_root, false)?;
            start.elapsed().as_nanos() as u64
        } else {
            0
        };

        report.push(json!({
            "id": id,
            "elapsed_ns": elapsed_ns,
            "gate": timing_gate
        }));
    }
    Ok(report)
}

pub fn run(workspace_root: &Path, update: bool) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = workspace_root.join("benchmarks/baseline.json");
    if !baseline_path.exists() {
        return Err(format!("Baseline missing: {}", baseline_path.display()).into());
    }

    let mut baseline: Value = serde_json::from_str(&fs::read_to_string(&baseline_path)?)?;
    let paths = build_artifacts(workspace_root)?;

    let mut measured = std::collections::BTreeMap::new();
    let mut failures = Vec::new();

    if let Some(art_obj) = baseline.get("artifacts").and_then(Value::as_object) {
        for (artifact_id, policy) in art_obj {
            let p = paths
                .get(artifact_id)
                .ok_or_else(|| format!("path missing for {artifact_id}"))?;
            let size = p.metadata()?.len();
            let baseline_bytes = policy
                .get("baseline_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0);
            let max_growth_percent = policy
                .get("maximum_growth_percent")
                .and_then(Value::as_f64)
                .unwrap_or(0.0);
            let max_growth_bytes = policy
                .get("maximum_growth_bytes")
                .and_then(Value::as_u64)
                .unwrap_or(0);

            let percent_allowance =
                ((baseline_bytes as f64) * max_growth_percent / 100.0).ceil() as u64;
            let allowance = percent_allowance.max(max_growth_bytes);
            let limit = baseline_bytes + allowance;

            let rel_path = p
                .strip_prefix(workspace_root)
                .unwrap_or(p)
                .to_string_lossy();

            measured.insert(
                artifact_id.clone(),
                json!({
                    "kind": policy.get("kind"),
                    "path": rel_path,
                    "bytes": size,
                    "baseline_bytes": baseline_bytes,
                    "limit_bytes": limit,
                }),
            );

            if size > limit {
                failures.push(format!(
                    "{artifact_id}: {size} bytes exceeds reviewed limit {limit}"
                ));
            }
        }
    }

    if update {
        if let Some(art_obj) = baseline.get_mut("artifacts").and_then(Value::as_object_mut) {
            for (artifact_id, res) in &measured {
                if let Some(policy) = art_obj.get_mut(artifact_id) {
                    if let Some(b) = res.get("bytes") {
                        policy["baseline_bytes"] = b.clone();
                    }
                }
            }
        }
        let commit =
            run_cmd(&["git", "rev-parse", "HEAD"], workspace_root, true).unwrap_or_default();
        let meta = host_metadata(workspace_root, &baseline);
        baseline["reviewed_commit"] = json!(commit);
        baseline["rustc"] = meta["rustc"].clone();

        fs::write(
            &baseline_path,
            format!("{}\n", serde_json::to_string_pretty(&baseline)?),
        )?;
        println!("Updated baseline at {}", baseline_path.display());
        return Ok(());
    }

    let workloads = baseline
        .get("workloads")
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let workload_report = workload_report(workspace_root, &workloads)?;
    let report = json!({
        "schema": "conduit.performance-report",
        "metadata": host_metadata(workspace_root, &baseline),
        "artifacts": measured,
        "workloads": workload_report,
    });

    println!("{}", serde_json::to_string_pretty(&report)?);

    if !failures.is_empty() {
        for f in failures {
            eprintln!("performance gate: {f}");
        }
        return Err("performance gate checks failed".into());
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::workload_test_targets;
    use serde_json::json;
    use std::collections::{BTreeMap, BTreeSet};

    #[test]
    fn groups_only_the_release_test_binaries_used_by_workloads() {
        let workloads = json!([
            {
                "command": ["cargo", "test", "--release", "-p", "alpha", "--test", "one"]
            },
            {
                "command": [
                    "cargo",
                    "test",
                    "--release",
                    "--package",
                    "alpha",
                    "--test",
                    "two"
                ]
            },
            {
                "command": ["cargo", "test", "--release", "-p", "beta", "--test", "three"]
            },
            {
                "command": ["cargo", "run", "-p", "ignored"]
            }
        ]);

        let expected = BTreeMap::from([
            (
                "alpha".to_owned(),
                BTreeSet::from(["one".to_owned(), "two".to_owned()]),
            ),
            ("beta".to_owned(), BTreeSet::from(["three".to_owned()])),
        ]);

        assert_eq!(
            workload_test_targets(workloads.as_array().unwrap()),
            expected
        );
    }
}
