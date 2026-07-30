use std::{
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

fn workload_report(workspace_root: &Path, workloads: &[Value]) -> Vec<Value> {
    let _ = run_cmd(
        &["cargo", "test", "--release", "--workspace", "--no-run"],
        workspace_root,
        false,
    );

    let mut report = Vec::new();
    for wl in workloads {
        let id = wl.get("id").and_then(Value::as_str).unwrap_or("");
        let timing_gate = wl.get("timing");
        let cmd_arr = wl.get("command").and_then(Value::as_array);

        let elapsed_ns = if let Some(arr) = cmd_arr {
            let str_args: Vec<&str> = arr.iter().filter_map(Value::as_str).collect();
            let start = Instant::now();
            let _ = run_cmd(&str_args, workspace_root, false);
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
    report
}

pub fn run(workspace_root: &Path, update: bool) -> Result<(), Box<dyn std::error::Error>> {
    let baseline_path = workspace_root.join("benchmarks/baseline-v1.json");
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

    let report = json!({
        "schema": "conduit.performance-report/v1",
        "metadata": host_metadata(workspace_root, &baseline),
        "artifacts": measured,
        "workloads": workload_report(workspace_root, &workloads),
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
