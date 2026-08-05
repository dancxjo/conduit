use std::io::Write;

use conduit_std_host::distributed_signal::{bind_listener, DistributedSource};

fn main() -> Result<(), String> {
    let source = DistributedSource::prepare()?;
    let listener = bind_listener()?;
    let url = listener.url().map_err(|error| format!("{error:?}"))?;
    println!("{url}");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    source.run(listener, &mut std::io::stdout())
}
