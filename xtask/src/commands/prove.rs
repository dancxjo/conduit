use crate::{
    cli::{GlobalOpts, ProveArgs, ProveTarget},
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
        ProveTarget::BrowserHost => run_suite(PROVE_BROWSER_HOST_STEPS, &root, opts),
        ProveTarget::StdPicoUsb => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                ..Default::default()
            };
            crate::commands::pico::run_prove_std_pico_usb(
                args.link_port.as_deref(),
                args.clue_port.as_deref(),
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
                args.clue_port.as_deref(),
                args.ssid_env.as_deref(),
                args.credential_env.as_deref(),
                false,
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.pico-wifi-bootstrap", error.to_string()))
        }
        ProveTarget::PicoWebsocketRoute => {
            let pico_args = crate::commands::pico::PicoArgs {
                dry_run: opts.dry_run,
                wifi_bootstrap: true,
                ..Default::default()
            };
            crate::commands::pico::run_prove_pico_wifi_bootstrap(
                args.link_port.as_deref(),
                args.clue_port.as_deref(),
                args.ssid_env.as_deref(),
                args.credential_env.as_deref(),
                true,
                &pico_args,
                opts,
            )
            .map_err(|error| StepError::prereq("prove.pico-websocket-route", error.to_string()))
        }
        ProveTarget::R1NewPlanRecovery => crate::commands::r1_recovery::run(opts),
    }
}
