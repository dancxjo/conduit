//! Versioned proof vocabulary and fail-closed proof substitution policy.

use serde::{Deserialize, Serialize};

pub const PROOF_SCHEMA_VERSION: u16 = 1;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum ProofClass {
    ContractCompile,
    DeterministicUnit,
    DeterministicSimulation,
    HostedIntegration,
    FreestandingEmulator,
    LiveBrowser,
    LiveTransport,
    FirmwareBuild,
    PhysicalLocalHardware,
    PhysicalCrossHost,
    ManualObservation,
}

pub const PROOF_CLASS_VOCABULARY: &[ProofClass] = &[
    ProofClass::ContractCompile,
    ProofClass::DeterministicUnit,
    ProofClass::DeterministicSimulation,
    ProofClass::HostedIntegration,
    ProofClass::FreestandingEmulator,
    ProofClass::LiveBrowser,
    ProofClass::LiveTransport,
    ProofClass::FirmwareBuild,
    ProofClass::PhysicalLocalHardware,
    ProofClass::PhysicalCrossHost,
    ProofClass::ManualObservation,
];

impl ProofClass {
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::ContractCompile => "contract-compile",
            Self::DeterministicUnit => "deterministic-unit",
            Self::DeterministicSimulation => "deterministic-simulation",
            Self::HostedIntegration => "hosted-integration",
            Self::FreestandingEmulator => "freestanding-emulator",
            Self::LiveBrowser => "live-browser",
            Self::LiveTransport => "live-transport",
            Self::FirmwareBuild => "firmware-build",
            Self::PhysicalLocalHardware => "physical-local-hardware",
            Self::PhysicalCrossHost => "physical-cross-host",
            Self::ManualObservation => "manual-observation",
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofRequirement {
    pub required: ProofClass,
    pub explicitly_accepted_substitutes: &'static [ProofClass],
}

impl ProofRequirement {
    pub fn accepts(&self, actual: ProofClass) -> bool {
        actual == self.required || self.explicitly_accepted_substitutes.contains(&actual)
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ProofCommandContract {
    pub id: &'static str,
    pub command: &'static str,
    pub proof_class: ProofClass,
    pub required_tools_or_targets: &'static [&'static str],
    pub named_artifacts: &'static [&'static str],
    pub allowed_claims: &'static [&'static str],
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ProofRecord {
    pub schema_version: u16,
    pub git_commit: String,
    pub dirty: bool,
    pub proof_class: ProofClass,
    pub command: String,
    pub required_tools_or_targets: Vec<String>,
    pub named_artifacts: Vec<String>,
    pub host_or_board_identity: Option<String>,
    pub success: bool,
    pub timestamp: String,
    pub claims: Vec<String>,
}

impl ProofRecord {
    pub fn validate(&self) -> Result<(), &'static str> {
        if self.schema_version != PROOF_SCHEMA_VERSION {
            return Err("unsupported proof schema version");
        }
        if self.git_commit.is_empty()
            || self.command.is_empty()
            || self.timestamp.is_empty()
            || self.claims.is_empty()
        {
            return Err("proof identity, command, timestamp, and claims must be present");
        }
        if matches!(
            self.proof_class,
            ProofClass::PhysicalLocalHardware | ProofClass::PhysicalCrossHost
        ) && self
            .host_or_board_identity
            .as_deref()
            .unwrap_or("")
            .is_empty()
        {
            return Err("physical proof requires an exact host or board identity");
        }
        Ok(())
    }

    pub fn validate_against(&self, contract: &ProofCommandContract) -> Result<(), &'static str> {
        self.validate()?;
        if self.proof_class != contract.proof_class || self.command != contract.command {
            return Err("record does not identify the exact proof command and class");
        }
        if self.required_tools_or_targets
            != contract
                .required_tools_or_targets
                .iter()
                .map(ToString::to_string)
                .collect::<Vec<_>>()
            || self.named_artifacts
                != contract
                    .named_artifacts
                    .iter()
                    .map(ToString::to_string)
                    .collect::<Vec<_>>()
        {
            return Err("record does not bind the command's exact tools and artifacts");
        }
        if self
            .claims
            .iter()
            .any(|claim| !contract.allowed_claims.contains(&claim.as_str()))
        {
            return Err("record claims exceed the proof command contract");
        }
        Ok(())
    }

    pub fn satisfies(
        &self,
        contract: &ProofCommandContract,
        requirement: &ProofRequirement,
    ) -> bool {
        self.success
            && self.validate_against(contract).is_ok()
            && requirement.accepts(self.proof_class)
    }
}

