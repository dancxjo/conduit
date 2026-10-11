//! Package an existing product kernel with exact, locally approved protocol input.
use super::{image, report::sha256_file, ConduitosError};
use crate::cli::GlobalOpts;
use clap::Args;
use conduitos::{
    protocol_boot::{ProtocolBootRequest, UsbProtocolBootRequest, MAXIMUM_REQUEST_BYTES},
    protocol_source::{PreparedProtocolEntry, MAXIMUM_PACKAGE_BYTES},
};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use std::{
    fs,
    io::Read,
    path::{Path, PathBuf},
};

#[derive(Args, Debug)]
pub(super) struct ImageArgs {
    /// Existing capable x86_64 product kernel; no target build occurs.
    #[arg(long)]
    kernel: PathBuf,
    /// Product build record binding the exact existing kernel digest.
    #[arg(long)]
    build_record: PathBuf,
    /// Exact bounded Source package.
    #[arg(long)]
    package: PathBuf,
    /// Local administrator request, including approved Source digest and native bounds.
    #[arg(long)]
    root_request: PathBuf,
    /// New directory for boot media and preparation receipts.
    #[arg(long)]
    output_dir: PathBuf,
}

pub(super) fn execute(args: ImageArgs, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        println!(
            "Package existing {} with {} and {} into {}",
            args.kernel.display(),
            args.package.display(),
            args.root_request.display(),
            args.output_dir.display()
        );
        return Ok(());
    }
    let record: Value = serde_json::from_slice(&read(&args.build_record, 16384)?)
        .map_err(|error| refusal("protocol-kernel-record-refused", error))?;
    check_kernel(&record, &sha256_file(&args.kernel)?)?;
    let package = read(&args.package, MAXIMUM_PACKAGE_BYTES)?;
    let request_bytes = read(&args.root_request, MAXIMUM_REQUEST_BYTES)?;
    let request = SelectedRootRequest::decode(&request_bytes)?;
    request.bind_package(&package)?;
    let root_kind = request.kind();
    let entry = PreparedProtocolEntry::prepare(&package, request.entry())
        .map_err(|error| refusal("protocol-source-preparation-refused", format!("{error:?}")))?;
    // Preserve existing media and evidence; the administrator supplies independent
    // firmware/electrical approval. Checking Source does not admit native owners.
    fs::create_dir(&args.output_dir).map_err(io_error)?;
    let modules = image::ProtocolModules {
        kernel: &args.kernel,
        output: &args.output_dir,
        source: &package,
        request: &request_bytes,
    };
    let image = match root_kind {
        image::ProtocolRootKind::I2c => image::assemble_protocol(modules, opts),
        image::ProtocolRootKind::Usb => image::assemble_protocol_root(modules, root_kind, opts),
    }?;
    check_kernel(
        &record,
        &sha256_file(&args.output_dir.join("iso-root/boot/conduitos"))?,
    )?;
    let expanded = &entry.expanded().expanded;
    let receipt = json!({
        "schema": "conduit.conduitos/protocol-image-receipt@1",
        "proof_class": "source-check-and-image-packaging",
        "kernel_sha256": record["elf_sha256"],
        "image_sha256": image.iso_sha256,
        "source_artifact_id": entry.artifact_id().as_str(),
        "source_document_id": expanded.source_document_id.as_str(),
        "checked_plot_id": expanded.checked_plot_id.as_str(),
        "expanded_plot_id": expanded.expanded_plot_id.as_str(),
        "root_kind": root_kind.name(),
        "root_request_schema": root_kind.schema(),
        "root_module_command": root_kind.command(),
        "root_request_sha256": format!("{:x}", Sha256::digest(&request_bytes)),
        "native_execution": "not-yet-observed",
    });
    fs::write(
        args.output_dir.join("protocol-receipt.json"),
        serde_json::to_vec_pretty(&receipt)
            .map_err(|error| refusal("protocol-image-receipt", error))?,
    )
    .map_err(io_error)?;
    Ok(())
}

