use crate::{
    cli::{GlobalOpts, ProveArgs, ProveTarget},
    evidence::{EvidenceManifest, EvidenceResult},
    process::{run_suite, StepError},
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
            match run_suite(PROVE_BROWSER_HOST_STEPS, &root, opts) {
                Ok(()) => evidence
                    .finish(EvidenceResult::Complete)
                    .map_err(|error| StepError::prereq("prove.browser-host.evidence", error)),
                Err(proof_error) => {
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
