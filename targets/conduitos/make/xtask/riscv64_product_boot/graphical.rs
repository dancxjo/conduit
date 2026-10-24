//! Normal PROFILE graphical boot using verified prepared tools and private VARS.
use super::super::{
    emitted_line, ia32_firmware, report, riscv64_emulator, riscv64_firmware,
    target_build::graphical::Capture, ConduitosArch, ConduitosError,
};
use super::{refusal, Paths, OBSERVATORY_PREFIX, PRODUCT_PREFIX};
use serde_json::Value;
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    time::{Duration, Instant},
};

pub(super) fn boot_if_selected(
    paths: &Paths,
    image: &Path,
) -> Result<Option<(String, Value)>, ConduitosError> {
    let manifest_path = image
        .parent()
        .unwrap_or(Path::new("."))
        .join("build-manifest.json");
    if !manifest_path.exists() {
        return Ok(None);
    }
    if fs::metadata(&manifest_path).map_err(io)?.len() > 1024 * 1024 {
        return Err(io("manifest exceeds 1 MiB"));
    }
    let manifest: Value =
        serde_json::from_slice(&fs::read(manifest_path).map_err(io)?).map_err(io)?;
    if !manifest["resolved_build"]["presenters"]
        .as_array()
        .is_some_and(|p| p.iter().any(|p| p == "presenter/native-graphical@1"))
    {
        return Ok(None);
    }
    if manifest["artifact_role"] != "product-host"
        || manifest["target"] != "conduitos/riscv64/virt"
        || manifest["image"]["sha256"] != report::sha256_file(image)?
    {
        return Err(io("wrong graphical target, role or image bytes"));
    }
    let source = manifest["source_identity"]
        .as_str()
        .filter(|v| !v.is_empty())
        .ok_or_else(|| io("source identity missing"))?;
    let capture = Capture::prepare(&paths.target, ConduitosArch::Riscv64)?;
    let (qemu, sbi) = riscv64_emulator::selected_graphical(paths)?;
    let (code, template) = riscv64_firmware::paths(&paths.root).map_err(io)?;
    let vars = capture.serial_path().with_file_name("vars.fd");
    ia32_firmware::copy_vars(&template, &vars).map_err(io)?;
    let template_hash = report::sha256_file(&template)?;
    if report::sha256_file(&vars)? != template_hash {
        return Err(io("private VARS does not match template"));
    }
    let mut command = Command::new(&qemu);
    command
        .args([
            "-machine",
            "virt,acpi=off",
            "-cpu",
            "rv64,zkr=true,sv57=off,sv48=off",
            "-m",
            "2G",
            "-smp",
            "1",
            "-accel",
            "tcg,thread=single",
            "-bios",
        ])
        .arg(&sbi)
        .arg("-blockdev")
        .arg(format!(
            "node-name=pflash0,driver=file,read-only=on,filename={}",
            code.display()
        ))
        .arg("-blockdev")
        .arg(format!(
            "node-name=pflash1,driver=file,filename={}",
            vars.display()
        ))
        .args(["-machine", "pflash0=pflash0,pflash1=pflash1", "-drive"])
        .arg(format!(
            "file={},format=raw,if=none,id=boot,readonly=on",
            image.display()
        ))
        .args([
            "-device",
            "virtio-blk-pci,drive=boot,bootindex=0",
            "-device",
            "ramfb",
            "-display",
            "none",
            "-monitor",
            "none",
            "-serial",
        ])
        .arg(format!("file:{}", capture.serial_path().display()))
        .args(["-net", "none", "-qmp"])
        .arg(capture.endpoint())
        .arg("-no-reboot")
        .env("QEMU_AUDIO_DRV", "none")
        .env_remove("QEMU_MODULE_DIR");
    capture.record_launch(
        &command,
        &paths.root,
        &[&sbi, &code, &template, &vars],
        image,
        source,
    )?;
    let started = Instant::now();
    let mut child = command
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(fs::File::create(capture.stderr_path()).map_err(io)?)
        .spawn()
        .map_err(io)?;
    let result = (|| loop {
        if fs::metadata(capture.serial_path()).is_ok_and(|m| m.len() > 1024 * 1024) {
            return Err(io("serial exceeds 1 MiB"));
        }
        let bytes = fs::read(capture.serial_path()).unwrap_or_default();
        let transcript = String::from_utf8_lossy(&bytes);
        if let Some(reason) = transcript
            .split("CONDUIT_RISCV64_PRODUCT_REFUSAL ")
            .nth(1)
            .and_then(|s| s.lines().next())
        {
            return Err(refusal("riscv64-product-refused", reason));
        }
        if let (Some(product), Some(_)) = (
            emitted_line::complete_json_line(&transcript, PRODUCT_PREFIX),
            emitted_line::complete_json_line(&transcript, OBSERVATORY_PREFIX),
        ) {
            let product: Value = serde_json::from_str(product).map_err(io)?;
            let proof = capture.finish(&mut child, &transcript, &product, started)?;
            validate_mapping(&proof)?;
            return Ok(Some((transcript.into_owned(), proof)));
        }
        if started.elapsed() >= Duration::from_secs(45) {
            return Err(io("graphical product boot deadline exceeded"));
        }
        if let Some(status) = child.try_wait().map_err(io)? {
            return Err(io(format!(
                "guest exited before complete product evidence: {status}"
            )));
        }
        std::thread::sleep(Duration::from_millis(100));
    })();
    let _ = child.kill();
    let _ = child.wait();
    if report::sha256_file(&template)? != template_hash {
        return Err(io("firmware template changed during boot"));
    }
    result
}
fn validate_mapping(proof: &Value) -> Result<(), ConduitosError> {
    let evidence = &proof["mapping"];
    let m = &evidence["mapping"];
    let f = &proof["face"]["framebuffer"];
    let bytes = f["pitch_bytes"]
        .as_u64()
        .and_then(|pitch| {
            f["height"]
                .as_u64()
                .and_then(|height| pitch.checked_mul(height))
        })
        .ok_or_else(|| io("framebuffer extent missing"))?;
    let pages = bytes.div_ceil(4096);
    let descriptors = m["descriptors_read"].as_u64().unwrap_or(0);
    if evidence["schema"] != "conduit.conduitos/riscv64-framebuffer-mapping@1"
        || evidence["cache_policy"] != "pma-inherited-qemu-tcg-shared-ram"
        || evidence["store_completion"] != "volatile-stores-fence-iorw-iorw"
        || evidence["exclusive_writer"] != true
        || evidence["framebuffer_reserved"] != true
        || evidence["table_page_custody"] != "bootloader-reclaimable"
        || evidence["physical_memory_type_claimed"] != false
        || bytes == 0
        || bytes > 8 * 1024 * 1024
        || m["byte_length"] != bytes
        || m["checked_pages"] != pages
        || !(pages..=pages * 3).contains(&descriptors)
        || m["satp"].as_u64().is_none_or(|v| v >> 60 != 8)
        || m["physical_start"].as_u64().is_none_or(|v| {
            v == 0
                || !v.is_multiple_of(4096)
                || v.checked_add(bytes).is_none_or(|end| end > 1 << 56)
        })
        || m["virtual_start"]
            .as_u64()
            .is_none_or(|v| !v.is_multiple_of(4096))
    {
        return Err(io(
            "mapping or completion evidence disagrees with admitted scanout",
        ));
    }
    Ok(())
}
fn io(error: impl std::fmt::Display) -> ConduitosError {
    refusal("riscv64-graphical-proof-unavailable", error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn proof() -> Value {
        json!({"face":{"framebuffer":{"pitch_bytes":3200,"height":600}},
            "mapping":{"schema":"conduit.conduitos/riscv64-framebuffer-mapping@1",
                "cache_policy":"pma-inherited-qemu-tcg-shared-ram",
                "store_completion":"volatile-stores-fence-iorw-iorw",
                "exclusive_writer":true,"framebuffer_reserved":true,
                "table_page_custody":"bootloader-reclaimable","physical_memory_type_claimed":false,
                "mapping":{"physical_start":2147483648_u64,"virtual_start":18446743798831644672_u64,
                    "byte_length":1920000,"checked_pages":469,"descriptors_read":1407,"satp":9223372036854775809_u64}}})
    }
    #[test]
    fn mapping_custody_completion_and_each_page_must_agree_with_frame() {
        assert!(validate_mapping(&proof()).is_ok());
        for (field, value) in [
            ("exclusive_writer", json!(false)),
            ("framebuffer_reserved", json!(false)),
            ("table_page_custody", json!("usable-ram")),
            ("store_completion", json!("none")),
            ("physical_memory_type_claimed", json!(true)),
        ] {
            let mut p = proof();
            p["mapping"][field] = value;
            assert!(validate_mapping(&p).is_err());
        }
        for (field, value) in [
            ("physical_start", json!(123)),
            ("byte_length", json!(4096)),
            ("checked_pages", json!(468)),
            ("descriptors_read", json!(1408)),
            ("satp", json!(0)),
        ] {
            let mut p = proof();
            p["mapping"]["mapping"][field] = value;
            assert!(validate_mapping(&p).is_err());
        }
    }
}
