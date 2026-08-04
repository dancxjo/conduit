use std::env;
use std::fs;

fn parse_panel(source: &str) -> Result<(), String> {
    let first = source.lines().next().unwrap_or("").trim();
    if first != "panel 0" {
        return Err(format!(
            "expected first line to be 'panel 0', got '{first}'"
        ));
    }
    Ok(())
}

fn run(path: &str) -> Result<(), String> {
    let source =
        fs::read_to_string(path).map_err(|err| format!("failed to read '{path}': {err}"))?;
    parse_panel(&source)?;

    // First vertical slice: one finite body, one local execution.
    println!("Hello, world!");
    Ok(())
}

fn main() {
    let mut args = env::args();
    let _program = args.next();
    let path = match args.next() {
        Some(path) => path,
        None => {
            eprintln!("usage: conduit <panel-file>");
            std::process::exit(2);
        }
    };

    if let Err(err) = run(&path) {
        eprintln!("error: {err}");
        std::process::exit(1);
    }
}

#[cfg(test)]
mod tests {
    use super::parse_panel;

    #[test]
    fn accepts_panel_zero_header() {
        assert!(parse_panel("panel 0\npart x").is_ok());
    }

    #[test]
    fn rejects_other_headers() {
        assert!(parse_panel("panel 1\npart x").is_err());
    }
}
