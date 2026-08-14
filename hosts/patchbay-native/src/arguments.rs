use std::path::PathBuf;

pub const USAGE: &str = "Usage: patchbay-native [OPTIONS]\n\nOptions:\n  --front-door                         Enter Patchbay truthfully; the current Body may be NONE\n  --form <PATH>                         Open a canonical .conduit Form\n  --environment <PATH>                  Open an authored environment\n  --prewake                             Rehearse against authored simulation truth\n  --prewake-hold                        Hold the simulated Plan effect-free until explicitly released\n  --observatory-snapshot <PATH>         Open an Observatory snapshot\n  --linear-observatory-snapshot <PATH>  Print an Observatory snapshot as text\n  --control-demo                        Run the native control demonstration\n  --control-demo-stop                   Run the native control stop demonstration\n  --body-parts-demo                     Birth and open the canonical Parts view\n  --browser-page-url <URL>              Browser Host page used by + Browser Part\n  --browser-chat-url <WS-URL>           Planned browser Host chat Line endpoint\n  --pico-admission-port <PATH>          Existing Pico USB CDC 0 admission Line\n  --native-copy-demo                    Run the protected-copy demonstration\n  --distributed-route-demo              Run the distributed-route demonstration\n  --distributed-play                    Run the distributed Play client\n  --distributed-play-server             Run the distributed Play server\n  --smoke-exit-after-window             Exit after the first rendered frame\n  --first-run-proof                     Run the finite native first-run acceptance journey\n  --help                                Print help";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    pub help: bool,
    pub front_door: bool,
    pub exit_after_window: bool,
    pub snapshot_path: Option<PathBuf>,
    pub linear_snapshot_path: Option<PathBuf>,
    pub form_path: Option<PathBuf>,
    pub environment_path: Option<PathBuf>,
    pub prewake: bool,
    pub prewake_hold: bool,
    pub control_demo: bool,
    pub control_demo_stop: bool,
    pub body_parts_demo: bool,
    pub browser_page_url: Option<String>,
    pub browser_chat_url: Option<String>,
    pub pico_admission_port: Option<String>,
    pub native_copy_demo: bool,
    pub distributed_route_demo: bool,
    pub distributed_play: bool,
    pub distributed_play_server: bool,
    pub first_run_proof: bool,
}

pub fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--help" if !parsed.help => parsed.help = true,
            "--front-door" if !parsed.front_door => parsed.front_door = true,
            "--smoke-exit-after-window" if !parsed.exit_after_window => {
                parsed.exit_after_window = true;
            }
            "--observatory-snapshot" if parsed.snapshot_path.is_none() => {
                parsed.snapshot_path = Some(
                    arguments
                        .next()
                        .ok_or("--observatory-snapshot requires a path")?
                        .into(),
                );
            }
            "--linear-observatory-snapshot" if parsed.linear_snapshot_path.is_none() => {
                parsed.linear_snapshot_path = Some(
                    arguments
                        .next()
                        .ok_or("--linear-observatory-snapshot requires a path")?
                        .into(),
                );
            }
            "--form" if parsed.form_path.is_none() => {
                parsed.form_path = Some(arguments.next().ok_or("--form requires a path")?.into());
            }
            "--environment" if parsed.environment_path.is_none() => {
                parsed.environment_path = Some(
                    arguments
                        .next()
                        .ok_or("--environment requires a path")?
                        .into(),
                );
            }
            "--prewake" if !parsed.prewake => parsed.prewake = true,
            "--prewake-hold" if !parsed.prewake_hold => parsed.prewake_hold = true,
            "--control-demo" if !parsed.control_demo && !parsed.control_demo_stop => {
                parsed.control_demo = true;
            }
            "--control-demo-stop" if !parsed.control_demo && !parsed.control_demo_stop => {
                parsed.control_demo_stop = true;
            }
            "--body-parts-demo" if !parsed.body_parts_demo => parsed.body_parts_demo = true,
            "--browser-page-url" if parsed.browser_page_url.is_none() => {
                parsed.browser_page_url = Some(
                    arguments
                        .next()
                        .ok_or("--browser-page-url requires a URL")?,
                );
            }
            "--browser-chat-url" if parsed.browser_chat_url.is_none() => {
                parsed.browser_chat_url = Some(
                    arguments
                        .next()
                        .ok_or("--browser-chat-url requires a WebSocket URL")?,
                );
            }
            "--pico-admission-port" if parsed.pico_admission_port.is_none() => {
                parsed.pico_admission_port = Some(
                    arguments
                        .next()
                        .ok_or("--pico-admission-port requires a path")?,
                );
            }
            "--native-copy-demo" if !parsed.native_copy_demo => {
                parsed.native_copy_demo = true;
            }
            "--distributed-route-demo" if !parsed.distributed_route_demo => {
                parsed.distributed_route_demo = true;
            }
            "--distributed-play" if !parsed.distributed_play => {
                parsed.distributed_play = true;
            }
            "--distributed-play-server" if !parsed.distributed_play_server => {
                parsed.distributed_play_server = true;
            }
            "--first-run-proof" if !parsed.first_run_proof => parsed.first_run_proof = true,
            _ => {
                return Err(format!(
                    "unsupported or repeated Patchbay argument: {argument}"
                ))
            }
        }
    }
    if parsed.browser_page_url.is_some() != parsed.browser_chat_url.is_some() {
        return Err("browser page and chat URLs must be configured together".into());
    }
    if parsed.prewake_hold && !parsed.prewake {
        return Err("--prewake-hold requires --prewake".into());
    }
    Ok(parsed)
}
