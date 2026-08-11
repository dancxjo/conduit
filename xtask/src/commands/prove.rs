use crate::{
    cli::{GlobalOpts, ProveArgs, ProveTarget},
    evidence::{
        EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance, EvidenceResult,
    },
    process::{run_suite, run_suite_with_environment, StepError},
    suites::prove::{
        PROVE_BROWSER_HOST_STEPS, PROVE_STD_BROWSER_S4_STEPS, PROVE_STD_BROWSER_TOGGLE_STEPS,
    },
    workspace::workspace_root,
};

pub fn run(args: ProveArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|error| StepError::prereq("workspace-root", error))?;

    match args.proof {
        ProveTarget::StdBrowserS4 => run_suite(PROVE_STD_BROWSER_S4_STEPS, &root, opts),
        ProveTarget::StdBrowserToggle => run_suite(PROVE_STD_BROWSER_TOGGLE_STEPS, &root, opts),
        ProveTarget::BrowserHost => {
            let evidence_root = args
                .evidence_root
                .unwrap_or_else(|| root.join("target/conduit-evidence/browser-host"));
            if opts.dry_run {
                return run_suite(PROVE_BROWSER_HOST_STEPS, &root, opts);
            }

            let mut evidence =
                EvidenceManifest::new(&evidence_root, &root, "browser-host", "prove.browser-host")
                    .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))?;
            clear_patchbay_capture_outputs(evidence.root())?;
            evidence
                .declare(EvidenceOutput {
                    id: "patchbay.capture-declarations".into(),
                    kind: EvidenceKind::MachineReadableManifest,
                    path: "captures.json".into(),
                    media_type: "application/json".into(),
                    required: true,
                    provenance: EvidenceProvenance {
                        scenario_id: "patchbay-html.canonical-captures@1".into(),
                        step_id: Some("prove.browser-host.patchbay-html-matrix".into()),
                        asserted_semantic_disposition: Some(
                            "five-canonical-states-asserted".into(),
                        ),
                        ..Default::default()
                    },
                })
                .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))?;
            let environment = [("CONDUIT_EVIDENCE_ROOT", evidence.root().as_os_str())];
            match run_suite_with_environment(PROVE_BROWSER_HOST_STEPS, &root, opts, &environment) {
                Ok(()) => {
                    import_patchbay_captures(&mut evidence, true)?;
                    evidence
                        .finish(EvidenceResult::Complete)
                        .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))
                }
                Err(proof_error) => {
                    if evidence.root().join("captures.json").is_file() {
                        if let Err(error) = import_patchbay_captures(&mut evidence, false) {
                            eprintln!("xtask evidence import error after proof failure: {error}");
                        }
                    }
                    if let Err(evidence_error) =
                        evidence.finish(EvidenceResult::DiagnosticIncomplete)
                    {
                        eprintln!("xtask evidence error after proof failure: {evidence_error}");
                    }
                    Err(proof_error)
                }
            }
        }
        ProveTarget::StdPicoUsb => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                ..Default::default()
            };
            crate::commands::pico::run_prove_std_pico_usb(
                args.link_port.as_deref(),
                args.sign_port.as_deref(),
                args.interactive,
                args.induce_sink_failure,
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.std-pico-usb", error.to_string()))
        }
        ProveTarget::PicoWifiBootstrap => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                wifi_bootstrap: true,
                ..Default::default()
            };
            crate::commands::pico::run_prove_pico_wifi_bootstrap(
                args.link_port.as_deref(),
                args.sign_port.as_deref(),
                args.ssid_env.as_deref(),
                args.credential_env.as_deref(),
                crate::commands::pico::WifiProofMode::Bootstrap,
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.pico-wifi-bootstrap", error.to_string()))
        }
        ProveTarget::PicoAppliance => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                appliance_hello: true,
                link_port: args.link_port.clone(),
                port: args.sign_port.clone(),
                ..Default::default()
            };
            crate::commands::pico::run_prove_pico_appliance(
                args.link_port.as_deref(),
                args.sign_port.as_deref(),
                args.client_interface.as_deref(),
                &pico_args,
            )
            .map_err(|error| StepError::prereq("prove.pico-appliance", error.to_string()))
        }
        ProveTarget::PicoApplianceHil => crate::commands::pico::run_prove_pico_appliance_hil(
            args.link_port.as_deref(),
            args.sign_port.as_deref(),
            args.client_link_port.as_deref(),
            args.client_sign_port.as_deref(),
            opts.dry_run,
        )
        .map_err(|error| StepError::prereq("prove.pico-appliance-hil", error.to_string())),
        ProveTarget::PicoWebsocketRoute => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                wifi_bootstrap: true,
                ..Default::default()
            };
            crate::commands::pico::run_prove_pico_wifi_bootstrap(
                args.link_port.as_deref(),
                args.sign_port.as_deref(),
                args.ssid_env.as_deref(),
                args.credential_env.as_deref(),
                crate::commands::pico::WifiProofMode::WebSocketRoute,
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.pico-websocket-route", error.to_string()))
        }
        ProveTarget::R1NewPlanRecoveryHil => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                r1_control: true,
                ..Default::default()
            };
            crate::commands::pico::run_prove_pico_wifi_bootstrap(
                args.link_port.as_deref(),
                args.sign_port.as_deref(),
                args.ssid_env.as_deref(),
                args.credential_env.as_deref(),
                crate::commands::pico::WifiProofMode::R1NewPlanRecovery {
                    interactive: args.interactive,
                },
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.r1-new-plan-recovery-hil", error.to_string()))
        }
        ProveTarget::R1PlanCContinuationHil => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                r1_control: true,
                ..Default::default()
            };
            crate::commands::pico::run_prove_pico_wifi_bootstrap(
                args.link_port.as_deref(),
                args.sign_port.as_deref(),
                args.ssid_env.as_deref(),
                args.credential_env.as_deref(),
                crate::commands::pico::WifiProofMode::R1PlanCContinuation {
                    interactive: args.interactive,
                },
                &pico_args,
                opts,
            )
            .map_err(|error| {
                StepError::prereq("prove.r1-plan-c-continuation-hil", error.to_string())
            })
        }
        ProveTarget::R1Hil => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                r1_control: true,
                ..Default::default()
            };
            crate::commands::pico::run_prove_pico_wifi_bootstrap(
                args.link_port.as_deref(),
                args.sign_port.as_deref(),
                args.ssid_env.as_deref(),
                args.credential_env.as_deref(),
                crate::commands::pico::WifiProofMode::R1Full {
                    interactive: args.interactive,
                },
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.r1-hil", error.to_string()))
        }
        ProveTarget::R1NewPlanRecovery => crate::commands::r1_recovery::run(opts),
    }
}

