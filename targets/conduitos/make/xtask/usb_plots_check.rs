//! Deterministic protocol-plot checks, distinct from native USB execution.
use std::process::Command;

use super::ConduitosError;
use crate::cli::GlobalOpts;

pub fn execute(cross: bool, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    let checks: &[&[&str]] = &[
        &["test", "--locked", "-p", "conduit-core"],
        &[
            "test",
            "--locked",
            "-p",
            "conduit-composite",
            "--lib",
            "flow_concat_finite",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduit-semantic-catalog",
            "--lib",
            "flow_concat_finite",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--lib",
            "flow_concat_finite",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--lib",
            "protocol_source::planning::tests",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduit-plot",
            "--test",
            "integer_widening",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduit-plot",
            "--test",
            "prepared_structured_payload",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--test",
            "usb_protocol_plots",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--test",
            "usb_hid_reports",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--lib",
            "machine_membrane",
        ],
        &["test", "--locked", "-p", "conduitos", "--lib", "usb_base"],
        &[
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--test",
            "usb_endpoint_read",
        ],
        &[
            "test",
            "--locked",
            "-p",
            "conduitos",
            "--lib",
            "arch::x86_64::usb::endpoint_",
        ],
    ];
    for arguments in checks {
        if opts.dry_run {
            println!("cargo {}", arguments.join(" "));
            continue;
        }
        let status = Command::new("cargo")
            .args(*arguments)
            .status()
            .map_err(|error| ConduitosError::refusal("usb-plots-check-start", error.to_string()))?;
        if !status.success() {
            return Err(ConduitosError::refusal(
                "usb-plots-check-failed",
                format!("cargo {}: {status}", arguments.join(" ")),
            ));
        }
    }
    if cross {
        // IA-32 uses the installed UEFI compilation target here; this is a
        // shared-library type check, not an IA-32 firmware/media boot proof.
        for target in [
            "x86_64-unknown-none",
            "i686-unknown-uefi",
            "aarch64-unknown-none",
            "riscv64gc-unknown-none-elf",
            "loongarch64-unknown-none",
        ] {
            let arguments = [
                "check",
                "--locked",
                "-p",
                "conduitos",
                "--lib",
                "--target",
                target,
            ];
            if opts.dry_run {
                println!("cargo {}", arguments.join(" "));
                continue;
            }
            let status = Command::new("cargo")
                .args(arguments)
                .status()
                .map_err(|error| {
                    ConduitosError::refusal("usb-plots-cross-check-start", error.to_string())
                })?;
            if !status.success() {
                return Err(ConduitosError::refusal(
                    "usb-plots-cross-check-failed",
                    format!("{target}: {status}"),
                ));
            }
        }
    }
    println!("USB plot checks establish checked wire transformations and cooperative register-possession fixtures; no USB device execution or physical acceptance claim.");
    Ok(())
}
