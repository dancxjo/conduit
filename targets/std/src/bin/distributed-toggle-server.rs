use std::io::{BufReader, Write};

use conduit_std_host::distributed_toggle::{bind_listener, DistributedToggleSource};

fn main() -> Result<(), String> {
    let source = DistributedToggleSource::prepare()?;
    let listener = bind_listener()?;
    let url = listener.url().map_err(|error| format!("{error:?}"))?;
    println!("{url}");
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    let mut stdin = BufReader::new(std::io::stdin());
    source.run(listener, &mut stdin, &mut std::io::stdout())
}
