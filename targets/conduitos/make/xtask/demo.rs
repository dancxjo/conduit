//! Human-visible ConduitOS QEMU entrance.

use std::process::{Command, Stdio};

use crate::cli::GlobalOpts;

use super::{live_media, ConduitosArch, ConduitosError};

pub const DEMO_PROFILE: &str = "q35-single-cpu-64m-visible-gtk-xhci-usb-kbd-mouse";

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

    live_media::build(live_media::LiveHost::X86_64, opts)?;
    live_media::boot(live_media::LiveHost::X86_64, opts)
}

pub(crate) fn boot_visible_image(
    image: &std::path::Path,
    image_sha256: Option<&str>,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    boot_visible_image_with_network(image, image_sha256, None, opts)
}

pub(crate) fn boot_visible_image_with_network(
    image: &std::path::Path,
    image_sha256: Option<&str>,
    netdev: Option<&str>,
    opts: &GlobalOpts,
) -> Result<(), ConduitosError> {
    let args = qemu_args(
        image.to_str().ok_or_else(|| {
            ConduitosError::refusal("demo-image-path-invalid", "image path is not UTF-8")
        })?,
        netdev,
    );
    if opts.dry_run {
        println!("qemu-system-x86_64 {}", args.join(" "));
        return Ok(());
    }

    if !opts.quiet {
        println!("ConduitOS live system");
        println!("  Host type: conduitos/x86_64/pc");
        println!("  image: {}", image.display());
        if let Some(image_sha256) = image_sha256 {
            println!("  image-sha256: {image_sha256}");
        }
        println!("  profile: {DEMO_PROFILE}");
        if netdev.is_some() {
            println!("  network: explicit provisioned owner route over VirtIO-net");
        }
        println!("Close the QEMU window or press Ctrl-C to exit.");
    }

    let status = Command::new("qemu-system-x86_64")
        .args(&args)
        .current_dir(image.parent().unwrap_or_else(|| std::path::Path::new(".")))
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

fn qemu_args<'a>(iso: &'a str, netdev: Option<&'a str>) -> Vec<&'a str> {
    let mut args = vec![
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
    ];
    if let Some(netdev) = netdev {
        args.extend([
            "-netdev",
            netdev,
            "-device",
            "virtio-net-pci,netdev=conduit-owner,disable-modern=on,rx_queue_size=256,tx_queue_size=256",
        ]);
    } else {
        args.extend(["-net", "none"]);
    }
    args.extend([
        "-device",
        "qemu-xhci,id=conduitos-xhci,p2=2,p3=0",
        "-device",
        "usb-kbd,bus=conduitos-xhci.0,port=1",
        "-device",
        "usb-mouse,bus=conduitos-xhci.0,port=2",
        "-cdrom",
        iso,
        "-boot",
        "d",
    ]);
    args
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn visible_profile_keeps_the_accepted_machine_and_keyboard_shape() {
        let args = qemu_args("conduitos.iso", None);
        assert!(args.windows(2).any(|pair| pair == ["-display", "gtk"]));
        assert!(args.windows(2).any(|pair| pair == ["-serial", "stdio"]));
        assert!(args.windows(2).any(|pair| pair == ["-M", "q35"]));
        assert!(args.windows(2).any(|pair| pair == ["-m", "64M"]));
        assert!(args.contains(&"qemu-xhci,id=conduitos-xhci,p2=2,p3=0"));
        assert!(args.contains(&"usb-kbd,bus=conduitos-xhci.0,port=1"));
        assert!(args.contains(&"usb-mouse,bus=conduitos-xhci.0,port=2"));
        assert!(!args.contains(&"-no-shutdown"));
        assert!(!args.contains(&"isa-debug-exit,iobase=0xf4,iosize=0x04"));
    }

    #[test]
    fn owner_profile_attaches_virtio_without_altering_visible_product_boot() {
        let netdev =
            "user,id=conduit-owner,restrict=on,guestfwd=tcp:10.0.2.100:9000-tcp:172.17.0.1:19000";
        let args = qemu_args("spore.iso", Some(netdev));
        assert!(args.windows(2).any(|pair| pair == ["-netdev", netdev]));
        assert!(args.contains(&"virtio-net-pci,netdev=conduit-owner,disable-modern=on,rx_queue_size=256,tx_queue_size=256"));
        assert!(args.windows(2).any(|pair| pair == ["-display", "gtk"]));
        assert!(!args.windows(2).any(|pair| pair == ["-net", "none"]));
    }

    #[test]
    fn unsupported_visible_backends_refuse_before_building() {
        let error = execute(ConduitosArch::Aarch64, &GlobalOpts::default()).unwrap_err();
        assert_eq!(error.reason, "unsupported-visible-demo-architecture");
        assert!(error.detail.contains("use x86-64"));
    }
}