const PATCHBAY_CAPTURE_IDS: &[&str] = &[
    "patchbay.overview",
    "patchbay.selected-gear",
    "patchbay.plan-lens",
    "patchbay.play-lens",
    "patchbay.signs-lens",
    "patchbay.route-recovery",
    "patchbay.interaction",
    "patchbay.high-contrast",
    "patchbay.disconnected",
];

fn import_patchbay_captures(
    evidence: &mut EvidenceManifest,
    require_complete: bool,
) -> Result<(), StepError> {
    let required = if require_complete {
        PATCHBAY_CAPTURE_IDS
    } else {
        &[]
    };
    evidence
        .import_capture_declarations(std::path::Path::new("captures.json"), required)
        .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))
}

fn clear_patchbay_capture_outputs(root: &std::path::Path) -> Result<(), StepError> {
    for name in [
        "captures.json",
        "captures.json.tmp",
        "overview.png",
        "selected-gear.png",
        "plan-lens.png",
        "play-lens.png",
        "signs-lens.png",
        "route-recovery.png",
        "interaction.png",
        "high-contrast.png",
        "disconnected.png",
    ] {
        let path = root.join(name);
        if path.exists() || path.is_symlink() {
            std::fs::remove_file(&path).map_err(|error| {
                StepError::prereq(
                    "prove.browser-host.evidence",
                    format!("cannot remove stale evidence {}: {error}", path.display()),
                )
            })?;
        }
    }
    Ok(())
}
