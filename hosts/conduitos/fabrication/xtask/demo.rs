//! Human-visible ConduitOS QEMU entrance.

use std::process::{Command, Stdio};

use crate::cli::GlobalOpts;

use super::{image, profile::Paths, ConduitosArch, ConduitosError};

pub const DEMO_PROFILE: &str = "q35-single-cpu-64m-visible-gtk-xhci-usb-kbd";

pub fn execute(arch: ConduitosArch, opts: &GlobalOpts) -> Result<(), ConduitosError> {
    if arch != ConduitosArch::X86_64 {
        return Err(ConduitosError::refusal(
            "unsupported-visible-demo-architecture",
            format!(
                "{} has no accepted visible interactive ConduitOS demo; use x86-64",
                arch.as_str()
            ),
        ));
    }
    if opts.json {
        return Err(ConduitosError::refusal(
            "interactive-demo-json-unsupported",
            "the human-visible demo does not emit a terminal JSON report; use run/prove for evidence",
        ));
    }

    let paths = Paths::new(arch)?;
    let image = image::execute(arch, opts)?;
    let args = qemu_args(paths.iso.to_str().ok_or_else(|| {
        ConduitosError::refusal("demo-image-path-invalid", "image path is not UTF-8")
    })?);
    if opts.dry_run {
        println!("qemu-system-x86_64 {}", args.join(" "));
        return Ok(());
    }

    if !opts.quiet {
        println!("ConduitOS interactive demo");
        println!("  architecture: {}", arch.as_str());
        println!("  image: {}", paths.iso.display());
        println!("  image-sha256: {}", image.iso_sha256);
        println!("  profile: {DEMO_PROFILE}");
        println!("Close the QEMU window or press Ctrl-C to exit.");
    }

    let status = Command::new("qemu-system-x86_64")
        .args(&args)
        .current_dir(&paths.root)
        .stdin(Stdio::inherit())
        .stdout(Stdio::inherit())
        .stderr(Stdio::inherit())
        .status()
        .map_err(|error| {
            ConduitosError::refusal(
                "interactive-demo-qemu-unavailable",
                format!(
                    "cannot launch visible QEMU profile {DEMO_PROFILE}: {error}; install qemu-system-x86 and provide a graphical display"
                ),
            )
        })?;
    if !status.success() {
        return Err(ConduitosError::refusal(
            "interactive-demo-qemu-failed",
            format!(
                "visible QEMU profile {DEMO_PROFILE} exited {status}; verify graphical display access and QEMU GTK support"
            ),
        ));
    }
    Ok(())
}

fn qemu_args(iso: &str) -> Vec<&str> {
    vec![
        "-M",
        "q35",
        "-cpu",
        "max",
        "-m",
        "64M",
        "-smp",
        "1",
        "-display",
        "gtk",
        "-vga",
        "std",
        "-monitor",
        "none",
        "-serial",
        "stdio",
        "-no-reboot",
        "-net",
        "none",
        "-device",
        "qemu-xhci,id=conduitos-xhci,p2=1,p3=0",
        "-device",
        "usb-kbd,bus=conduitos-xhci.0,port=1",
        "-cdrom",
        iso,
        "-boot",
        "d",
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_profile_keeps_the_accepted_machine_and_keyboard_shape() {
        let args = qemu_args("conduitos.iso");
        assert!(args.windows(2).any(|pair| pair == ["-display", "gtk"]));
        assert!(args.windows(2).any(|pair| pair == ["-serial", "stdio"]));
        assert!(args.windows(2).any(|pair| pair == ["-M", "q35"]));
        assert!(args.windows(2).any(|pair| pair == ["-m", "64M"]));
        assert!(args.contains(&"qemu-xhci,id=conduitos-xhci,p2=1,p3=0"));
        assert!(args.contains(&"usb-kbd,bus=conduitos-xhci.0,port=1"));
        assert!(!args.contains(&"-no-shutdown"));
        assert!(!args.contains(&"isa-debug-exit,iobase=0xf4,iosize=0x04"));
    }

    #[test]
    fn unsupported_visible_backends_refuse_before_building() {
        let error = execute(ConduitosArch::Aarch64, &GlobalOpts::default()).unwrap_err();
        assert_eq!(error.reason, "unsupported-visible-demo-architecture");
        assert!(error.detail.contains("use x86-64"));
    }
}