enum SelectedRootRequest {
    I2c(ProtocolBootRequest),
    Usb(UsbProtocolBootRequest),
}
impl SelectedRootRequest {
    fn decode(bytes: &[u8]) -> Result<Self, ConduitosError> {
        let value: Value = serde_json::from_slice(bytes)
            .map_err(|error| refusal("protocol-root-request-refused", error))?;
        let decode_error = |error| refusal("protocol-root-request-refused", format!("{error:?}"));
        match value.get("schema").and_then(Value::as_str) {
            Some(schema) if schema == image::ProtocolRootKind::I2c.schema() => {
                ProtocolBootRequest::decode(bytes)
                    .map(Self::I2c)
                    .map_err(decode_error)
            }
            Some(schema) if schema == image::ProtocolRootKind::Usb.schema() => {
                UsbProtocolBootRequest::decode(bytes)
                    .map(Self::Usb)
                    .map_err(decode_error)
            }
            _ => Err(refusal(
                "protocol-root-request-refused",
                "requires an explicit supported Root request schema",
            )),
        }
    }
    fn kind(&self) -> image::ProtocolRootKind {
        match self {
            Self::I2c(_) => image::ProtocolRootKind::I2c,
            Self::Usb(_) => image::ProtocolRootKind::Usb,
        }
    }
    fn entry(&self) -> &str {
        match self {
            Self::I2c(request) => &request.entry,
            Self::Usb(request) => &request.entry,
        }
    }
    fn bind_package(&self, bytes: &[u8]) -> Result<(), ConduitosError> {
        let result = match self {
            Self::I2c(request) => request.bind_package(bytes),
            Self::Usb(request) => request.bind_package(bytes),
        };
        result.map_err(|error| refusal("protocol-source-binding-refused", format!("{error:?}")))
    }
}

fn check_kernel(record: &Value, digest: &str) -> Result<(), ConduitosError> {
    if record["schema"] != "conduit.conduitos.build/v2"
        || record["artifact_role"] != "product-host"
        || record["architecture"] != "x86_64"
        || record["elf_sha256"] != digest
    {
        return Err(refusal(
            "protocol-kernel-record-refused",
            "requires the exact x86_64 product kernel and build record",
        ));
    }
    Ok(())
}

fn read(path: &Path, maximum: usize) -> Result<Vec<u8>, ConduitosError> {
    let mut bytes = Vec::new();
    fs::File::open(path)
        .map_err(io_error)?
        .take(maximum as u64 + 1)
        .read_to_end(&mut bytes)
        .map_err(io_error)?;
    if bytes.is_empty() || bytes.len() > maximum {
        return Err(refusal("protocol-image-input-bounds", path.display()));
    }
    Ok(bytes)
}
fn refusal(reason: &'static str, detail: impl std::fmt::Display) -> ConduitosError {
    ConduitosError::refusal(reason, detail.to_string())
}
fn io_error(error: std::io::Error) -> ConduitosError {
    refusal("protocol-image-io", error)
}

#[cfg(test)]
mod tests {
    use super::*;
    fn usb_request() -> Value {
        json!({"schema":image::ProtocolRootKind::Usb.schema(), "entry":"usb-control",
            "package_sha256":format!("{:x}", Sha256::digest(b"package")),
            "administrator_approval":true,"firmware_handoff":true,"electrical_attachment_approved":true,
            "root_port":1,"attachment_epoch":7,"maximum_control_operations":64,"maximum_steps":10000,"inputs":[]})
    }
    #[test]
    fn explicit_usb_schema_selects_usb_command_and_exact_source_binding() {
        let request =
            SelectedRootRequest::decode(&serde_json::to_vec(&usb_request()).unwrap()).unwrap();
        assert_eq!(request.kind(), image::ProtocolRootKind::Usb);
        assert_eq!(
            request.kind().command(),
            "conduit.protocol/usb-root-request@1"
        );
        assert_eq!(request.entry(), "usb-control");
        request.bind_package(b"package").unwrap();
        assert!(request.bind_package(b"different").is_err());
    }
    #[test]
    fn schema_dispatch_never_falls_back_to_another_root() {
        for schema in [image::ProtocolRootKind::I2c.schema(), "unknown"] {
            let mut request = usb_request();
            request["schema"] = schema.into();
            assert!(SelectedRootRequest::decode(&serde_json::to_vec(&request).unwrap()).is_err());
        }
        let mut request = usb_request();
        request.as_object_mut().unwrap().remove("schema");
        assert!(SelectedRootRequest::decode(&serde_json::to_vec(&request).unwrap()).is_err());
        let mut request = usb_request();
        request["pci_device"] = 31.into();
        assert!(SelectedRootRequest::decode(&serde_json::to_vec(&request).unwrap()).is_err());
    }
    #[test]
    fn proof_appliances_foreign_architectures_and_changed_kernels_cannot_be_packaged_as_product() {
        let record = json!({"schema":"conduit.conduitos.build/v2",
            "artifact_role":"product-host", "architecture":"x86_64", "elf_sha256":"abc"});
        check_kernel(&record, "abc").unwrap();
        assert!(check_kernel(&record, "changed").is_err());
        for (field, invalid) in [
            ("artifact_role", "architecture-proof-appliance"),
            ("architecture", "aarch64"),
            ("schema", "unknown"),
        ] {
            let mut other = record.clone();
            other[field] = invalid.into();
            assert!(check_kernel(&other, "abc").is_err());
        }
    }
}
