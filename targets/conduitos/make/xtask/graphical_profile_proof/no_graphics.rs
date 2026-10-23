//! Ordinary product with the graphics device genuinely absent, before any Show.
use super::{io_error, ConduitosError, Paths};
use serde_json::{json, Value};
use std::{
    fs,
    path::Path,
    process::{Command, Stdio},
    thread,
    time::{Duration, Instant},
};
const MAX_SERIAL_BYTES: u64 = 16 * 1024 * 1024;

pub(super) fn exercise(
    paths: &Paths,
    image: &Path,
    digest: &str,
    source: &str,
    metadata: &Value,
) -> Result<(), ConduitosError> {
    fs::create_dir_all(&paths.target).map_err(io_error)?;
    let serial = paths.target.join("graphics-provider-absence-serial.log");
    let errors = paths.target.join("graphics-provider-absence-stderr.log");
    let receipt = paths.target.join("graphics-provider-absence-proof.json");
    for path in [&serial, &receipt] {
        match fs::remove_file(path) {
            Ok(()) => {}
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => {}
            Err(error) => return Err(io_error(error)),
        }
    }
    let mut child = Command::new("qemu-system-x86_64")
        .args([
            "-M",
            "q35",
            "-cpu",
            "max",
            "-m",
            conduitos::make::PC_PRODUCT_QEMU_MEMORY,
            "-smp",
            "1",
            "-display",
            "none",
            "-vga",
            "none",
            "-monitor",
            "none",
            "-serial",
            &format!("file:{}", serial.display()),
            "-no-reboot",
            "-net",
            "none",
            "-device",
            "qemu-xhci,id=conduitos-xhci,p2=3,p3=0",
            "-device",
            "usb-kbd,id=conduitos-keyboard,bus=conduitos-xhci.0,port=1",
            "-device",
            "usb-mouse,id=conduitos-pointer,bus=conduitos-xhci.0,port=2",
            "-device",
            "isa-debug-exit,iobase=0xf4,iosize=0x04",
            "-cdrom",
        ])
        .arg(image)
        .args(["-boot", "d"])
        .current_dir(&paths.root)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::from(fs::File::create(&errors).map_err(io_error)?))
        .spawn()
        .map_err(io_error)?;
    let deadline = Instant::now() + Duration::from_secs(45);
    let result = loop {
        match child.try_wait() {
            Ok(Some(status)) => break Ok(status),
            Ok(None) if Instant::now() < deadline => thread::sleep(Duration::from_millis(20)),
            Ok(None) => break Err(refusal("no bounded provider-absence result")),
            Err(error) => break Err(io_error(error)),
        }
    };
    if result.is_err() {
        let _ = child.kill();
        let _ = child.wait();
    }
    let status = result?;
    if fs::metadata(&serial).map_err(io_error)?.len() > MAX_SERIAL_BYTES {
        return Err(refusal("provider-absence serial bound exceeded"));
    }
    let transcript = fs::read_to_string(&serial).map_err(io_error)?;
    let observation = validate(&transcript, status.code())?;
    let record = json!({
        "schema":"conduit.conduitos/graphics-provider-absence-proof@1", "status":"passed",
        "proof_class":"freestanding-emulator", "fixture":false,
        "same_uninstrumented_product_binary":true, "source_commit":source, "image_sha256":digest,
        "canonical_build_metadata":metadata, "observation":observation,
        "machine":{"emulator":"qemu-system-x86_64", "model":"q35", "cpu":"max",
            "memory":conduitos::make::PC_PRODUCT_QEMU_MEMORY, "vcpus":1,
            "graphics_device":"none", "display_window":"none", "network":"none",
            "debug_exit":"external exit witness, no product feature"},
        "serial_sha256":super::super::report::sha256_file(&serial)?,
        "fallback":"typed terminal startup refusal before Face/Show; no graphical substitute invented",
        "boot_identity":"not published: display setup refuses before ordinary Boot/Host/Show publication",
        "hot_unplug_recovery":false, "physical_evidence":false, "human_enactment":false,
        "screenshots":"not applicable: no graphics device"
    });
    fs::write(
        receipt,
        serde_json::to_vec_pretty(&record).map_err(|error| refusal(&error.to_string()))?,
    )
    .map_err(io_error)
}

