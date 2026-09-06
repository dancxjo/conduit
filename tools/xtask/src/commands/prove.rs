use crate::{
    cli::{GlobalOpts, ProveArgs, ProveTarget},
    evidence::{
        EvidenceKind, EvidenceManifest, EvidenceOutput, EvidenceProvenance, EvidenceResult,
    },
    process::{run_suite, run_suite_with_environment, StepError},
    suites::patchbay_body_workbench::PROVE_PATCHBAY_BODY_WORKBENCH_STEPS,
    suites::prove::{
        PROVE_BODY_MEMBERSHIP_HIL_BROWSER_STEPS, PROVE_BODY_MEMBERSHIP_STEPS,
        PROVE_BROWSER_HOST_STEPS, PROVE_DEGRADED_PROFILES_STEPS, PROVE_DIVERSITY_STEPS,
        PROVE_DORMANT_READMISSION_STEPS, PROVE_LLM_CROSS_HOST_STEPS, PROVE_LLM_EMBODIMENT_STEPS,
        PROVE_PATCHBAY_FRONT_DOOR_STEPS, PROVE_RECURSIVE_RECOVERY_STEPS,
        PROVE_STD_BROWSER_S4_STEPS, PROVE_STD_BROWSER_TOGGLE_STEPS,
    },
    workspace::workspace_root,
};

pub fn run(args: ProveArgs, opts: &GlobalOpts) -> Result<(), StepError> {
    let root = workspace_root().map_err(|error| StepError::prereq("workspace-root", error))?;

    match args.proof {
        ProveTarget::BluetoothLine => crate::commands::bluetooth::run(&args, &root, opts),
        ProveTarget::BluetoothPico => crate::commands::bluetooth::run_pico(&args, &root, opts),
        ProveTarget::BodyMembership => run_suite(PROVE_BODY_MEMBERSHIP_STEPS, &root, opts),
        ProveTarget::BodyMembershipHil => run_body_membership_hil(&args, &root, opts),
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
            if args.induce_pre_capture_failure {
                evidence
                    .finish(EvidenceResult::DiagnosticIncomplete)
                    .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))?;
                return Err(StepError::prereq(
                    "prove.browser-host.induced-pre-capture",
                    "intentional failure before canonical capture",
                ));
            }
            let mut environment = vec![("CONDUIT_EVIDENCE_ROOT", evidence.root().as_os_str())];
            if args.induce_capture_restart_failure {
                environment.push(("CONDUIT_CAPTURE_RESTART_PROBE", std::ffi::OsStr::new("1")));
            }
            match run_suite_with_environment(PROVE_BROWSER_HOST_STEPS, &root, opts, &environment) {
                Ok(()) => {
                    declare_patchbay_capture_manifest(&mut evidence)?;
                    declare_browser_webrtc_manifest(&mut evidence)?;
                    import_patchbay_captures(&mut evidence, true)?;
                    evidence
                        .finish(EvidenceResult::Complete)
                        .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))
                }
                Err(proof_error) => {
                    if evidence.root().join("captures.json").is_file() {
                        if let Err(error) = declare_patchbay_capture_manifest(&mut evidence)
                            .and_then(|()| import_patchbay_captures(&mut evidence, false))
                        {
                            eprintln!("xtask evidence import error after proof failure: {error}");
                        }
                    }
                    if [
                        "browser-webrtc-session.json",
                        "browser-webrtc-session-firefox.json",
                    ]
                    .iter()
                    .any(|name| evidence.root().join(name).is_file())
                    {
                        if let Err(error) = declare_browser_webrtc_manifest(&mut evidence) {
                            eprintln!(
                                "xtask WebRTC evidence import error after proof failure: {error}"
                            );
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
        ProveTarget::CalendarGoogle => crate::commands::calendar_google::run(&args, &root, opts),
        ProveTarget::DegradedProfiles => run_suite(PROVE_DEGRADED_PROFILES_STEPS, &root, opts),
        ProveTarget::ResourceFrame => run_suite(crate::suites::resource_frame::STEPS, &root, opts),
        ProveTarget::Diversity => run_suite(PROVE_DIVERSITY_STEPS, &root, opts),
        ProveTarget::DistributedLenia => {
            crate::commands::distributed_lenia::run(&args, &root, opts)
        }
        ProveTarget::DormantReadmission => run_suite(PROVE_DORMANT_READMISSION_STEPS, &root, opts),
        ProveTarget::RecursiveRecovery => run_suite(PROVE_RECURSIVE_RECOVERY_STEPS, &root, opts),
        ProveTarget::LlmPlanningAdvice => {
            crate::commands::ollama_planning_advice::run(&args, &root, opts)
        }
        ProveTarget::LlmEmbodiment => {
            run_suite(PROVE_LLM_EMBODIMENT_STEPS, &root, opts)?;
            crate::commands::ollama_embodiment::run(&args, &root, opts)
        }
        ProveTarget::LlmCrossHost => run_suite(PROVE_LLM_CROSS_HOST_STEPS, &root, opts),
        ProveTarget::MessagingGithub => crate::commands::messaging_github::run(&args, &root, opts),
        ProveTarget::PatchbayBodyWorkbench => {
            run_suite(PROVE_PATCHBAY_BODY_WORKBENCH_STEPS, &root, opts)
        }
        ProveTarget::PatchbayFrontDoor => run_patchbay_front_door(&args, &root, opts),
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
                    membership_receipt: None,
                },
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.r1-hil", error.to_string()))
        }
        ProveTarget::R1NewPlanRecovery => crate::commands::r1_recovery::run(opts),
    }
}

