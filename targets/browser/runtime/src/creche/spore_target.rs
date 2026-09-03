//! Exact fabrication-package selection for one Crèche physical Host spore.

use std::collections::BTreeMap;

use conduit_body_fabrication::{
    check_body_description, BodyBindingTarget, BodyDescription, BodyHostDescription,
    DeploymentDescription, SporeDescription, SporeJoinMode,
};
use conduit_host_avr_fabrication::{
    AvrProMicroFabricationPackage, FABRICATION_DESCRIPTOR as AVR_DESCRIPTOR,
    PACKAGE_ID as AVR_PACKAGE_ID, TARGET_ID as AVR_TARGET_ID,
};
use conduit_host_browser_fabrication::BrowserFabricationPackage;
use conduit_host_conduitos_fabrication::ConduitOsFabricationPackage;
use conduit_host_esp32_fabrication::{
    esp32_descriptor_binding, Esp32FabricationPackage, Esp32FamilyTarget,
};
use conduit_host_fabrication::{
    ConfigurationBase, ConfigurationTarget, FabricationCatalog, FabricationPackageSet, HostBounds,
    HostConfiguration, HostFabricationPackage, SporeOutputKind,
};
use conduit_host_hosted::{
    HostedFabricationPackage, HOSTED_MACOS_AARCH64_TARGET_ID, HOSTED_TARGET_ID,
    HOSTED_WINDOWS_X86_64_TARGET_ID,
};
use conduit_host_orange_pi::{
    OrangePiFabricationPackage, ORANGE_PI_5_TARGET, PACKAGE_ID as ORANGE_PI_PACKAGE_ID,
};
use conduit_host_raspberry_pi::{
    RaspberryPiFabricationPackage, B_PLUS_TARGET, RASPBERRY_PI_OS_TARGET, ZERO_2_WH_TARGET,
    ZERO_2_W_TARGET, ZERO_TARGET, ZERO_WH_TARGET, ZERO_W_TARGET,
};
use conduit_host_rp2040::Rp2040FabricationPackage;

pub(super) const PICO_W_TARGET_ID: &str = "conduitos/thumbv6m/pico-w";
pub(super) const STD_COMPUTER_TARGET_ID: &str = HOSTED_TARGET_ID;
pub(super) const BROWSER_PAGE_TARGET_ID: &str = "browser/wasm32/page";
pub(super) const CONDUITOS_X86_64_TARGET_ID: &str = "conduitos/x86_64/pc";
pub(super) const CONDUITOS_AARCH64_TARGET_ID: &str = "conduitos/aarch64/virt";
pub(super) const CONDUITOS_IA32_TARGET_ID: &str = "conduitos/ia32/pc";
pub(super) const CONDUITOS_RISCV64_TARGET_ID: &str = "conduitos/riscv64/virt";
pub(super) const CONDUITOS_LOONGARCH64_TARGET_ID: &str = "conduitos/loongarch64/virt";

pub(super) struct PreparedTarget {
    pub(super) body: conduit_body_fabrication::CheckedBodyDescription,
    pub(super) configuration: conduit_host_fabrication::CheckedHostConfiguration,
    pub(super) packages: FabricationPackageSet,
    pub(super) output: SporeOutputKind,
    pub(super) host_name: &'static str,
    pub(super) source_identity: &'static str,
}

struct TargetFacts {
    configuration_name: &'static str,
    host_name: &'static str,
    source_identity: &'static str,
    deployment_destination: Option<&'static str>,
    output: SporeOutputKind,
    configuration: HostConfiguration,
    packages: FabricationPackageSet,
}

pub(super) fn prepare(
    body_id: &str,
    invitation_id: &str,
    target_id: &str,
) -> Result<PreparedTarget, String> {
    prepare_with_checked_configuration(body_id, invitation_id, target_id, None)
}

pub(super) fn prepare_browser(
    body_id: &str,
    invitation_id: &str,
    checked: conduit_host_fabrication::CheckedHostConfiguration,
) -> Result<PreparedTarget, String> {
    prepare_with_checked_configuration(
        body_id,
        invitation_id,
        BROWSER_PAGE_TARGET_ID,
        Some(checked),
    )
}

