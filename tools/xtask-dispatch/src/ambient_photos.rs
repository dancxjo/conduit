//! Lightweight supported entrance; provider/image dependencies stay in std-host.
use std::process::Command;

pub(super) fn run(arguments: &[String]) -> Result<(), String> {
    if !cfg!(unix) {
        return Err("ambient-photo preparation requires the Unix no-follow cache profile".into());
    }
    let (date, output) = options(arguments)?;
    let root = crate::workspace::workspace_root()?;
    let status = Command::new("cargo")
        .current_dir(root)
        .args([
            "run",
            "--locked",
            "--package",
            "conduit-std-host",
            "--example",
            "ambient_photo_prepare",
            "--",
            "--date",
            date,
            "--output",
            output,
        ])
        .status()
        .map_err(|error| format!("launch admitted photo preparation: {error}"))?;
    if status.success() {
        Ok(())
    } else {
        Err(format!("ambient-photo preparation failed: {status}"))
    }
}
fn options(arguments: &[String]) -> Result<(&str, &str), String> {
    let mut date = None;
    let mut output = None;
    if arguments.len() != 4 {
        return Err(
            "usage: cargo xtask make host ambient-photos --date YYYY-MM-DD --output DIRECTORY"
                .into(),
        );
    }
    for pair in arguments.chunks_exact(2) {
        match pair[0].as_str() {
            "--date" if date.is_none() => date = Some(pair[1].as_str()),
            "--output" if output.is_none() => output = Some(pair[1].as_str()),
            _ => {
                return Err(
                    "ambient-photo preparation requires one explicit --date and --output".into(),
                );
            }
        }
    }
    let date = date.ok_or("ambient-photo date missing")?;
    let output = output.ok_or("ambient-photo output missing")?;
    if date.len() != 10
        || date.as_bytes()[4] != b'-'
        || date.as_bytes()[7] != b'-'
        || !date
            .bytes()
            .enumerate()
            .all(|(i, c)| matches!(i, 4 | 7) || c.is_ascii_digit())
        || output.is_empty()
        || output.len() > 2048
    {
        return Err("ambient-photo date or output bound is invalid".into());
    }
    Ok((date, output))
}

#[cfg(test)]
mod tests {
    use super::*;
    fn args(values: &[&str]) -> Vec<String> {
        values.iter().map(|value| (*value).into()).collect()
    }
    #[test]
    fn explicit_date_and_output_are_required_before_launch() {
        let valid = args(&["--date", "2026-11-16", "--output", "target/photos"]);
        assert_eq!(options(&valid).unwrap(), ("2026-11-16", "target/photos"));
        for invalid in [
            vec![],
            args(&["--date", "2026-11-16"]),
            args(&["--date", "2026-11-16", "--date", "2026-11-15"]),
            args(&["--date", "2026-1-16", "--output", "target/photos"]),
            args(&["--date", "2026-11-16", "--output", ""]),
        ] {
            assert!(options(&invalid).is_err());
        }
    }
}