fn run_patchbay_front_door(
    args: &ProveArgs,
    root: &std::path::Path,
    opts: &GlobalOpts,
) -> Result<(), StepError> {
    let evidence_root = args
        .evidence_root
        .clone()
        .unwrap_or_else(|| root.join("target/conduit-evidence/patchbay-front-door"));
    if opts.dry_run {
        return run_suite(PROVE_PATCHBAY_FRONT_DOOR_STEPS, root, opts);
    }
    let mut evidence = EvidenceManifest::new(
        &evidence_root,
        root,
        "patchbay-front-door",
        "prove.patchbay-front-door",
    )
    .map_err(|error| StepError::prereq("prove.patchbay-front-door.evidence", error))?;
    for name in ["front-door.json", "topology.json"] {
        let path = evidence.root().join(name);
        if path.exists() || path.is_symlink() {
            std::fs::remove_file(&path).map_err(|error| {
                StepError::prereq(
                    "prove.patchbay-front-door.evidence",
                    format!("cannot remove stale evidence {}: {error}", path.display()),
                )
            })?;
        }
    }
    for output in [
        EvidenceOutput {
            id: "patchbay.front-door".into(),
            kind: EvidenceKind::MachineReadableManifest,
            path: "front-door.json".into(),
            media_type: "application/json".into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "patchbay.front-door@1".into(),
                step_id: Some("prove.patchbay-front-door.browser".into()),
                asserted_semantic_disposition: Some(
                    "world-intent-plan-play-navigation-follow-back-and-refusal-asserted".into(),
                ),
                proof_class: Some("live-browser".into()),
                ..Default::default()
            },
        },
        EvidenceOutput {
            id: "patchbay.live-topology".into(),
            kind: EvidenceKind::MachineReadableManifest,
            path: "topology.json".into(),
            media_type: "application/json".into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "patchbay.live-topology@1".into(),
                step_id: Some("prove.patchbay-front-door.live-membership".into()),
                asserted_semantic_disposition: Some(
                    "join-offline-plan-immutability-and-explicit-replan-asserted".into(),
                ),
                proof_class: Some("live-browser".into()),
                ..Default::default()
            },
        },
    ] {
        evidence
            .declare(output)
            .map_err(|error| StepError::prereq("prove.patchbay-front-door.evidence", error))?;
    }
    let front_door_receipt = evidence.root().join("front-door.json");
    let topology_receipt = evidence.root().join("topology.json");
    let environment = [
        (
            "CONDUIT_PATCHBAY_FRONT_DOOR_RECEIPT_PATH",
            front_door_receipt.as_os_str(),
        ),
        (
            "CONDUIT_PATCHBAY_TOPOLOGY_RECEIPT_PATH",
            topology_receipt.as_os_str(),
        ),
    ];
    match run_suite_with_environment(PROVE_PATCHBAY_FRONT_DOOR_STEPS, root, opts, &environment) {
        Ok(()) => evidence
            .finish(EvidenceResult::Complete)
            .map_err(|error| StepError::prereq("prove.patchbay-front-door.evidence", error)),
        Err(proof_error) => {
            if let Err(error) = evidence.finish(EvidenceResult::DiagnosticIncomplete) {
                eprintln!("xtask evidence error after proof failure: {error}");
            }
            Err(proof_error)
        }
    }
}

