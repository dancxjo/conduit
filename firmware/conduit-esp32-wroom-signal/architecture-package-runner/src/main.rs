mod check;
mod cli;
mod descriptor;
mod process;
mod provenance;
mod receipt;
mod selection;

fn main() {
    if let Err(error) = run() {
        eprintln!("ESP32 architecture-package check refused: {error}");
        std::process::exit(1);
    }
}

fn run() -> Result<(), Box<dyn std::error::Error>> {
    let args = cli::Args::parse(std::env::args_os().skip(1))?;
    check::run(args)
}
