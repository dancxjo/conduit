//! Normal BIOS product boots remain separate from instrumented boundary entries.
use super::super::{
    ConduitosError, ia32_product_boot,
    profile::Paths,
    report::{git_head, sha256_file},
};
use crate::{cli::GlobalOpts, commands::host::host_target::TargetBuildManifest};
use serde_json::{Value, json};
use std::{fs, path::Path};

pub(super) fn capture(
    paths: &Paths,
    image: &Path,
    manifest: &TargetBuildManifest,
    opts: &GlobalOpts,
) -> Result<Value, ConduitosError> {
    let image_sha256 = sha256_file(image)?;
    if image_sha256 != manifest.image.sha256 {
        return Err(refusal("verified normal product image changed before boot"));
    }
    let source = git_head(&paths.root)?;
    let mut boots = Vec::with_capacity(2);
    for run in ["first", "second"] {
        ia32_product_boot::boot_legacy_bios(
            image,
            &manifest.profile_id,
            &manifest.build_id,
            &manifest.resolved_description_binding,
            opts,
        )?;
        let receipt_path = paths.target.join("ia32-legacy-bios-product-proof.json");
        let receipt: Value = serde_json::from_slice(&fs::read(&receipt_path).map_err(io_error)?)
            .map_err(|error| refusal(error.to_string()))?;
        let transcript = fs::read_to_string(paths.target.join("ia32-product-legacy-bios-only.log"))
            .map_err(io_error)?;
        if receipt["base_commit"] != source
            || receipt["image_sha256"] != image_sha256
            || sha256_file(image)? != image_sha256
            || transcript.contains("CONDUIT_IA32_DOMAIN_NEGATIVES")
        {
            return Err(refusal(
                "normal product identity changed or diagnostic entries were enabled",
            ));
        }
        for (from, suffix) in [
            ("ia32-legacy-bios-product-proof.json", "receipt.json"),
            ("ia32-product-legacy-bios-only.log", "serial.log"),
            ("ia32-product-legacy-bios-only-vga.json", "vga.json"),
            ("ia32-product-legacy-bios-only-vga.bin", "vga.bin"),
        ] {
            fs::copy(
                paths.target.join(from),
                paths
                    .target
                    .join(format!("ia32-normal-domain-{run}-{suffix}")),
            )
            .map_err(io_error)?;
        }
        boots.push(receipt);
    }
    if git_head(&paths.root)? != source {
        return Err(refusal("source changed during normal product proof"));
    }
    validate_pair(&boots[0]["product"], &boots[1]["product"])?;
    let receipt = json!({
        "schema":"conduit.conduitos/ia32-normal-domain-product-proof@1",
        "proof_class":"freestanding-ia32-legacy-bios-emulator",
        "base_commit":source,"image_sha256":image_sha256,
        "first":boots[0]["product"],"second":boots[1]["product"],
        "first_observatory":boots[0]["observatory"],
        "second_observatory":boots[1]["observatory"],
        "fresh_host_id":true,"fresh_boot_id":true,"fresh_realization_identities":true,
        "same_immutable_image":true,"diagnostic_entries_enabled":false,
        "physical_machine_booted":false,
    });
    fs::write(
        paths.target.join("ia32-normal-domain-product-proof.json"),
        serde_json::to_vec_pretty(&receipt).map_err(|error| refusal(error.to_string()))?,
    )
    .map_err(io_error)?;
    Ok(receipt)
}

fn validate_pair(first: &Value, second: &Value) -> Result<(), ConduitosError> {
    for field in ["host_id", "boot_id", "ordinary_plan_id", "ordinary_play_id"] {
        if first[field].as_str().is_none_or(str::is_empty)
            || second[field].as_str().is_none_or(str::is_empty)
            || first[field] == second[field]
        {
            return Err(refusal(format!(
                "independent normal boots reused or omitted {field}"
            )));
        }
    }
    for field in [
        "source_document_id",
        "checked_plot_id",
        "expanded_plot_id",
        "semantic_shape_sha256",
    ] {
        let first = &first["ordinary_source_conformance"][field];
        let second = &second["ordinary_source_conformance"][field];
        if first.as_str().is_none_or(str::is_empty) || first != second {
            return Err(refusal(format!("normal boots disagree on {field}")));
        }
    }
    Ok(())
}

fn refusal(detail: impl Into<String>) -> ConduitosError {
    ConduitosError::refusal("ia32-normal-domain-product-unproven", detail)
}
fn io_error(error: std::io::Error) -> ConduitosError {
    refusal(error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn product(suffix: &str) -> Value {
        json!({"host_id":format!("host-{suffix}"),"boot_id":format!("boot-{suffix}"),
            "ordinary_plan_id":format!("plan-{suffix}"),"ordinary_play_id":format!("play-{suffix}"),
            "ordinary_source_conformance":{"source_document_id":"source","checked_plot_id":"checked",
                "expanded_plot_id":"expanded","semantic_shape_sha256":"shape"}})
    }

    #[test]
    fn repeated_normal_boots_require_fresh_realization_and_stable_meaning() {
        let first = product("first");
        let second = product("second");
        assert!(validate_pair(&first, &second).is_ok());
        for field in ["host_id", "boot_id", "ordinary_plan_id", "ordinary_play_id"] {
            let mut stale = second.clone();
            stale[field] = first[field].clone();
            assert!(validate_pair(&first, &stale).is_err(), "{field}");
            stale[field] = Value::Null;
            assert!(validate_pair(&first, &stale).is_err(), "missing {field}");
        }
        for field in [
            "source_document_id",
            "checked_plot_id",
            "expanded_plot_id",
            "semantic_shape_sha256",
        ] {
            let mut changed = second.clone();
            changed["ordinary_source_conformance"][field] = json!("changed");
            assert!(validate_pair(&first, &changed).is_err(), "{field}");
            changed["ordinary_source_conformance"][field] = Value::Null;
            assert!(validate_pair(&first, &changed).is_err(), "missing {field}");
        }
        assert!(validate_pair(&Value::Null, &Value::Null).is_err());
    }
}
