//! Ordinary selected HT protocol adapters still check and emit exact MCU work.
use super::*;
use sha2::{Digest, Sha256};
#[test]
fn selected_ht_initialization_and_output_preserve_one_native_bus_per_entry() {
    let units = [
        include_str!("../../../../plots/device-protocols/ht16k33.conduit"),
        include_str!("../../../../plots/device-protocols/ht16k33-exchanges.conduit"),
        include_str!("../../../../plots/device-protocols/ht16k33-feedback.conduit"),
    ];
    let imports = units
        .iter()
        .flat_map(|u| u.lines())
        .filter(|line| line.starts_with("with "))
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>()
        .join("\n");
    let bodies = units
        .iter()
        .flat_map(|u| u.lines())
        .filter(|line| !line.starts_with("with "))
        .collect::<Vec<_>>()
        .join("\n");
    let source = format!("{imports}\n{bodies}");
    let reference = |type_name| ValueReference {
        type_name,
        maximum_bytes: 4096,
    };
    let requests = [
        Specialization::SeededUntil(reference("Ht16k33ProtocolFrame")),
        Specialization::Zip {
            left: reference("Ht16k33State"),
            right: reference("I2cResult"),
            feedback: true,
        },
        Specialization::Merge(reference("Ht16k33Failure")),
    ];
    let entries = ["ht16k33-initialize-display", "ht16k33-write-display"].map(String::from);
    let set = crate::workset::compile_specialized(
        source.as_bytes(),
        &format!("{:x}", Sha256::digest(source.as_bytes())),
        &entries,
        None,
        &requests,
    )
    .unwrap();
    for entry in &set.entries {
        assert_eq!(
            entry
                .plot
                .expanded
                .gears
                .iter()
                .filter(|g| g.kind_id.as_str() == conduit_i2c_transaction::contract::I2C_KIND)
                .count(),
            1
        );
        let material = crate::software::compile_with_types(
            &entry.plot.expanded,
            Some(&set.checked_source),
            64 * 1024,
        )
        .unwrap();
        assert!(material.nodes.iter().all(|node| node.offer_image.is_some()));
        assert!(entry.rust.contains("Static{"));
    }
}
