//! Retained QMP capture for the separately selected normal IA-32 graphical image.
use super::super::{report, target_build::graphical::Capture, ConduitosArch};
use super::{refusal, ConduitosError, Paths};
use serde_json::{json, Value};
use std::{fs, path::Path, process::Command};

pub(super) struct Selected {
    pub capture: Capture,
    source: String,
}
impl Selected {
    pub fn prepare(
        image: &Path,
        paths: &Paths,
        uefi: bool,
    ) -> Result<Option<Self>, ConduitosError> {
        let manifest = image
            .parent()
            .unwrap_or(Path::new("."))
            .join("build-manifest.json");
        if !manifest.exists() {
            return Ok(None);
        }
        if fs::metadata(&manifest).map_err(io)?.len() > 1024 * 1024 {
            return Err(refusal(
                "ia32-graphical-manifest-invalid",
                "manifest exceeds 1 MiB",
            ));
        }
        let manifest: Value = serde_json::from_slice(&fs::read(manifest).map_err(io)?)
            .map_err(|e| refusal("ia32-graphical-manifest-invalid", e.to_string()))?;
        let native = manifest["resolved_build"]["presenters"]
            .as_array()
            .is_some_and(|v| v.iter().any(|p| p == "presenter/native-graphical@1"));
        if !native {
            return Ok(None);
        }
        if !uefi {
            return Err(refusal(
                "ia32-graphical-legacy-proof-unavailable",
                "graphical image has no VGA text receipt; select the separate serial profile for legacy text proof",
            ));
        }
        if manifest["artifact_role"] != "product-host"
            || manifest["target"] != "conduitos/ia32/pc"
            || manifest["image"]["sha256"] != report::sha256_file(image)?
        {
            return Err(refusal(
                "ia32-graphical-manifest-invalid",
                "wrong target, role or image bytes",
            ));
        }
        let source = manifest["source_identity"]
            .as_str()
            .filter(|v| !v.is_empty())
            .ok_or_else(|| refusal("ia32-graphical-manifest-invalid", "source identity missing"))?
            .to_owned();
        Ok(Some(Self {
            capture: Capture::prepare(&paths.target, ConduitosArch::Ia32)?,
            source,
        }))
    }
    pub fn finish(
        &self,
        child: &mut std::process::Child,
        transcript: &str,
        product: &Value,
        started: std::time::Instant,
    ) -> Result<Value, ConduitosError> {
        let proof = self.capture.finish(child, transcript, product, started)?;
        validate_mapping(&proof["mapping"], &product["framebuffer_observation"])?;
        Ok(proof)
    }
    pub fn record(
        &self,
        command: &mut Command,
        paths: &Paths,
        image: &Path,
        run: &str,
    ) -> Result<(), ConduitosError> {
        let (code, template) = super::ia32_a1::firmware_paths(paths)?;
        let vars = paths.target.join(format!("ia32-product-{run}-vars.fd"));
        if report::sha256_file(&vars)? != report::sha256_file(&template)? {
            return Err(refusal(
                "ia32-graphical-vars-invalid",
                "per-run VARS differs before guest execution",
            ));
        }
        command.arg("-qmp").arg(self.capture.endpoint());
        self.capture.record_launch(
            command,
            &paths.root,
            &[&code, &template, &vars],
            image,
            &self.source,
        )?;
        command.stderr(fs::File::create(self.capture.stderr_path()).map_err(io)?);
        Ok(())
    }
}