fn validate(serial: &str, exit: Option<i32>) -> Result<Value, ConduitosError> {
    if exit != Some(35) || !serial.ends_with('\n') || serial.len() as u64 > MAX_SERIAL_BYTES {
        return Err(refusal(
            "requires complete transcript and exact product refusal exit 35",
        ));
    }
    let mut signs = Vec::new();
    let mut display_phases = Vec::new();
    for line in serial.lines() {
        if [
            "CONDUIT_NATIVE_FACE ",
            "CONDUIT_FACE_SIGN ",
            "CONDUIT_CRECHE_CHANGE ",
            "CONDUIT_NATIVE_BIRTH ",
            "CONDUIT_COMPOSITOR_SERVICE_FRAME ",
            "CONDUIT_JOURNEY_SIGN ",
        ]
        .iter()
        .any(|prefix| line.starts_with(prefix))
        {
            return Err(refusal(
                "missing provider must not manufacture a Show, scanout or source effect",
            ));
        }
        if let Some(encoded) = line.strip_prefix("CONDUIT_KERNEL_SIGN ") {
            signs.push(
                serde_json::from_str::<Value>(encoded)
                    .map_err(|_| refusal("malformed refusal Sign"))?,
            );
        }
        if let Some(encoded) = line.strip_prefix("CONDUIT_NATIVE_PROGRESS ") {
            let value: Value =
                serde_json::from_str(encoded).map_err(|_| refusal("malformed progress"))?;
            if value["phase"] == "display-setup" {
                if value["schema"] != "conduit.conduitos/native-progress@1"
                    || value["scope"] != "native-root-boot"
                    || value["context"] != "boot"
                {
                    return Err(refusal("foreign display setup observation"));
                }
                display_phases.push(value);
            }
        }
    }
    let expected = json!({"schema":"conduit.conduitos.kernel-sign/v2", "status":"refused", "reason":"display-absent"});
    if signs != vec![expected.clone()]
        || display_phases.len() != 2
        || display_phases[0]["state"] != "started"
        || display_phases[1]["state"] != "refused"
        || display_phases[0]["span"] != display_phases[1]["span"]
        || display_phases[0]["span"].as_u64().is_none()
        || !serial
            .lines()
            .any(|line| line == "CONDUIT_BOOT_STAGE usb-configured")
    {
        return Err(refusal(
            "expected one typed display-absent refusal after actual display setup",
        ));
    }
    Ok(
        json!({"refusal":expected,"exit_code":exit,"display_setup":display_phases,
        "acknowledged_show_count":0,"source_change_count":0,"scanout_frame_count":0}),
    )
}
fn refusal(detail: &str) -> ConduitosError {
    ConduitosError::refusal("graphics-provider-absence-mismatch", detail)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn transcript() -> String {
        let mut text = String::from("CONDUIT_BOOT_STAGE usb-configured\n");
        for state in ["started", "refused"] {
            text.push_str(&format!(
                "CONDUIT_NATIVE_PROGRESS {}\n",
                json!({
                    "schema":"conduit.conduitos/native-progress@1","scope":"native-root-boot",
                    "phase":"display-setup","context":"boot","span":5,"state":state
                })
            ));
        }
        text.push_str("CONDUIT_KERNEL_SIGN {\"schema\":\"conduit.conduitos.kernel-sign/v2\",\"status\":\"refused\",\"reason\":\"display-absent\"}\n");
        text
    }
    #[test]
    fn provider_absence_requires_exact_refusal_and_no_forged_show_or_effect() {
        let good = transcript();
        assert!(validate(&good, Some(35)).is_ok());
        for bad in [
            good.replace("display-absent", "display-base-lost"),
            format!("{good}{good}"),
            format!("{good}CONDUIT_NATIVE_FACE {{}}\n"),
            format!("{good}CONDUIT_CRECHE_CHANGE {{}}\n"),
            good.replace("\"span\":5", "\"span\":null"),
            good.trim_end().to_owned(),
            String::from("CONDUIT_KERNEL_SIGN {\n"),
        ] {
            assert!(validate(&bad, Some(35)).is_err());
        }
        assert!(validate(&good, Some(33)).is_err());
        assert!(validate(&good, None).is_err());
    }
}