fn prepare_with_checked_configuration(
    body_id: &str,
    invitation_id: &str,
    target_id: &str,
    checked_browser: Option<conduit_host_fabrication::CheckedHostConfiguration>,
) -> Result<PreparedTarget, String> {
    let mut target = target_facts(target_id)?;
    let expected_configuration_id = checked_browser
        .as_ref()
        .map(|checked| checked.configuration_id().to_owned());
    if let Some(checked) = checked_browser {
        if target_id != BROWSER_PAGE_TARGET_ID {
            return Err("checked browser configuration cannot select another target".into());
        }
        target.configuration = checked.configuration().clone();
    }
    let mut configurations = BTreeMap::new();
    configurations.insert(target.configuration_name.into(), target.configuration);
    let catalog = FabricationCatalog::canonical().with_packages(&target.packages);
    let body = check_body_description(
        BodyDescription {
            schema: 1,
            name: "Crèche physical Host".into(),
            body: BodyBindingTarget { id: body_id.into() },
            hosts: vec![BodyHostDescription {
                name: target.host_name.into(),
                part: None,
                configuration: target.configuration_name.into(),
                spore: SporeDescription {
                    join_mode: SporeJoinMode::SelfJoining,
                    output: target.output.clone(),
                    invitation: Some(invitation_id.into()),
                },
                deployment: target.deployment_destination.map(|destination| {
                    DeploymentDescription {
                        destination: destination.into(),
                    }
                }),
            }],
        },
        &configurations,
        &catalog,
        &target.packages,
    )
    .map_err(|errors| format!("check physical Host description: {errors:?}"))?;
    if expected_configuration_id
        .as_deref()
        .is_some_and(|expected| body.hosts()[0].configuration.configuration_id() != expected)
    {
        return Err(
            "browser fabrication did not consume the reviewed configuration identity".into(),
        );
    }
    Ok(PreparedTarget {
        configuration: body.hosts()[0].configuration.clone(),
        body,
        packages: target.packages,
        output: target.output,
        host_name: target.host_name,
        source_identity: target.source_identity,
    })
}

fn target_facts(target_id: &str) -> Result<TargetFacts, String> {
    match target_id {
        STD_COMPUTER_TARGET_ID
        | HOSTED_WINDOWS_X86_64_TARGET_ID
        | HOSTED_MACOS_AARCH64_TARGET_ID => return hosted_target(target_id),
        BROWSER_PAGE_TARGET_ID => return browser_target(),
        _ => {}
    }
    if target_id == PICO_W_TARGET_ID {
        return pico_target();
    }
    if target_id == AVR_TARGET_ID {
        return avr_target();
    }
    if target_id == ORANGE_PI_5_TARGET {
        return orange_pi_target();
    }
    if matches!(
        target_id,
        RASPBERRY_PI_OS_TARGET
            | ZERO_2_W_TARGET
            | ZERO_2_WH_TARGET
            | B_PLUS_TARGET
            | ZERO_TARGET
            | ZERO_W_TARGET
            | ZERO_WH_TARGET
    ) {
        return raspberry_pi_target(target_id);
    }
    if matches!(
        target_id,
        CONDUITOS_X86_64_TARGET_ID
            | CONDUITOS_AARCH64_TARGET_ID
            | CONDUITOS_IA32_TARGET_ID
            | CONDUITOS_RISCV64_TARGET_ID
            | CONDUITOS_LOONGARCH64_TARGET_ID
    ) {
        return conduitos_target(target_id);
    }
    let family = Esp32FamilyTarget::ALL
        .into_iter()
        .find(|candidate| candidate.target_descriptor().key() == target_id)
        .ok_or_else(|| format!("unsupported exact Crèche physical Host target {target_id:?}"))?;
    esp32_target(family)
}