pub const CURRENT_PROOF_COMMANDS: &[ProofCommandContract] = &[
    ProofCommandContract {
        id: "bluetooth.ble-gatt-physical",
        command: "cargo xtask prove bluetooth-line --evidence-root <directory> --bluetooth-role <source|sink> --bluetooth-adapter <hci-name> --bluetooth-peer-address <address> [--bluetooth-peer-host-id <host-id> --bluetooth-peer-boot-id <boot-id>]",
        proof_class: ProofClass::PhysicalCrossHost,
        required_tools_or_targets: &["cargo", "BlueZ", "two paired BLE controllers", "physical BLE radio path"],
        named_artifacts: &["bluetooth-line-source.json", "bluetooth-line-sink.json"],
        allowed_claims: &["two paired BlueZ probe sides exchange one bounded shared SessionMachine conversation over the first physical BLE GATT profile; this does not claim production-kernel or Host/Body admission"],
    },
    ProofCommandContract {
        id: "conduitos.observatory",
        command: "cargo xtask conduitos prove --arch x86-64",
        proof_class: ProofClass::FreestandingEmulator,
        required_tools_or_targets: &[
            "i686-unknown-uefi",
            "x86_64-unknown-none",
            "aarch64-unknown-none",
            "riscv64gc-unknown-none-elf",
            "loongarch64-unknown-none",
            "readelf",
            "xorriso",
            "QEMU x86_64",
        ],
        named_artifacts: &[
            "target/conduitos/x86_64/conduitos",
            "target/conduitos/x86_64/conduitos.iso",
            "target/conduitos/x86_64/kernel-proof.json",
            "target/conduitos/x86_64/observatory-snapshot.json",
        ],
        allowed_claims: &[
            "a reproducible pinned-Limine image boots in QEMU, one ordinary Form executes through the production conduit-kernel, and the Host exports bounded ordinary Observatory truth consumed by native Patchbay with sealed boot provenance",
        ],
    },
    ProofCommandContract {
        id: "simulation.current-fixtures",
        command: "cargo xtask check sim",
        proof_class: ProofClass::DeterministicSimulation,
        required_tools_or_targets: &["cargo", "wasm32-unknown-unknown", "thumbv6m-none-eabi"],
        named_artifacts: &[],
        allowed_claims: &["current browser and Pico fixtures satisfy deterministic simulation contracts"],
    },
    ProofCommandContract {
        id: "std.kernel-takeover",
        command: "cargo xtask check kernel-takeover",
        proof_class: ProofClass::HostedIntegration,
        required_tools_or_targets: &["cargo"],
        named_artifacts: &[],
        allowed_claims: &["production std execution uses the admitted conduit-kernel path"],
    },
    ProofCommandContract {
        id: "browser.host",
        command: "cargo xtask prove browser-host",
        proof_class: ProofClass::LiveBrowser,
        required_tools_or_targets: &[
            "wasm32-unknown-unknown",
            "playwright",
            "chromium",
            "firefox",
        ],
        named_artifacts: &["proof/browser/conduit_browser_runtime.wasm"],
        allowed_claims: &["actual browser host executes through the browser/WASM kernel"],
    },
    ProofCommandContract {
        id: "patchbay.front-door",
        command: "cargo xtask prove patchbay-front-door",
        proof_class: ProofClass::LiveBrowser,
        required_tools_or_targets: &["cargo", "wasm32-unknown-unknown", "playwright", "chromium"],
        named_artifacts: &[
            "target/debug/patchbay-native",
            "target/debug/patchbay-html",
            "proof/browser/conduit_browser_runtime.wasm",
            "target/conduit-evidence/patchbay-front-door/manifest.json",
        ],
        allowed_claims: &[
            "one world-first Patchbay semantic entrance executes Form to Plan to Play, retains a bounded exact-identity Program-to-Body navigation receipt, and observes live browser membership change without mutating an existing Plan",
        ],
    },
    ProofCommandContract {
        id: "browser.std-live-transport",
        command: "cargo xtask prove std-browser-s4",
        proof_class: ProofClass::LiveTransport,
        required_tools_or_targets: &["wasm32-unknown-unknown", "playwright", "chromium", "loopback WebSocket"],
        named_artifacts: &["proof/browser/conduit_browser_runtime.wasm"],
        allowed_claims: &["one bounded live loopback WebSocket carries the exact std-to-browser session"],
    },
    ProofCommandContract {
        id: "pico.firmware-build",
        command: "cargo xtask pico build",
        proof_class: ProofClass::FirmwareBuild,
        required_tools_or_targets: &["thumbv6m-none-eabi", "elf2uf2-rs"],
        named_artifacts: &["targets/rp2040/firmware/pico-w-signal/target/thumbv6m-none-eabi/release/conduit-pico-w-signal.uf2"],
        allowed_claims: &["Pico W firmware artifact builds for the reviewed target"],
    },
    ProofCommandContract {
        id: "pico.local-hardware",
        command: "cargo xtask pico verify",
        proof_class: ProofClass::PhysicalLocalHardware,
        required_tools_or_targets: &["Pico W", "USB CDC sign port"],
        named_artifacts: &[],
        allowed_claims: &["exact running Pico W produces the local physical receipt sequence"],
    },
    ProofCommandContract {
        id: "pico.cross-host-usb",
        command: "cargo xtask prove std-pico-usb",
        proof_class: ProofClass::PhysicalCrossHost,
        required_tools_or_targets: &["Pico W", "USB CDC link port", "USB CDC sign port"],
        named_artifacts: &[],
        allowed_claims: &["std and Pico kernels complete one exact physical cross-host USB session"],
    },
    ProofCommandContract {
        id: "body.pico-admission-physical",
        command: "cargo xtask pico prove-body-admission --link-port <path>",
        proof_class: ProofClass::PhysicalCrossHost,
        required_tools_or_targets: &["provisioned Pico W", "USB CDC link port", "udevadm"],
        named_artifacts: &[],
        allowed_claims: &[
            "one physically identified provisioned Pico publishes an exact bounded advertisement, remains inert until explicit authenticated admission, becomes one Body Part, and is eligible for ordinary planning",
        ],
    },
    ProofCommandContract {
        id: "pico.appliance-hello-physical",
        command: "cargo xtask prove pico-appliance --client-interface <name>",
        proof_class: ProofClass::PhysicalLocalHardware,
        required_tools_or_targets: &[
            "Pico W",
            "USB CDC Sign port",
            "NetworkManager Wi-Fi client",
            "physical 2.4 GHz radio path",
        ],
        named_artifacts: &[
            "targets/rp2040/firmware/pico-w-signal/target/thumbv6m-none-eabi/release/conduit-pico-w-signal.uf2",
            "target/pico-appliance-physical.json",
        ],
        allowed_claims: &["one physical client associates with the exact finite Pico W appliance, receives its bounded DHCP lease, resolves hello.conduit, loads the literal Hello response, and observes the exact terminal Sign sequence"],
    },
    ProofCommandContract {
        id: "pico.appliance-hello-two-pico-physical",
        command: "cargo xtask prove pico-appliance-hil --link-port <appliance-cdc0> --sign-port <appliance-cdc1> --client-link-port <client-cdc0>",
        proof_class: ProofClass::PhysicalLocalHardware,
        required_tools_or_targets: &[
            "two Pico W boards",
            "two exact USB CDC pairs",
            "physical 2.4 GHz radio path",
        ],
        named_artifacts: &[
            "target/pico-appliance-hil/appliance.uf2",
            "target/pico-appliance-hil/appliance.identity.json",
            "target/pico-appliance-hil/client.uf2",
            "target/pico-appliance-hil/client.identity.json",
            "target/pico-appliance-two-pico-physical.json",
        ],
        allowed_claims: &["one physical Pico W client associates with the exact finite Pico W appliance, receives its bounded DHCP lease, resolves hello.conduit, loads the literal Hello response, and correlates its exact receipt with the appliance terminal Sign sequence"],
    },
    ProofCommandContract {
        id: "copy.unfamiliar-user",
        command: "target/debug/conduit copy <source> <destination> --inspect",
        proof_class: ProofClass::ManualObservation,
        required_tools_or_targets: &["conduit"],
        named_artifacts: &[],
        allowed_claims: &["an unfamiliar user can complete and inspect the copy task"],
    },
    ProofCommandContract {
        id: "r1.new-plan-recovery-simulation",
        command: "cargo xtask prove r1-new-plan-recovery",
        proof_class: ProofClass::DeterministicSimulation,
        required_tools_or_targets: &["cargo"],
        named_artifacts: &[],
        allowed_claims: &["typed R1 lifecycle and planning recover from an injected WebSocket Line-unavailable Sign without physical acceptance"],
    },
    ProofCommandContract {
        id: "r1.new-plan-recovery-physical",
        command: "cargo xtask prove r1-new-plan-recovery-hil --interactive --ssid-env <name> --credential-env <name>",
        proof_class: ProofClass::PhysicalCrossHost,
        required_tools_or_targets: &[
            "Pico W",
            "USB CDC link port",
            "USB CDC Sign port",
            "ordinary Wi-Fi LAN",
            "physical Wi-Fi/network fault",
        ],
        named_artifacts: &["targets/rp2040/firmware/pico-w-signal/target/thumbv6m-none-eabi/release/conduit-pico-w-signal.uf2"],
        allowed_claims: &["one physical Pico boot executes WebSocket-only Plan A, becomes unavailable after a real network fault, and executes distinct USB-only Plan B"],
    },
    ProofCommandContract {
        id: "r1.same-plan-continuation-physical",
        command: "cargo xtask prove r1-plan-c-continuation-hil --interactive --ssid-env <name> --credential-env <name>",
        proof_class: ProofClass::PhysicalCrossHost,
        required_tools_or_targets: &[
            "Pico W",
            "USB CDC link port",
            "USB CDC Sign port",
            "ordinary Wi-Fi LAN",
            "physical Wi-Fi/network fault",
        ],
        named_artifacts: &["targets/rp2040/firmware/pico-w-signal/target/thumbv6m-none-eabi/release/conduit-pico-w-signal.uf2"],
        allowed_claims: &["one physical Pico boot retains Plan C and Play C while its selected Line changes from unavailable WebSocket to already-admitted USB CDC after bounded reconciliation"],
    },
    ProofCommandContract {
        id: "r1.complete-physical",
        command: "cargo xtask prove r1-hil --interactive --ssid-env <name> --credential-env <name>",
        proof_class: ProofClass::PhysicalCrossHost,
        required_tools_or_targets: &[
            "Pico W",
            "USB CDC link port",
            "USB CDC Sign port",
            "ordinary Wi-Fi LAN",
            "two physical Wi-Fi/network faults",
            "pinned Chromium",
        ],
        named_artifacts: &["targets/rp2040/firmware/pico-w-signal/target/thumbv6m-none-eabi/release/conduit-pico-w-signal.uf2"],
        allowed_claims: &["one born Body and one physical Pico boot execute live three-peer control, new-Plan recovery, same-Plan continuation, Lull, and a later Wake"],
    },
    ProofCommandContract {
        id: "body.membership-capstone",
        command: "cargo xtask prove body-membership",
        proof_class: ProofClass::LiveBrowser,
        required_tools_or_targets: &["cargo", "wasm32-unknown-unknown", "playwright", "chromium"],
        named_artifacts: &["proof/browser/conduit_browser_runtime.wasm"],
        allowed_claims: &[
            "bounded conformance proves Body membership topology continuity and Pico simulation contracts plus one live Chromium admission over a loopback Line",
        ],
    },
    ProofCommandContract {
        id: "body.membership-physical-capstone",
        command: "cargo xtask prove body-membership-hil --interactive --link-port <path> --sign-port <path> --ssid-env <name> --credential-env <name>",
        proof_class: ProofClass::PhysicalCrossHost,
        required_tools_or_targets: &[
            "Pico W",
            "USB CDC link port",
            "USB CDC Sign port",
            "ordinary Wi-Fi LAN",
            "two physical Wi-Fi/network faults",
            "pinned Chromium",
        ],
        named_artifacts: &["proof/browser/conduit_browser_runtime.wasm"],
        allowed_claims: &[
            "one exact Body links local std, at least two browser Parts, and one explicitly admitted physical Pico to the same immutable R1 Plan and production Play evidence",
        ],
    },
    ProofCommandContract {
        id: "calendar.google-live",
        command: "cargo xtask prove calendar-google --credential-env <name> --calendar-config-env <name>",
        proof_class: ProofClass::LiveTransport,
        required_tools_or_targets: &["cargo", "Google Calendar API", "OAuth bearer credential"],
        named_artifacts: &["target/calendar-google-live.json"],
        allowed_claims: &["one explicitly selected Google Calendar resource performs read, free-busy, create, update, and cancel through ordinary Plan/Play with separately granted authority and exact hashed receipts"],
    },
    ProofCommandContract {
        id: "llm.embodiment-live",
        command: "cargo xtask prove llm-embodiment --ollama-url <url> --ollama-model <model>",
        proof_class: ProofClass::LiveTransport,
        required_tools_or_targets: &[
            "cargo",
            "curl",
            "Ollama",
            "installed local model",
            "playwright",
            "pinned Chromium",
        ],
        named_artifacts: &[
            "target/ollama-planning-advice-live.json",
            "target/ollama-embodiment-live.json",
        ],
        allowed_claims: &["three exact Forms give the same local model distinct graph-defined perception, expression, and narrowly authorized effect power; ordinary proposal admission and runtime Signs remain separate from model output"],
    },
    ProofCommandContract {
        id: "llm.planning-advice-live",
        command: "cargo xtask prove llm-planning-advice --ollama-url <url> --ollama-model <model>",
        proof_class: ProofClass::LiveTransport,
        required_tools_or_targets: &["cargo", "curl", "Ollama", "installed local model"],
        named_artifacts: &["target/ollama-planning-advice-live.json"],
        allowed_claims: &["one bounded live model-derived placement proposal is revalidated against exact current candidate truth and submitted to the ordinary planner, which alone seals a distinct immutable Plan"],
    },
    ProofCommandContract {
        id: "messaging.github-issue-comment-live",
        command: "cargo xtask prove messaging-github --credential-env <name> --messaging-config-env <name>",
        proof_class: ProofClass::LiveTransport,
        required_tools_or_targets: &["cargo", "GitHub Issues API", "named bearer credential"],
        named_artifacts: &["target/messaging-github-live.json"],
        allowed_claims: &["one explicitly authorized portable message is acknowledged by the bounded GitHub issue-comment provider profile as sent without claiming end-recipient delivery"],
    },
];

