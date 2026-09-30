//! Product-readiness truth, deliberately distinct from A0-A4 architecture conformance.

use serde::Serialize;

use crate::cli::GlobalOpts;

use super::{ConduitosArch, ConduitosError};

const SCHEMA: &str = "conduit.conduitos/product-readiness-matrix@1";

#[derive(Debug, Serialize)]
struct ProductRow {
    architecture: &'static str,
    profile_built_artifact: bool,
    bootable_image_binding: bool,
    runtime_image_bound_host_offer: bool,
    long_lived_product_host: bool,
    zero_body_front_door: bool,
    interactive_local_control: bool,
    ordinary_body_lifecycle: bool,
    ordinary_plan_play_from_product: bool,
    presenter: &'static str,
    proof_class: &'static str,
    blocker: Option<&'static str>,
}

#[derive(Debug, Serialize)]
struct ProductMatrix {
    schema: &'static str,
    basis: &'static str,
    architectures: Vec<ProductRow>,
}

pub fn execute(opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if opts.dry_run {
        return Err(ConduitosError::refusal(
            "dry-run-has-no-product-matrix",
            "product readiness reports checked executable surfaces, not planned commands",
        ));
    }
    let matrix = matrix();
    println!(
        "{}",
        serde_json::to_string_pretty(&matrix).map_err(|error| {
            ConduitosError::refusal("product-matrix-encoding-failed", error.to_string())
        })?
    );
    Ok(())
}

fn matrix() -> ProductMatrix {
    ProductMatrix {
        schema: SCHEMA,
        basis: "checked PROFILE lowerers plus ordinary product entrances; A0-A4 excluded",
        architectures: ConduitosArch::ALL.into_iter().map(row).collect(),
    }
}

fn row(arch: ConduitosArch) -> ProductRow {
    match arch {
        ConduitosArch::X86_64 => ProductRow {
            architecture: "x86_64",
            profile_built_artifact: true,
            bootable_image_binding: true,
            runtime_image_bound_host_offer: true,
            long_lived_product_host: true,
            zero_body_front_door: true,
            interactive_local_control: true,
            ordinary_body_lifecycle: true,
            ordinary_plan_play_from_product: true,
            presenter: "presenter/native-graphical@1",
            proof_class: "freestanding-emulator-native-product",
            blocker: None,
        },
        ConduitosArch::Aarch64 => ProductRow {
            architecture: "aarch64",
            profile_built_artifact: true,
            bootable_image_binding: true,
            runtime_image_bound_host_offer: true,
            long_lived_product_host: true,
            zero_body_front_door: true,
            interactive_local_control: false,
            ordinary_body_lifecycle: false,
            ordinary_plan_play_from_product: true,
            presenter: "presenter/linear-serial@1",
            proof_class: "freestanding-emulator-linear-product",
            blocker: Some("aarch64-local-input-base-unavailable"),
        },
        ConduitosArch::Ia32 => linear_product(
            "ia32",
            "presenter/ia32-linear-debugcon@1",
            "freestanding-emulator-linear-product",
            "ia32-local-input-base-unavailable",
        ),
        ConduitosArch::Armv6 => absent("armv6", "armv6-rpi-b-plus-physical-boot-unproven"),
        ConduitosArch::Riscv64 => linear_product(
            "riscv64",
            "presenter/riscv64-linear-sbi-console@1",
            "freestanding-emulator-linear-product",
            "riscv64-local-input-base-unavailable",
        ),
        ConduitosArch::Loongarch64 => linear_product(
            "loongarch64",
            "presenter/loongarch64-linear-uart@1",
            "freestanding-emulator-linear-product",
            "loongarch64-local-input-base-unavailable",
        ),
    }
}

fn linear_product(
    architecture: &'static str,
    presenter: &'static str,
    proof_class: &'static str,
    blocker: &'static str,
) -> ProductRow {
    ProductRow {
        architecture,
        profile_built_artifact: true,
        bootable_image_binding: true,
        runtime_image_bound_host_offer: true,
        long_lived_product_host: true,
        zero_body_front_door: true,
        interactive_local_control: false,
        ordinary_body_lifecycle: false,
        ordinary_plan_play_from_product: true,
        presenter,
        proof_class,
        blocker: Some(blocker),
    }
}

fn absent(architecture: &'static str, blocker: &'static str) -> ProductRow {
    ProductRow {
        architecture,
        profile_built_artifact: false,
        bootable_image_binding: false,
        runtime_image_bound_host_offer: false,
        long_lived_product_host: false,
        zero_body_front_door: false,
        interactive_local_control: false,
        ordinary_body_lifecycle: false,
        ordinary_plan_play_from_product: false,
        presenter: "none",
        proof_class: "none",
        blocker: Some(blocker),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn product_matrix_keeps_live_products_and_proof_appliances_distinct() {
        let matrix = matrix();
        let aarch64 = matrix
            .architectures
            .iter()
            .find(|row| row.architecture == "aarch64")
            .unwrap();
        assert!(aarch64.profile_built_artifact);
        assert!(aarch64.ordinary_plan_play_from_product);
        assert!(!aarch64.interactive_local_control);
        assert!(!aarch64.ordinary_body_lifecycle);
        assert_eq!(aarch64.presenter, "presenter/linear-serial@1");
        for architecture in ["ia32", "riscv64", "loongarch64"] {
            let row = matrix
                .architectures
                .iter()
                .find(|row| row.architecture == architecture)
                .unwrap();
            assert!(row.profile_built_artifact);
            assert!(row.long_lived_product_host);
            assert!(!row.interactive_local_control);
        }
        let armv6 = matrix
            .architectures
            .iter()
            .find(|row| row.architecture == "armv6")
            .unwrap();
        assert!(!armv6.profile_built_artifact);
        assert_eq!(matrix.architectures.len(), ConduitosArch::ALL.len());
    }
}