pub(super) fn finish_twice(
    image: &Path,
    image_sha256: &str,
    boots: [(Value, Value); 2],
) -> Result<(), ConduitosError> {
    let [(first, first_observatory), (second, second_observatory)] = boots;
    if first["host_id"] == second["host_id"]
        || first["boot_id"] == second["boot_id"]
        || second.get("graphical").is_none()
        || report::sha256_file(image)? != image_sha256
    {
        return Err(refusal(
            "stale-ia32-graphical-identity",
            "two fresh graphical boots of the same immutable image required",
        ));
    }
    let paths = Paths::new(ConduitosArch::Ia32)?;
    let snapshot = paths.target.join("ia32-product-observatory.json");
    fs::write(
        &snapshot,
        serde_json::to_vec_pretty(&first_observatory).map_err(io)?,
    )
    .map_err(io)?;
    super::prove_patchbay(&snapshot, &first)?;
    let proof = json!({"schema":"conduit.conduitos/ia32-product-proof@2",
        "proof_class":"freestanding-ia32-uefi32-graphical-product",
        "base_commit":report::git_head(&paths.root)?,"image_sha256":image_sha256,
        "first":first,"second":second,"first_observatory":first_observatory,
        "second_observatory":second_observatory,"fresh_host_id":true,"fresh_boot_id":true,
        "uefi32_booted":true,"legacy_bios_booted":false,"legacy_bios_vga_text_receipt":false,
        "same_immutable_image":true,"native_patchbay_consumed":true,
        "local_input_proved":false,"physical_machine_booted":false,
        "gpu_acceleration_proved":false,"stopped_by_harness":true});
    fs::write(
        paths.target.join("ia32-product-proof.json"),
        serde_json::to_vec_pretty(&proof).map_err(io)?,
    )
    .map_err(io)
}
fn io(error: impl std::fmt::Display) -> ConduitosError {
    refusal("ia32-graphical-proof-unavailable", error.to_string())
}

fn validate_mapping(mapping: &Value, observation: &Value) -> Result<(), ConduitosError> {
    let frame = &observation["framebuffer"];
    let line = mapping["cache_line_bytes"].as_u64().unwrap_or(0);
    if mapping["schema"] != "conduit.conduitos/ia32-framebuffer-mapping@1"
        || mapping["physical_start"] != frame["physical_start"]
        || mapping["virtual_start"] != frame["physical_start"]
        || mapping["byte_length"] != frame["byte_length"]
        || mapping["base_id"] != frame["base_id"]
        || mapping["mapping"] != "root-flat-protected-mode-paging-disabled"
        || mapping["store_completion"]
            != "volatile-word-stores-mfence-clflush-modified-lines-mfence"
        || mapping["memory_type_claimed"] != false
        || mapping["exclusive_writer"] != true
        || mapping["nonreserved_overlap"] != false
        || mapping["owned_storage_overlap"] != false
        || mapping["memory_map_entries"] != observation["memory_map"]["entries"]
        || mapping["covered_by_reserved_entry"]
            != observation["memory_map"]["covered_by_reserved_entry"]
        || !(16..=256).contains(&line)
        || !line.is_power_of_two()
        || frame["physical_start"]
            .as_u64()
            .is_none_or(|v| !v.is_multiple_of(line))
        || frame["byte_length"]
            .as_u64()
            .is_none_or(|v| !v.is_multiple_of(line))
    {
        return Err(refusal(
            "ia32-graphical-mapping-mismatch",
            "mapping/cache custody disagrees with actual framebuffer observation",
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn mapping_must_join_observed_memory_and_independent_completion_prerequisites() {
        let observation = json!({"framebuffer":{"physical_start":2147483648_u64,"byte_length":4096000,"base_id":"base"},
            "memory_map":{"entries":18,"covered_by_reserved_entry":false}});
        let mapping = json!({"schema":"conduit.conduitos/ia32-framebuffer-mapping@1",
            "physical_start":2147483648_u64,"virtual_start":2147483648_u64,"byte_length":4096000,"base_id":"base",
            "cache_line_bytes":64,"mapping":"root-flat-protected-mode-paging-disabled",
            "store_completion":"volatile-word-stores-mfence-clflush-modified-lines-mfence",
            "memory_type_claimed":false,"exclusive_writer":true,"nonreserved_overlap":false,"owned_storage_overlap":false,
            "memory_map_entries":18,"covered_by_reserved_entry":false});
        assert!(validate_mapping(&mapping, &observation).is_ok());
        for (field, value) in [
            ("physical_start", json!(1234)),
            ("virtual_start", json!(1234)),
            ("cache_line_bytes", json!(0)),
            ("cache_line_bytes", json!(48)),
            ("byte_length", json!(8192)),
            ("exclusive_writer", json!(false)),
            ("memory_map_entries", json!(17)),
            ("covered_by_reserved_entry", json!(true)),
            ("store_completion", json!("none")),
        ] {
            let mut altered = mapping.clone();
            altered[field] = value;
            assert!(
                validate_mapping(&altered, &observation).is_err(),
                "accepted {field}"
            );
        }
    }
}