#[derive(Debug, Serialize)]
pub struct ProofCatalog {
    pub schema_version: u16,
    pub vocabulary: &'static [ProofClass],
    pub commands: &'static [ProofCommandContract],
}

pub const fn current_catalog() -> ProofCatalog {
    ProofCatalog {
        schema_version: PROOF_SCHEMA_VERSION,
        vocabulary: PROOF_CLASS_VOCABULARY,
        commands: CURRENT_PROOF_COMMANDS,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn unknown_and_future_schema_values_fail_closed() {
        assert!(serde_json::from_str::<ProofClass>("\"unknown\"").is_err());
        let record = serde_json::from_str::<ProofRecord>(
            r#"{"schema_version":2,"git_commit":"abc","dirty":false,"proof_class":"deterministic-unit","command":"cargo test","required_tools_or_targets":[],"named_artifacts":[],"host_or_board_identity":null,"success":true,"timestamp":"supporting-only","claims":["one claim"]}"#,
        )
        .unwrap();
        assert_eq!(record.validate(), Err("unsupported proof schema version"));
    }

    #[test]
    fn substitution_requires_an_explicit_per_requirement_allowlist() {
        let live_browser = ProofRequirement {
            required: ProofClass::LiveBrowser,
            explicitly_accepted_substitutes: &[],
        };
        assert!(!live_browser.accepts(ProofClass::DeterministicSimulation));
        assert!(!live_browser.accepts(ProofClass::FirmwareBuild));
        assert!(!live_browser.accepts(ProofClass::PhysicalCrossHost));

        let explicitly_substitutable = ProofRequirement {
            required: ProofClass::HostedIntegration,
            explicitly_accepted_substitutes: &[ProofClass::PhysicalCrossHost],
        };
        assert!(explicitly_substitutable.accepts(ProofClass::PhysicalCrossHost));
    }

    #[test]
    fn firmware_and_simulation_never_imply_physical_acceptance() {
        let physical = ProofRequirement {
            required: ProofClass::PhysicalLocalHardware,
            explicitly_accepted_substitutes: &[],
        };
        assert!(!physical.accepts(ProofClass::FirmwareBuild));
        assert!(!physical.accepts(ProofClass::DeterministicSimulation));
    }

    #[test]
    fn physical_records_require_exact_hardware_identity() {
        let record = ProofRecord {
            schema_version: PROOF_SCHEMA_VERSION,
            git_commit: "abc".into(),
            dirty: false,
            proof_class: ProofClass::PhysicalCrossHost,
            command: "cargo xtask prove std-pico-usb".into(),
            required_tools_or_targets: vec![],
            named_artifacts: vec![],
            host_or_board_identity: None,
            success: true,
            timestamp: "supporting-only".into(),
            claims: vec!["physical claim".into()],
        };
        assert_eq!(
            record.validate(),
            Err("physical proof requires an exact host or board identity")
        );
    }

    #[test]
    fn current_catalog_serializes_with_one_versioned_vocabulary() {
        let value = serde_json::to_value(current_catalog()).unwrap();
        assert_eq!(value["schema_version"], PROOF_SCHEMA_VERSION);
        assert_eq!(value["vocabulary"].as_array().unwrap().len(), 11);
        let commands = value["commands"].as_array().unwrap();
        let proof_class = |id: &str| {
            commands
                .iter()
                .find(|command| command["id"] == id)
                .map(|command| command["proof_class"].clone())
                .unwrap()
        };
        assert_eq!(
            proof_class("conduitos.observatory"),
            "freestanding-emulator"
        );
        assert_eq!(proof_class("browser.host"), "live-browser");
        assert_eq!(proof_class("calendar.google-live"), "live-transport");
        assert_eq!(proof_class("pico.firmware-build"), "firmware-build");
        assert_eq!(proof_class("pico.cross-host-usb"), "physical-cross-host");
    }

    #[test]
    fn live_browser_contract_keeps_pinned_single_worker_zero_retry_policy() {
        let config = include_str!("../../../proof/browser/playwright.config.mjs");
        assert!(config.contains("workers: 1"));
        assert!(config.contains("retries: 0"));
        assert!(config.contains("name: \"chromium\""));
        assert!(config.contains("name: \"firefox\""));
        assert!(config.contains("testMatch: [\"browser-webrtc-body.spec.mjs\"]"));
    }

    #[test]
    fn record_cannot_transfer_claims_to_another_command_or_implementation() {
        let contract = &CURRENT_PROOF_COMMANDS[1];
        let mut record = ProofRecord {
            schema_version: PROOF_SCHEMA_VERSION,
            git_commit: "abc".into(),
            dirty: false,
            proof_class: contract.proof_class,
            command: contract.command.into(),
            required_tools_or_targets: contract
                .required_tools_or_targets
                .iter()
                .map(ToString::to_string)
                .collect(),
            named_artifacts: contract
                .named_artifacts
                .iter()
                .map(ToString::to_string)
                .collect(),
            host_or_board_identity: None,
            success: true,
            timestamp: "supporting-only".into(),
            claims: contract
                .allowed_claims
                .iter()
                .map(ToString::to_string)
                .collect(),
        };
        assert!(record.validate_against(contract).is_ok());
        record.command = "another face-compatible implementation".into();
        assert_eq!(
            record.validate_against(contract),
            Err("record does not identify the exact proof command and class")
        );
    }

    #[test]
    fn failed_record_is_preserved_but_cannot_satisfy_a_claim() {
        let contract = &CURRENT_PROOF_COMMANDS[0];
        let requirement = ProofRequirement {
            required: contract.proof_class,
            explicitly_accepted_substitutes: &[],
        };
        let record = ProofRecord {
            schema_version: PROOF_SCHEMA_VERSION,
            git_commit: "abc".into(),
            dirty: true,
            proof_class: contract.proof_class,
            command: contract.command.into(),
            required_tools_or_targets: contract
                .required_tools_or_targets
                .iter()
                .map(ToString::to_string)
                .collect(),
            named_artifacts: contract
                .named_artifacts
                .iter()
                .map(ToString::to_string)
                .collect(),
            host_or_board_identity: Some("failed physical hosts".into()),
            success: false,
            timestamp: "supporting-only".into(),
            claims: contract
                .allowed_claims
                .iter()
                .map(ToString::to_string)
                .collect(),
        };
        assert!(record.validate_against(contract).is_ok());
        assert!(!record.satisfies(contract, &requirement));
    }
}
