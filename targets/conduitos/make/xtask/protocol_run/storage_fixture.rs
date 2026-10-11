//! Generated, bounded emulator storage; no external disk input or class driver.
use super::{ConduitosError, refusal, sha256_file};
use serde_json::{Value, json};
use std::{
    fs,
    io::Write,
    path::{Path, PathBuf},
    process::Command,
};

const IMAGE_BYTES: u64 = 1024 * 1024;
const DEVICE: &str =
    "usb-storage,id=protocol-storage,bus=protocol-usb.0,port=1,drive=protocol-storage-image";
const CONTROLLER: &str = "qemu-xhci,id=protocol-usb,p2=1,p3=0";
const DESCRIPTOR_SOURCE: &str =
    "https://gitlab.com/qemu-project/qemu/-/raw/v10.2.1/hw/usb/dev-storage.c";

pub(super) struct StorageFixture {
    image: PathBuf,
    digest: String,
    version: String,
    pub(super) arguments: Vec<String>,
}

impl StorageFixture {
    /// Caller has already created a new output directory. Never overwrite media.
    pub(super) fn create(directory: &Path) -> Result<Self, ConduitosError> {
        let output = Command::new("qemu-system-x86_64")
            .arg("--version")
            .output()
            .map_err(|error| refusal("protocol-run-emulator-unavailable", error))?;
        let version = String::from_utf8(output.stdout)
            .map_err(|error| refusal("protocol-run-fixture-version-refused", error))?;
        if !output.status.success() || !version_is_supported(&version) {
            return Err(refusal(
                "protocol-run-fixture-version-refused",
                "storage geometry requires QEMU 10.2.1",
            ));
        }
        let image = directory.join("usb-storage-fixture.raw");
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&image)
            .map_err(|error| refusal("protocol-run-fixture-create-refused", error))?;
        let block = [0u8; 4096];
        for _ in 0..IMAGE_BYTES / block.len() as u64 {
            file.write_all(&block)
                .map_err(|error| refusal("protocol-run-fixture-create-refused", error))?;
        }
        file.sync_all()
            .map_err(|error| refusal("protocol-run-fixture-create-refused", error))?;
        let mut permissions = file
            .metadata()
            .map_err(|error| refusal("protocol-run-fixture-create-refused", error))?
            .permissions();
        permissions.set_readonly(true);
        file.set_permissions(permissions)
            .map_err(|error| refusal("protocol-run-fixture-create-refused", error))?;
        drop(file);
        let image = fs::canonicalize(image)
            .map_err(|error| refusal("protocol-run-fixture-create-refused", error))?;
        let filename = image.to_str().ok_or_else(|| {
            refusal(
                "protocol-run-fixture-path-refused",
                "UTF-8 image path required",
            )
        })?;
        let arguments = arguments(filename);
        let digest = sha256_file(&image)?;
        Ok(Self {
            image,
            digest,
            version,
            arguments,
        })
    }

    pub(super) fn receipt(&self) -> Result<Value, ConduitosError> {
        if fs::metadata(&self.image)
            .map_err(|error| refusal("protocol-run-fixture-read-refused", error))?
            .len()
            != IMAGE_BYTES
            || sha256_file(&self.image)? != self.digest
        {
            return Err(refusal(
                "protocol-run-fixture-changed",
                "readonly generated image changed",
            ));
        }
        Ok(json!({
            "schema": "conduit.conduitos/usb-storage-emulator-fixture@1",
            "image": "usb-storage-fixture.raw",
            "image_sha256": self.digest,
            "image_bytes": IMAGE_BYTES,
            "initial_content": "zero-filled",
            "readonly": true,
            "qemu_version": self.version.trim_end(),
            "qemu_arguments": self.arguments,
            "descriptor_source": DESCRIPTOR_SOURCE,
            "geometry": {
                "controller": CONTROLLER, "device": DEVICE,
                "usb_speed": "high", "ep0_max_packet_bytes": 64,
                "configuration_value": 1, "interface_number": 0,
                "interface_class": 8, "interface_subclass": 6, "interface_protocol": 80,
                "bulk_in_address": 129, "bulk_out_address": 2, "bulk_max_packet_bytes": 512,
                "root_port": "selected independently by explicit product Root admission"
            }
        }))
    }
}

fn version_is_supported(version: &str) -> bool {
    version.lines().next().is_some_and(|line| {
        line == "QEMU emulator version 10.2.1" || line.starts_with("QEMU emulator version 10.2.1 ")
    })
}
fn arguments(filename: &str) -> Vec<String> {
    // JSON blockdev avoids comma/path option interpolation and has no writable layer.
    vec![
        "-device".into(),
        CONTROLLER.into(),
        "-blockdev".into(),
        json!({
            "driver": "raw", "node-name": "protocol-storage-image", "read-only": true,
            "file": { "driver": "file", "filename": filename, "read-only": true }
        })
        .to_string(),
        "-device".into(),
        DEVICE.into(),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn readonly_fixture_arguments_preserve_paths_and_exact_usb_geometry() {
        let args = arguments("/new proof/usb,fixture.raw");
        assert_eq!(args[1], CONTROLLER);
        assert_eq!(args[5], DEVICE);
        let backend: Value = serde_json::from_str(&args[3]).unwrap();
        assert_eq!(backend["read-only"], true);
        assert_eq!(backend["file"]["read-only"], true);
        assert_eq!(backend["file"]["filename"], "/new proof/usb,fixture.raw");
        assert_eq!(IMAGE_BYTES, 2048 * 512);
        assert!(version_is_supported(
            "QEMU emulator version 10.2.1 (vendor)\n"
        ));
        assert!(!version_is_supported("QEMU emulator version 10.2.10\n"));
        assert!(!version_is_supported("QEMU emulator version 10.2.0\n"));
    }
}
