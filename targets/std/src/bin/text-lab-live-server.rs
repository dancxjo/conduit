use std::io::Write;
use std::process::ExitCode;

use conduit_std_host::text_lab_live::TextLabLiveServer;

fn main() -> ExitCode {
    let server = match TextLabLiveServer::bind() {
        Ok(server) => server,
        Err(error) => {
            eprintln!("{error}");
            return ExitCode::FAILURE;
        }
    };
    println!("{}", server.url());
    if let Err(error) = std::io::stdout().flush() {
        eprintln!("{error}");
        return ExitCode::FAILURE;
    }
    match server.run(&mut std::io::stdout()) {
        Ok(()) => ExitCode::SUCCESS,
        Err(error) => {
            eprintln!("{error}");
            ExitCode::FAILURE
        }
    }
}