fn orange_pi_target() -> Result<TargetFacts, String> {
    let package = OrangePiFabricationPackage;
    let conduit_host_fabrication::FabricationContribution::Anchor(anchor) = package.contribution()
    else {
        return Err("Orange Pi fabrication package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .find(|target| target.key() == ORANGE_PI_5_TARGET)
        .ok_or_else(|| "Orange Pi package omitted the exact Orange Pi 5 target".to_string())?;
    let configuration_name = "creche-conduitos-orange-pi-5-rk3588s";
    Ok(TargetFacts {
        configuration_name,
        host_name: configuration_name,
        source_identity: "conduitos/reviewed-orange-pi-5-rk3588s-sd-image@1",
        deployment_destination: Some("operator/removable-sd-writer"),
        output: descriptor.default_output,
        configuration: HostConfiguration {
            schema: 1,
            name: configuration_name.into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture,
                machine: descriptor.machine,
                board: descriptor.board,
                os: descriptor.os,
                fabrication_descriptor: None,
            },
            bases: vec![ConfigurationBase {
                kind: "serial/text".into(),
                implementation: Some("orange-pi/dw-apb-uart2@1".into()),
                implementations: Vec::new(),
            }],
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: FabricationPackageSet::compose(&[&OrangePiFabricationPackage])
            .map_err(|error| format!("compose {ORANGE_PI_PACKAGE_ID}: {error:?}"))?,
    })
}

fn conduitos_target(target_id: &str) -> Result<TargetFacts, String> {
    let package = ConduitOsFabricationPackage;
    let conduit_host_fabrication::FabricationContribution::Anchor(anchor) = package.contribution()
    else {
        return Err("ConduitOS fabrication package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .find(|target| target.key() == target_id)
        .ok_or_else(|| format!("ConduitOS package omitted exact product target {target_id:?}"))?;
    let (configuration_name, source_identity, serial_base) = match target_id {
        CONDUITOS_X86_64_TARGET_ID => (
            "creche-conduitos-x86-64-pc",
            "conduitos/reviewed-x86-64-pc-release@1",
            None,
        ),
        CONDUITOS_AARCH64_TARGET_ID => (
            "creche-conduitos-aarch64-virt",
            "conduitos/reviewed-aarch64-virt-release@1",
            Some(("serial/text", "conduitos/pl011@1")),
        ),
        CONDUITOS_IA32_TARGET_ID => (
            "creche-conduitos-ia32-pc",
            "conduitos/reviewed-ia32-pc-release@1",
            Some(("conduitos/ia32-debugcon-text", "conduitos/ia32-debugcon@1")),
        ),
        CONDUITOS_RISCV64_TARGET_ID => (
            "creche-conduitos-riscv64-virt",
            "conduitos/reviewed-riscv64-virt-release@1",
            Some((
                "conduitos/riscv64-sbi-console-text",
                "conduitos/riscv64-sbi-console@1",
            )),
        ),
        CONDUITOS_LOONGARCH64_TARGET_ID => (
            "creche-conduitos-loongarch64-virt",
            "conduitos/reviewed-loongarch64-virt-release@1",
            Some((
                "conduitos/loongarch64-uart-text",
                "conduitos/loongarch64-uart@1",
            )),
        ),
        _ => {
            return Err(format!(
                "unsupported ConduitOS product target {target_id:?}"
            ))
        }
    };
    Ok(TargetFacts {
        configuration_name,
        host_name: configuration_name,
        source_identity,
        deployment_destination: Some("operator/local-disk-or-vm-loader"),
        output: SporeOutputKind::DiskImage,
        configuration: HostConfiguration {
            schema: 1,
            name: configuration_name.into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture,
                machine: descriptor.machine,
                board: descriptor.board,
                os: descriptor.os,
                fabrication_descriptor: None,
            },
            bases: serial_base
                .map(|(kind, implementation)| {
                    vec![ConfigurationBase {
                        kind: kind.into(),
                        implementation: Some(implementation.into()),
                        implementations: Vec::new(),
                    }]
                })
                .unwrap_or_default(),
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: FabricationPackageSet::compose(&[&ConduitOsFabricationPackage])
            .map_err(|error| format!("compose ConduitOS fabrication package: {error:?}"))?,
    })
}

fn raspberry_pi_target(target_id: &str) -> Result<TargetFacts, String> {
    let package = RaspberryPiFabricationPackage;
    let conduit_host_fabrication::FabricationContribution::Anchor(anchor) = package.contribution()
    else {
        return Err("Raspberry Pi fabrication package is not an anchor".into());
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
                fabrication_descriptor: None,
            },
            bases: vec![ConfigurationBase {
                kind: "serial/text".into(),
                implementation: Some(if is_os_target {
                    "raspberry-pi-os/serial@1".into()
                } else {
                    "raspberry-pi/pl011@1".into()
                }),
                implementations: Vec::new(),
            }],
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: FabricationPackageSet::compose(&[&RaspberryPiFabricationPackage])
            .map_err(|error| format!("compose Raspberry Pi fabrication package: {error:?}"))?,
    })
}

fn avr_target() -> Result<TargetFacts, String> {
    let package = AvrProMicroFabricationPackage;
    let conduit_host_fabrication::FabricationContribution::Anchor(anchor) = package.contribution()
    else {
        return Err("AVR Pro Micro fabrication package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .find(|target| target.key() == AVR_TARGET_ID)
        .ok_or_else(|| "AVR Pro Micro package omitted its exact target".to_string())?;
    Ok(TargetFacts {
        configuration_name: "creche-avr-promicro-prebuilt",
        host_name: "creche-avr-promicro",
        source_identity: "conduit-avr-promicro/reviewed-intel-hex@1",
        deployment_destination: None,
        output: SporeOutputKind::IntelHex,
        configuration: HostConfiguration {
            schema: 1,
            name: "creche-avr-promicro-prebuilt".into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture,
                machine: descriptor.machine,
                board: descriptor.board,
                os: None,
                fabrication_descriptor: Some(AVR_DESCRIPTOR.into()),
            },
            bases: Vec::new(),
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: FabricationPackageSet::compose(&[&AvrProMicroFabricationPackage])
            .map_err(|error| format!("compose {AVR_PACKAGE_ID}: {error:?}"))?,
    })
}

fn hosted_target(target_id: &str) -> Result<TargetFacts, String> {
    let (configuration_name, source_identity) = match target_id {
        STD_COMPUTER_TARGET_ID => (
            "creche-hosted-linux-x86-64",
            "conduit/reviewed-hosted-linux-release@1",
        ),
        HOSTED_WINDOWS_X86_64_TARGET_ID => (
            "creche-hosted-windows-x86-64",
            "conduit/reviewed-hosted-windows-release@1",
        ),
        HOSTED_MACOS_AARCH64_TARGET_ID => (
            "creche-hosted-macos-aarch64",
            "conduit/reviewed-hosted-macos-release@1",
        ),
        _ => return Err(format!("unsupported hosted target {target_id:?}")),
    };
    let package = HostedFabricationPackage;
    let conduit_host_fabrication::FabricationContribution::Anchor(anchor) = package.contribution()
    else {
        return Err("hosted fabrication package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .find(|target| target.key() == target_id)
        .ok_or_else(|| format!("hosted fabrication package omitted exact target {target_id:?}"))?;
    Ok(TargetFacts {
        configuration_name,
        host_name: configuration_name,
        source_identity,
        deployment_destination: Some("operator/local-download"),
        output: SporeOutputKind::NativeBundle,
        configuration: HostConfiguration {
            schema: 1,
            name: configuration_name.into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture,
                machine: descriptor.machine,
                board: descriptor.board,
                os: descriptor.os,
                fabrication_descriptor: None,
            },
            bases: vec![
                ConfigurationBase {
                    kind: "clock/monotonic".into(),
                    implementation: Some("hosted/monotonic-clock@1".into()),
                    implementations: Vec::new(),
                },
                ConfigurationBase {
                    kind: "serial/text".into(),
                    implementation: Some("hosted/serial@1".into()),
                    implementations: Vec::new(),
                },
            ],
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: FabricationPackageSet::compose(&[&HostedFabricationPackage])
            .map_err(|error| format!("compose hosted fabrication package: {error:?}"))?,
    })
}

fn browser_target() -> Result<TargetFacts, String> {
    let package = BrowserFabricationPackage;
    let conduit_host_fabrication::FabricationContribution::Anchor(anchor) = package.contribution()
    else {
        return Err("browser fabrication package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .next()
        .ok_or_else(|| "browser fabrication package omitted its page target".to_string())?;
    Ok(TargetFacts {
        configuration_name: "creche-browser-page",
        host_name: "creche-browser-page",
        source_identity: "conduit/reviewed-browser-host-release@1",
        deployment_destination: Some("browser/local-sandbox"),
        output: SporeOutputKind::BrowserBundle,
        configuration: HostConfiguration {
            schema: 1,
            name: "creche-browser-page".into(),
            target: ConfigurationTarget {
                architecture: descriptor.architecture,
                machine: descriptor.machine,
                board: descriptor.board,
                os: descriptor.os,
                fabrication_descriptor: None,
            },
            bases: conduit_host_browser_fabrication::default_configuration_bases(),
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: FabricationPackageSet::compose(&[&BrowserFabricationPackage])
            .map_err(|error| format!("compose browser fabrication package: {error:?}"))?,
    })
}

fn pico_target() -> Result<TargetFacts, String> {
    Ok(TargetFacts {
        configuration_name: "creche-pico-w-prebuilt",
        host_name: "creche-pico-w",
        source_identity: "conduit-pico-w-signal/pico-local-b8@1",
        deployment_destination: Some("browser/webusb"),
        output: SporeOutputKind::Uf2,
        configuration: HostConfiguration {
            schema: 1,
            name: "creche-pico-w-prebuilt".into(),
            target: ConfigurationTarget {
                architecture: "thumbv6m".into(),
                machine: "pico-w".into(),
                board: Some("pico-w".into()),
                os: None,
                fabrication_descriptor: None,
            },
            bases: vec![ConfigurationBase {
                kind: "serial/text".into(),
                implementation: Some("pico/usb-cdc@1".into()),
                implementations: Vec::new(),
            }],
            resources: Vec::new(),
            limits: HostBounds {
                static_memory_bytes: 256 * 1024,
                heap_arena_bytes: 1,
                queue_items: 16,
                buffered_bytes: 64 * 1024,
                active_instances: 16,
                operation_slots: 16,
                timer_slots: 16,
                line_sessions: 1,
                evidence_items: 64,
            },
        },
        packages: FabricationPackageSet::compose(&[&Rp2040FabricationPackage])
            .map_err(|error| format!("compose Pico fabrication package: {error:?}"))?,
    })
}

fn esp32_target(target: Esp32FamilyTarget) -> Result<TargetFacts, String> {
    let facts = target.facts();
    let descriptor = target.target_descriptor();
    let descriptor_binding = esp32_descriptor_binding(&target.board_descriptor())
        .map_err(|error| format!("bind ESP32 fabrication descriptor: {error:?}"))?;
    let configuration_name = match target {
        Esp32FamilyTarget::C3 => "creche-esp32-c3-prebuilt",
        Esp32FamilyTarget::S3 => "creche-esp32-s3-prebuilt",
        Esp32FamilyTarget::Wroom => "creche-esp32-wroom-prebuilt",
    };
    let source_identity = match target {
        Esp32FamilyTarget::C3 => "conduit-esp32-c3-signal/reviewed-browser-image@1",
        Esp32FamilyTarget::S3 => "conduit-esp32-s3-signal/reviewed-browser-image@1",
        Esp32FamilyTarget::Wroom => "conduit-esp32-wroom-signal/reviewed-browser-image@1",
    };
    Ok(TargetFacts {
        configuration_name,
        host_name: configuration_name,
        source_identity,
        deployment_destination: Some("browser/webserial"),
        output: SporeOutputKind::Esp32Image,
        configuration: HostConfiguration {
            schema: 1,
            name: configuration_name.into(),
            target: ConfigurationTarget {
                architecture: facts.architecture.into(),
                machine: facts.machine.into(),
                board: Some(facts.machine.into()),
                os: None,
                fabrication_descriptor: Some(descriptor_binding),
            },
            bases: vec![
                ConfigurationBase {
                    kind: "kernel/signal".into(),
                    implementation: Some("esp32/kernel-signal@1".into()),
                    implementations: Vec::new(),
                },
                ConfigurationBase {
                    kind: "line/bluetooth-le-gatt".into(),
                    implementation: Some("esp32/bluetooth-le-gatt@1".into()),
                    implementations: Vec::new(),
                },
            ],
            resources: Vec::new(),
            limits: descriptor.maxima,
        },
        packages: FabricationPackageSet::compose(&[&Esp32FabricationPackage])
            .map_err(|error| format!("compose ESP32 fabrication package: {error:?}"))?,
    })
}

#[cfg(test)]
#[path = "spore_target_tests.rs"]
mod tests;
