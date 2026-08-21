use std::io::Write;

use conduit_std_host::text_lab_live::TextLabLiveServer;

fn main() -> Result<(), String> {
    let server = TextLabLiveServer::bind()?;
    println!("{}", server.url());
    std::io::stdout()
        .flush()
        .map_err(|error| error.to_string())?;
    server.run(&mut std::io::stdout())
}
