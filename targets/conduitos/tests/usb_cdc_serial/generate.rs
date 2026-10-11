//! Reproduce inert checked package/Root input for the separately owned product
//! proof. This does not run USB or establish native readiness or authority.
use super::super::*;
use sha2::{Digest, Sha256};
#[test]
fn emit_exact_selected_back_product_inputs_when_requested() {
    let Some(directory) = std::env::var_os("USB_CDC_PROOF_DIR") else {
        return;
    };
    let directory = std::path::PathBuf::from(directory);
    std::fs::create_dir_all(&directory).unwrap();
    let package = package();
    let bytes = serde_json::to_vec(&package).unwrap();
    let entry = PreparedProtocolEntry::prepare(&bytes, "cdc-serial-duplex-demonstration").unwrap();
    let schema = entry.input_schema(&port_id("seed")).unwrap();
    let input = request(&schema, "transmit", b"cdc-plot-tx\n")
        .canonical_bytes()
        .unwrap();
    let hash = format!("{:x}", Sha256::digest(&bytes));
    let root = serde_json::json!({
    "schema":"conduit.conduitos/usb-protocol-root-request@1", "entry":"cdc-serial-duplex-demonstration", "package_sha256":hash,
    "administrator_approval":true,"firmware_handoff":true,"electrical_attachment_approved":true,
    "root_port":1,"attachment_epoch":1,"maximum_control_operations":5,"maximum_steps":1000000,
    "inputs":[{"port":"seed","canonical_bytes":input}],
    "endpoint_read":{"address":129,"transfer_type":2,"maximum_packet":64,"interval":0,"endpoint_epoch":1,"maximum_operations":16},
    "endpoint_write":{"address":2,"transfer_type":2,"maximum_packet":64,"interval":0,"endpoint_epoch":1,"maximum_operations":16}
    });
    std::fs::write(directory.join("package.json"), bytes).unwrap();
    std::fs::write(directory.join("source.conduit"), &package.source).unwrap();
    std::fs::write(
        directory.join("root.json"),
        serde_json::to_vec_pretty(&root).unwrap(),
    )
    .unwrap();
    println!(
        "selected CDC package {hash}; entry cdc-serial-duplex-demonstration; output observation; TX12 RX64; 16 pairs"
    );
}