fn run_body_membership_hil(
    args: &ProveArgs,
    root: &std::path::Path,
    opts: &GlobalOpts,
) -> Result<(), StepError> {
    let link_port = args.link_port.as_deref().ok_or_else(|| {
        StepError::prereq(
            "prove.body-membership-hil",
            "--link-port is required so browser admission and R1 Play use the same Pico",
        )
    })?;
    let receipt_path = root.join("target/conduit-evidence/body-membership-hil/membership.json");
    if !opts.dry_run && receipt_path.exists() {
        std::fs::remove_file(&receipt_path).map_err(|error| {
            StepError::prereq(
                "prove.body-membership-hil",
                format!("remove stale receipt: {error}"),
            )
        })?;
    }
    // The physical identity link is the capstone, not a substitute for the
    // finite hostile, bounds, stale-Boot, and browser no-autorun suite.
    run_suite(PROVE_BODY_MEMBERSHIP_STEPS, root, opts)?;
    let environment = [
        ("CONDUIT_B9_PICO_LINK_PORT", std::ffi::OsStr::new(link_port)),
        (
            "CONDUIT_B9_MEMBERSHIP_RECEIPT_PATH",
            receipt_path.as_os_str(),
        ),
    ];
    run_suite_with_environment(
        PROVE_BODY_MEMBERSHIP_HIL_BROWSER_STEPS,
        root,
        opts,
        &environment,
    )?;

    let pico_args = crate::commands::pico::PicoArgs {
        dry_run: opts.dry_run,
        r1_control: true,
        ..Default::default()
    };
    crate::commands::pico::run_prove_pico_wifi_bootstrap(
        Some(link_port),
        args.sign_port.as_deref(),
        args.ssid_env.as_deref(),
        args.credential_env.as_deref(),
        crate::commands::pico::WifiProofMode::R1Full {
            interactive: args.interactive,
            membership_receipt: Some(receipt_path),
        },
        &pico_args,
        opts,
    )
    .map_err(|error| StepError::prereq("prove.body-membership-hil", error.to_string()))
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
    "patchbay.responsive",
];

fn declare_patchbay_capture_manifest(evidence: &mut EvidenceManifest) -> Result<(), StepError> {
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
                asserted_semantic_disposition: Some("five-canonical-states-asserted".into()),
                ..Default::default()
            },
        })
        .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))
}

fn declare_browser_webrtc_manifest(evidence: &mut EvidenceManifest) -> Result<(), StepError> {
    declare_browser_webrtc_output(
        evidence,
        "browser-host.body-granted-webrtc-session.chromium",
        "browser-webrtc-session.json",
        "prove.browser-host.playwright",
        "chromium",
    )?;
    declare_browser_webrtc_output(
        evidence,
        "browser-host.body-granted-webrtc-session.firefox",
        "browser-webrtc-session-firefox.json",
        "prove.browser-host.playwright",
        "firefox",
    )
}

fn declare_browser_webrtc_output(
    evidence: &mut EvidenceManifest,
    id: &str,
    path: &str,
    step_id: &str,
    browser_engine: &str,
) -> Result<(), StepError> {
    evidence
        .declare(EvidenceOutput {
            id: id.into(),
            kind: EvidenceKind::MachineReadableManifest,
            path: path.into(),
            media_type: "application/json".into(),
            required: true,
            provenance: EvidenceProvenance {
                scenario_id: "browser-host.body-granted-webrtc-session@1".into(),
                step_id: Some(step_id.into()),
                browser_engine: Some(browser_engine.into()),
                asserted_semantic_disposition: Some(
                    "two-admitted-browser-hosts-ready-value-delivery-and-line-loss-asserted".into(),
                ),
                proof_class: Some("live-browser".into()),
                ..Default::default()
            },
        })
        .map_err(|error| StepError::prereq("prove.browser-host.evidence", error))
}

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
        "responsive.png",
        "browser-webrtc-session.json",
        "browser-webrtc-session.json.tmp",
        "browser-webrtc-session-firefox.json",
        "browser-webrtc-session-firefox.json.tmp",
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
