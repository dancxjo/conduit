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
use conduit_host_hosted::HostedFabricationPackage;
use conduit_host_raspberry_pi::{
    RaspberryPiFabricationPackage, B_PLUS_TARGET, RASPBERRY_PI_OS_TARGET,
};
use conduit_host_rp2040::Rp2040FabricationPackage;

pub(super) const PICO_W_TARGET_ID: &str = "conduitos/thumbv6m/pico-w";
pub(super) const STD_WORKSTATION_TARGET_ID: &str = "std/x86_64/workstation";
pub(super) const STD_SERVER_TARGET_ID: &str = "std/x86_64/server";
pub(super) const BROWSER_PAGE_TARGET_ID: &str = "browser/wasm32/page";
pub(super) const CONDUITOS_X86_64_TARGET_ID: &str = "conduitos/x86_64/pc";
pub(super) const CONDUITOS_AARCH64_TARGET_ID: &str = "conduitos/aarch64/virt";

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
    let target = target_facts(target_id)?;
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
        STD_WORKSTATION_TARGET_ID => return hosted_target("workstation"),
        STD_SERVER_TARGET_ID => return hosted_target("server"),
        BROWSER_PAGE_TARGET_ID => return browser_target(),
        _ => {}
    }
    if target_id == PICO_W_TARGET_ID {
        return pico_target();
    }
    if target_id == AVR_TARGET_ID {
        return avr_target();
    }
    if matches!(target_id, RASPBERRY_PI_OS_TARGET | B_PLUS_TARGET) {
        return raspberry_pi_target(target_id);
    }
    if matches!(
        target_id,
        CONDUITOS_X86_64_TARGET_ID | CONDUITOS_AARCH64_TARGET_ID
    ) {
        return conduitos_target(target_id);
    }
    let family = Esp32FamilyTarget::ALL
        .into_iter()
        .find(|candidate| candidate.target_descriptor().key() == target_id)
        .ok_or_else(|| format!("unsupported exact Crèche physical Host target {target_id:?}"))?;
    esp32_target(family)
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
    let (configuration_name, source_identity) = if target_id == CONDUITOS_X86_64_TARGET_ID {
        (
            "creche-conduitos-x86-64-pc",
            "conduitos/reviewed-x86-64-pc-release@1",
        )
    } else {
        (
            "creche-conduitos-aarch64-virt",
            "conduitos/reviewed-aarch64-virt-release@1",
        )
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
            bases: if target_id == CONDUITOS_AARCH64_TARGET_ID {
                vec![ConfigurationBase {
                    kind: "serial/text".into(),
                    implementation: Some("conduitos/pl011@1".into()),
                    implementations: Vec::new(),
                }]
            } else {
                Vec::new()
            },
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
    let (configuration_name, host_name, source_identity, deployment_destination) =
        if target_id == RASPBERRY_PI_OS_TARGET {
            (
                "creche-raspberry-pi-os-pi4",
                "creche-raspberry-pi-os-pi4",
                "conduit/reviewed-raspberry-pi-os-aarch64-release@1",
                Some("operator/local-package-installer"),
            )
        } else {
            (
                "creche-conduitos-rpi-b-plus",
                "creche-conduitos-rpi-b-plus",
                "conduitos/reviewed-armv6-rpi-b-plus-sd-image@1",
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
                implementation: Some(if target_id == RASPBERRY_PI_OS_TARGET {
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

fn hosted_target(machine: &'static str) -> Result<TargetFacts, String> {
    let configuration_name = match machine {
        "workstation" => "creche-hosted-linux-workstation",
        "server" => "creche-hosted-linux-server",
        _ => return Err(format!("unsupported hosted machine {machine:?}")),
    };
    let package = HostedFabricationPackage;
    let conduit_host_fabrication::FabricationContribution::Anchor(anchor) = package.contribution()
    else {
        return Err("hosted fabrication package is not an anchor".into());
    };
    let descriptor = anchor
        .targets
        .into_iter()
        .find(|target| target.machine == machine)
        .ok_or_else(|| format!("hosted fabrication package omitted {machine:?}"))?;
    Ok(TargetFacts {
        configuration_name,
        host_name: configuration_name,
        source_identity: "conduit/reviewed-hosted-linux-release@1",
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
            bases: vec![ConfigurationBase {
                kind: "browser/dom".into(),
                implementation: Some("browser/dom@1".into()),
                implementations: Vec::new(),
            }],
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
        source_identity: "conduit-pico-w-signal/pico-local-b7@1",
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
mod tests {
    use super::*;

    #[test]
    fn existing_computer_targets_are_exact_package_owned_outputs() {
        let body_id = "a".repeat(64);
        for (target_id, architecture, machine, output) in [
            (
                STD_WORKSTATION_TARGET_ID,
                "x86_64",
                "workstation",
                SporeOutputKind::NativeBundle,
            ),
            (
                STD_SERVER_TARGET_ID,
                "x86_64",
                "server",
                SporeOutputKind::NativeBundle,
            ),
            (
                BROWSER_PAGE_TARGET_ID,
                "wasm32",
                "page",
                SporeOutputKind::BrowserBundle,
            ),
        ] {
            let prepared = prepare(&body_id, "invitation/existing", target_id).unwrap();
            let profile = prepared.configuration.profile();
            assert_eq!(profile.target.architecture, architecture);
            assert_eq!(profile.target.machine, machine);
            assert_eq!(prepared.output, output);
        }
    }

    #[test]
    fn conduitos_creche_targets_are_only_earned_product_hosts() {
        let body_id = "a".repeat(64);
        for (target_id, architecture, machine) in [
            (CONDUITOS_X86_64_TARGET_ID, "x86_64", "pc"),
            (CONDUITOS_AARCH64_TARGET_ID, "aarch64", "virt"),
        ] {
            let prepared = prepare(&body_id, "invitation/conduitos", target_id).unwrap();
            let profile = prepared.configuration.profile();
            assert_eq!(profile.target.architecture, architecture);
            assert_eq!(profile.target.machine, machine);
            assert_eq!(prepared.output, SporeOutputKind::DiskImage);
        }
        for proof_only in [
            "conduitos/ia32/pc",
            "conduitos/riscv64/virt",
            "conduitos/loongarch64/virt",
        ] {
            assert!(prepare(&body_id, "invitation/conduitos", proof_only).is_err());
        }
    }

    #[test]
    fn broad_or_unknown_existing_computer_targets_are_not_inferred() {
        let body_id = "b".repeat(64);
        for target in ["std/*/*", "std/aarch64/server", "browser/wasm32/worker"] {
            let error = match prepare(&body_id, "invitation/existing", target) {
                Ok(_) => panic!("broad target {target:?} was inferred"),
                Err(error) => error,
            };
            assert!(error.contains("unsupported exact"));
        }
    }

    #[test]
    fn exact_pro_micro_retains_intel_hex_and_package_identity() {
        let prepared = prepare(&"c".repeat(64), "invitation/avr", AVR_TARGET_ID).unwrap();
        assert_eq!(prepared.configuration.profile().target.key(), AVR_TARGET_ID);
        assert_eq!(prepared.output, SporeOutputKind::IntelHex);
        assert_eq!(
            prepared
                .packages
                .anchor_for_target(AVR_TARGET_ID)
                .unwrap()
                .package_id,
            AVR_PACKAGE_ID
        );
    }

    #[test]
    fn raspberry_pi_os_and_bare_metal_are_distinct_exact_targets() {
        let body_id = "d".repeat(64);
        let os = prepare(&body_id, "invitation/pi-os", RASPBERRY_PI_OS_TARGET).unwrap();
        let bare = prepare(&body_id, "invitation/pi-bare", B_PLUS_TARGET).unwrap();
        assert_eq!(os.output, SporeOutputKind::NativeBundle);
        assert_eq!(
            os.configuration.profile().target.key(),
            RASPBERRY_PI_OS_TARGET
        );
        assert_eq!(bare.output, SporeOutputKind::SdImage);
        assert_eq!(bare.configuration.profile().target.key(), B_PLUS_TARGET);
        for unsupported in [
            "std/aarch64/raspberry-pi-5",
            "conduitos/armv7/raspberry-pi-3",
            "conduitos/armv6/raspberry-pi-b-plus",
        ] {
            let error = match prepare(&body_id, "invitation/pi", unsupported) {
                Ok(_) => panic!("unsupported Raspberry Pi target was inferred"),
                Err(error) => error,
            };
            assert!(error.contains("unsupported exact"));
        }
    }
}
