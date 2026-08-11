use std::path::PathBuf;

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Arguments {
    pub exit_after_window: bool,
    pub snapshot_path: Option<PathBuf>,
    pub linear_snapshot_path: Option<PathBuf>,
    pub form_path: Option<PathBuf>,
    pub environment_path: Option<PathBuf>,
    pub control_demo: bool,
    pub control_demo_stop: bool,
    pub native_copy_demo: bool,
    pub distributed_route_demo: bool,
    pub distributed_play: bool,
    pub distributed_play_server: bool,
}

pub fn parse_arguments(mut arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut parsed = Arguments::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
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
            "--control-demo" if !parsed.control_demo && !parsed.control_demo_stop => {
                parsed.control_demo = true;
            }
            "--control-demo-stop" if !parsed.control_demo && !parsed.control_demo_stop => {
                parsed.control_demo_stop = true;
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
            _ => {
                return Err(format!(
                    "unsupported or repeated Patchbay argument: {argument}"
                ))
            }
        }
    }
    Ok(parsed)
}
