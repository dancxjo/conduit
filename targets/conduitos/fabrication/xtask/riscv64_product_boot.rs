//! Pinned two-boot acceptance for the RISC-V64 product Host.

use super::{
    profile::Paths,
    report::{git_head, sha256_file},
    riscv64_a1, ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use std::fs;

const PRODUCT_PREFIX: &str = "CONDUIT_RISCV64_PRODUCT ";
const OBSERVATORY_PREFIX: &str = "CONDUIT_OBSERVATORY_SNAPSHOT ";

pub(super) fn boot_twice(
    image: &std::path::Path,
    profile: &str,
    build: &str,
    binding: &str,
    _opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let paths = Paths::new(ConduitosArch::Riscv64)?;
    let (first, snapshot) = boot_once(&paths, image, profile, build, binding)?;
    let (second, _) = boot_once(&paths, image, profile, build, binding)?;
    if first["host_id"] == second["host_id"] || first["boot_id"] == second["boot_id"] {
        return Err(refusal(
            "stale-riscv64-product-identity",
            "independent product boots reused HostId or BootId",
        ));
    }
    let snapshot_path = paths.target.join("riscv64-product-observatory.json");
    fs::write(
        &snapshot_path,
        serde_json::to_vec_pretty(&snapshot).map_err(invalid)?,
    )
    .map_err(|e| refusal("riscv64-product-proof-unavailable", e.to_string()))?;
    prove_patchbay(&paths, &snapshot_path, &first)?;
    let proof = serde_json::json!({
        "schema": "conduit.conduitos/riscv64-product-proof@1",
        "base_commit": git_head(&paths.root)?, "image_sha256": sha256_file(image)?,
        "first": first, "second": second, "fresh_host_id": true, "fresh_boot_id": true,
        "native_patchbay_consumed": true, "stopped_by_harness": true
    });
    fs::write(
        paths.target.join("riscv64-product-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(invalid)?,
    )
    .map_err(|e| refusal("riscv64-product-proof-unavailable", e.to_string()))?;
    Ok(())
}

fn boot_once(
    paths: &Paths,
    image: &std::path::Path,
    profile: &str,
    build: &str,
    binding: &str,
) -> Result<(serde_json::Value, serde_json::Value), ConduitosError> {
    let text = riscv64_a1::boot_until_image(paths, image, OBSERVATORY_PREFIX)?;
    let product = parse_one(&text, PRODUCT_PREFIX, "product")?;
    if product["schema"] != "conduit.conduitos/riscv64-product@1"
        || product["status"] != "ready"
        || product["profile_id"] != profile
        || product["build_id"] != build
        || product["image_id"] != binding
        || product["host_id"].as_str().is_none_or(str::is_empty)
        || product["boot_id"].as_str().is_none_or(str::is_empty)
        || product["presenter_implementation_id"] != "presenter/riscv64-linear-sbi-console@1"
        || product["semantic_result"] != "HELLO, CONDUITOS"
        || product["timer_irq_wakes"] != 1
        || product["long_lived"] != true
    {
        return Err(refusal(
            "profile-built-fabrication-mismatch",
            product.to_string(),
        ));
    }
    let snapshot = parse_one(&text, OBSERVATORY_PREFIX, "Observatory")?;
    conduit_observatory::validate_snapshot(
        &serde_json::from_value(snapshot.clone()).map_err(invalid)?,
    )
    .map_err(|e| refusal("invalid-riscv64-product-observatory", e.to_string()))?;
    if snapshot["hosts"][0]["advertisement"]["host_id"] != product["host_id"]
        || snapshot["hosts"][0]["advertisement"]["boot_id"] != product["boot_id"]
        || snapshot["plans"][0]["plan_id"] != product["ordinary_plan_id"]
        || snapshot["plays"][0]["active_play_id"] != product["ordinary_play_id"]
    {
        return Err(refusal(
            "broken-riscv64-product-observatory-correlation",
            "snapshot does not correlate product identities",
        ));
    }
    Ok((product, snapshot))
}

fn parse_one(text: &str, prefix: &str, name: &str) -> Result<serde_json::Value, ConduitosError> {
    let values = text
        .split(prefix)
        .skip(1)
        .filter_map(|s| s.lines().next())
        .collect::<Vec<_>>();
    if values.len() != 1 {
        return Err(refusal(
            "absent-or-duplicate-riscv64-product-sign",
            format!("expected one {name}, found {}", values.len()),
        ));
    }
    serde_json::from_str(values[0].trim_end_matches('\r')).map_err(invalid)
}

fn prove_patchbay(
    paths: &Paths,
    snapshot: &std::path::Path,
    product: &serde_json::Value,
) -> Result<(), ConduitosError> {
    let output = super::profile::command(
        "cargo",
        &[
            "run",
            "--quiet",
            "-p",
            "patchbay-native",
            "--",
            "--linear-observatory-snapshot",
            snapshot.to_str().unwrap_or_default(),
        ],
        &paths.root,
        "patchbay-rejected-riscv64-product",
    )?;
    let linear = String::from_utf8(output.stdout)
        .map_err(|e| refusal("patchbay-rejected-riscv64-product", e.to_string()))?;
    for required in [
        product["host_id"].as_str().unwrap_or_default(),
        product["boot_id"].as_str().unwrap_or_default(),
        product["ordinary_plan_id"].as_str().unwrap_or_default(),
        product["ordinary_play_id"].as_str().unwrap_or_default(),
        "BASES 7",
        "lifecycle=Completed",
        "firmware=sbi",
        "proof=FreestandingEmulator",
    ] {
        if required.is_empty() || !linear.contains(required) {
            return Err(refusal(
                "patchbay-riscv64-product-projection-incomplete",
                format!("native Patchbay omitted {required}"),
            ));
        }
    }
    Ok(())
}

fn invalid(e: serde_json::Error) -> ConduitosError {
    refusal("riscv64-product-proof-invalid", e.to_string())
}
fn refusal(code: &'static str, detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal(code, detail)
}
