use std::io::Write;
use std::path::PathBuf;

use conduit_signal_conformance::DISTRIBUTED_MAXIMUM_FRAME_BYTES;
use conduit_std_host::triple_signal::{default_pico_ports, TriplePhysicalRunner};
use conduit_std_host::websocket::NativeWebSocketListener;

fn main() -> Result<(), String> {
    let (default_link, default_sign) = default_pico_ports()?;
    let mut link = std::env::var_os("CONDUIT_PICO_LINK_PORT")
        .map(PathBuf::from)
        .unwrap_or(default_link);
    let mut sign = std::env::var_os("CONDUIT_PICO_SIGN_PORT")
        .map(PathBuf::from)
        .unwrap_or(default_sign);
    let mut args = std::env::args().skip(1);
    while let Some(argument) = args.next() {
        match argument.as_str() {
            "--link-port" => {
                link = PathBuf::from(args.next().ok_or("--link-port requires a path")?)
            }
            "--sign-port" => {
                sign = PathBuf::from(args.next().ok_or("--sign-port requires a path")?)
            }
            other => return Err(format!("unknown triple-signal-server argument: {other}")),
        }
    }
    let runner = TriplePhysicalRunner::prepare()?;
    let listener = NativeWebSocketListener::bind_loopback(DISTRIBUTED_MAXIMUM_FRAME_BYTES)
        .map_err(|error| format!("{error:?}"))?;
    let url = listener.url().map_err(|error| format!("{error:?}"))?;
    println!("{url}");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    runner.run(listener, &link, &sign, &mut std::io::stdout())
}
