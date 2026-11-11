//! Physical product capture: exact image binding, Show acknowledgement and cue completion.
use super::{
    armv6_rpi_board::Armv6RpiBoard, armv6_rpi_physical, profile::Paths, report::sha256_file,
    ConduitosArch, ConduitosError,
};
use crate::cli::GlobalOpts;
use serde_json::{json, Value};
use std::{
    fs::{self, File},
    io::{Read, Write},
    path::Path,
    thread,
    time::{Duration, Instant, SystemTime, UNIX_EPOCH},
};
const MAX_BYTES: usize = 128 * 1024; // The shared Face evidence is bounded to 96 KiB.
const READY: &[u8] = b"CONDUIT_BOOT_STAGE armv6-product-ready\n";

pub(super) fn execute(
    board: Armv6RpiBoard,
    device: &Path,
    seconds: u64,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    if board != Armv6RpiBoard::BPlusV1_2 {
        return Err(refusal("product-board-unsupported", board.id()));
    }
    armv6_rpi_physical::inspect_serial(device)?;
    if opts.dry_run {
        println!("capture at most {MAX_BYTES} bytes from {} for {seconds}s and verify the flashed product identities, framebuffer Show and startup Plot",device.display());
        return Ok(());
    }
    let paths = Paths::new(ConduitosArch::Armv6)?;
    let manifest = read_json(&paths.target.join("armv6-product-manifest.json"))?;
    let image = read_json(&paths.target.join("rpi-b-plus-image.json"))?;
    let flash = read_json(&paths.target.join("rpi-b-plus-flash.json"))?;
    if flash["write_completed"] != true
        || flash["byte_verification_completed"] != true
        || flash["image_sha256"] != image["image_sha256"]
        || image["image_id"] != manifest["image_id"]
        || image["image_sha256"] != sha256_file(&paths.target.join("conduitos-rpi-b-plus.img"))?
    {
        return Err(refusal(
            "product-flash-binding-invalid",
            "build and flash the exact product image before capture",
        ));
    }
    armv6_rpi_physical::configure_serial(device)?;
    let epoch = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|e| refusal("capture-clock", e))?
        .as_millis();
    let raw_path = paths
        .target
        .join(format!("rpi-b-plus-product-uart-{epoch}.txt"));
    eprintln!(
        "Waiting up to {seconds}s for the B+ product on {}; preserving all bytes in {}",
        device.display(),
        raw_path.display()
    );
    let raw = capture(device, Duration::from_secs(seconds), &raw_path)?;
    let text = String::from_utf8(raw).map_err(|e| refusal("product-uart-invalid-utf8", e))?;
    let boot = one(&text, "CONDUIT_PRODUCT_BOOT ")?;
    let face = one(&text, "CONDUIT_FIRMWARE_FACE ")?;
    let startup = one(&text, "CONDUIT_STARTUP_PLOT ")?;
    validate(&manifest, &boot, &face, &startup)?;
    let record = json!({
        "schema":"conduit.conduitos/bplus-product-uart@1",
        "proof_class":"physical-uart-product-framebuffer-show-and-startup-completion",
        "target":manifest["target"],"source_identity":manifest["source_identity"],
        "profile_id":manifest["profile_id"],"build_id":manifest["build_id"],"image_id":manifest["image_id"],
        "image_sha256":image["image_sha256"],"firmware_board_revision":boot["firmware_board_revision"],
        "transcript_path":raw_path,"transcript_sha256":sha256_file(&raw_path)?,
        "physical_boot_proved":true,"framebuffer_show_acknowledged":true,"pcm_completed":true,
        "hdmi_seen_by_person":false,"sound_heard_by_person":false,"accepted_release_claimed":false,
        "startup":startup
    });
    fs::write(
        paths.target.join("rpi-b-plus-product-physical.json"),
        serde_json::to_vec_pretty(&record).map_err(|e| refusal("product-receipt-encoding", e))?,
    )
    .map_err(|e| refusal("product-receipt-write", e))?;
    if opts.json {
        println!("{record}");
    } else {
        println!("Accepted B+ product UART proof: framebuffer Show and startup PCM completed");
    }
    Ok(())
}
fn capture(device: &Path, timeout: Duration, path: &Path) -> Result<Vec<u8>, ConduitosError> {
    let mut serial = File::open(device).map_err(|e| refusal("serial-open-failed", e))?;
    let mut file = File::create(path).map_err(|e| refusal("product-transcript-write", e))?;
    let mut raw = Vec::new();
    let deadline = Instant::now() + timeout;
    let mut buffer = [0_u8; 1024];
    while Instant::now() < deadline {
        let count = serial
            .read(&mut buffer)
            .map_err(|e| refusal("serial-read-failed", e))?;
        if count == 0 {
            thread::sleep(Duration::from_millis(10));
            continue;
        }
        if raw.len() + count > MAX_BYTES {
            return Err(refusal("product-transcript-pressure", raw.len() + count));
        }
        file.write_all(&buffer[..count])
            .map_err(|e| refusal("product-transcript-write", e))?;
        raw.extend_from_slice(&buffer[..count]);
        if raw.ends_with(READY) {
            return Ok(raw);
        }
    }
    Err(refusal(
        "product-uart-timeout",
        format!("captured {} bytes; retained {}", raw.len(), path.display()),
    ))
}
fn read_json(path: &Path) -> Result<Value, ConduitosError> {
    serde_json::from_slice(&fs::read(path).map_err(|e| refusal("product-record-unavailable", e))?)
        .map_err(|e| refusal("product-record-invalid", e))
}
fn one(text: &str, prefix: &str) -> Result<Value, ConduitosError> {
    let mut lines = text.lines().filter_map(|line| line.strip_prefix(prefix));
    let line = lines
        .next()
        .ok_or_else(|| refusal("product-sign-missing", prefix))?;
    if lines.next().is_some() {
        return Err(refusal("product-sign-duplicate", prefix));
    }
    serde_json::from_str(line).map_err(|e| refusal("product-sign-invalid", e))
}
fn validate(
    manifest: &Value,
    boot: &Value,
    face: &Value,
    startup: &Value,
) -> Result<(), ConduitosError> {
    let revision = boot["firmware_board_revision"]
        .as_u64()
        .and_then(|v| u32::try_from(v).ok())
        .ok_or_else(|| refusal("product-board-invalid", "missing revision"))?;
    if !Armv6RpiBoard::BPlusV1_2.accepts_revision(revision) || boot["target"] != manifest["target"]
    {
        return Err(refusal(
            "product-board-mismatch",
            "UART does not identify the B+ product",
        ));
    }
    for field in ["profile_id", "build_id", "image_id"] {
        if boot[field] != manifest[field] || face[field] != manifest[field] {
            return Err(refusal("product-image-mismatch", field));
        }
    }
    if face["status"] != "shown"
        || face["mask_render"]["scanout_pixels_written"]
            .as_u64()
            .unwrap_or(0)
            == 0
    {
        return Err(refusal(
            "product-show-missing",
            "no completed framebuffer Show",
        ));
    }
    if startup["source"] != "plots/startup-chime/main.conduit"
        || startup["outcome"] != "completed"
        || startup["frames"] != 57_600
        || startup["allocation_stable_during_play"] != true
    {
        return Err(refusal(
            "product-startup-incomplete",
            "ordinary startup plot did not complete its PCM operation without allocations",
        ));
    }
    Ok(())
}
fn refusal(reason: &'static str, error: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, error.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn physical_receipt_requires_exact_image_show_and_completed_plot() {
        let manifest = json!({"target":"raspberry-pi-model-b-plus-v1.2", "profile_id":"profile/a", "build_id":"build/a", "image_id":"image/a"});
        let boot = json!({"firmware_board_revision":16,"target":manifest["target"],"profile_id":"profile/a","build_id":"build/a","image_id":"image/a"});
        let face = json!({"status":"shown","profile_id":"profile/a","build_id":"build/a","image_id":"image/a","mask_render":{"scanout_pixels_written":480000}});
        let startup = json!({"source":"plots/startup-chime/main.conduit","outcome":"completed","frames":57600,"allocation_stable_during_play":true});
        validate(&manifest, &boot, &face, &startup).unwrap();
        let mut wrong = boot.clone();
        wrong["image_id"] = json!("image/other");
        assert!(validate(&manifest, &wrong, &face, &startup).is_err());
        wrong = boot.clone();
        wrong["firmware_board_revision"] = json!(0xa02082);
        assert!(validate(&manifest, &wrong, &face, &startup).is_err());
        let mut hidden = face.clone();
        hidden["mask_render"]["scanout_pixels_written"] = json!(0);
        assert!(validate(&manifest, &boot, &hidden, &startup).is_err());
        let mut failed = startup.clone();
        failed["outcome"] = json!("failed");
        assert!(validate(&manifest, &boot, &face, &failed).is_err());
        failed = startup.clone();
        failed["allocation_stable_during_play"] = json!(false);
        assert!(validate(&manifest, &boot, &face, &failed).is_err());
        assert!(one(
            "CONDUIT_PRODUCT_BOOT {}\nCONDUIT_PRODUCT_BOOT {}\n",
            "CONDUIT_PRODUCT_BOOT "
        )
        .is_err());
    }
}
