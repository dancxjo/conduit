//! Exact Raspberry Pi target and package composition for Crèche.
use super::*;

pub(super) fn raspberry_pi_target(target_id: &str) -> Result<TargetFacts, String> {
    let package = RaspberryPiMakePackage;
    let conduit_host_make::MakeContribution::Anchor(anchor) = package.contribution() else {
        return Err("Raspberry Pi make package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .find(|target| target.key() == target_id)
        .ok_or_else(|| format!("Raspberry Pi package omitted exact target {target_id:?}"))?;
    let is_os_target = matches!(
        target_id,
        RASPBERRY_PI_OS_TARGET | ZERO_2_W_TARGET | ZERO_2_WH_TARGET
    );
    let (configuration_name, host_name, source_identity, deployment_destination) = if is_os_target {
        (
            "creche-raspberry-pi-os-aarch64",
            "creche-raspberry-pi-os-aarch64",
            "conduit/reviewed-raspberry-pi-os-aarch64-release@1",
            Some("operator/local-package-installer"),
        )
    } else {
        (
            "creche-conduitos-rpi-armv6",
            "creche-conduitos-rpi-armv6",
            "conduitos/reviewed-armv6-rpi-sd-image@1",
            Some("operator/local-removable-media-writer"),
        )
    };
    Ok(TargetFacts {
        configuration_name,
        host_name,
        source_identity,
        deployment_destination,
        output: descriptor.default_output,
        configuration: HostConfiguration {
            schema: 1,
            name: configuration_name.into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture,
                machine: descriptor.machine,
                board: descriptor.board,
                os: descriptor.os,
                make_descriptor: None,
            },
            bases: vec![ConfigurationBase {
                kind: "serial/text".into(),
                implementation: Some(if is_os_target {
                    "raspberry-pi-os/serial@1".into()
                } else {
                    "raspberry-pi/pl011@1".into()
                }),
                implementations: Vec::new(),
                initialization: None,
            }],
            capabilities: Vec::new(),
            preparation: None,
            execution: None,
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        // The B+ product's serial presenter is owned by ConduitOS; the board
        // anchor owns its exact target and PL011 implementation.
        packages: MakePackageSet::compose(if target_id == B_PLUS_TARGET {
            &[&RaspberryPiMakePackage, &ConduitOsMakePackage]
        } else {
            &[&RaspberryPiMakePackage]
        })
        .map_err(|error| format!("compose Raspberry Pi make package: {error:?}"))?,
    })
}
