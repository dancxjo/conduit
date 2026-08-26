use std::{ffi::OsString, path::PathBuf};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Args {
    pub repo_root: PathBuf,
    pub receipt: PathBuf,
    pub allow_dirty: bool,
    pub dry_run: bool,
    pub quiet: bool,
    pub json: bool,
}

impl Args {
    pub fn parse<I>(values: I) -> Result<Self, Box<dyn std::error::Error>>
    where
        I: IntoIterator<Item = OsString>,
    {
        let mut values = values.into_iter();
        match values.next().and_then(|value| value.into_string().ok()) {
            Some(command) if command == "check" => {}
            _ => return Err("expected the `check` subcommand".into()),
        }

        let mut repo_root = None;
        let mut receipt = None;
        let mut allow_dirty = false;
        let mut dry_run = false;
        let mut quiet = false;
        let mut json = false;

        while let Some(value) = values.next() {
            let flag = value
                .into_string()
                .map_err(|_| "arguments must be valid UTF-8")?;
            match flag.as_str() {
                "--repo-root" => {
                    set_path(&mut repo_root, values.next(), "--repo-root")?;
                }
                "--receipt" => set_path(&mut receipt, values.next(), "--receipt")?,
                "--allow-dirty" => set_once(&mut allow_dirty, "--allow-dirty")?,
                "--dry-run" => set_once(&mut dry_run, "--dry-run")?,
                "--quiet" => set_once(&mut quiet, "--quiet")?,
                "--json" => set_once(&mut json, "--json")?,
                _ => return Err(format!("unknown argument `{flag}`").into()),
            }
        }

        let repo_root = repo_root.ok_or("--repo-root is required")?;
        let receipt = receipt.ok_or("--receipt is required")?;
        if !repo_root.is_absolute() {
            return Err("--repo-root must be absolute".into());
        }
        if !receipt.is_absolute() {
            return Err("--receipt must be absolute".into());
        }
        Ok(Self {
            repo_root,
            receipt,
            allow_dirty,
            dry_run,
            quiet,
            json,
        })
    }
}

fn set_path(
    destination: &mut Option<PathBuf>,
    value: Option<OsString>,
    flag: &str,
) -> Result<(), Box<dyn std::error::Error>> {
    if destination.is_some() {
        return Err(format!("{flag} may be supplied only once").into());
    }
    *destination = Some(PathBuf::from(
        value.ok_or_else(|| format!("{flag} requires a value"))?,
    ));
    Ok(())
}

fn set_once(value: &mut bool, flag: &str) -> Result<(), Box<dyn std::error::Error>> {
    if *value {
        return Err(format!("{flag} may be supplied only once").into());
    }
    *value = true;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn strings(values: &[&str]) -> Vec<OsString> {
        values.iter().map(OsString::from).collect()
    }

    #[test]
    fn exact_check_arguments_are_accepted() {
        let args = Args::parse(strings(&[
            "check",
            "--repo-root",
            "/repo",
            "--receipt",
            "/evidence/receipt.json",
            "--allow-dirty",
            "--dry-run",
            "--quiet",
            "--json",
        ]))
        .unwrap();
        assert_eq!(args.repo_root, PathBuf::from("/repo"));
        assert_eq!(args.receipt, PathBuf::from("/evidence/receipt.json"));
        assert!(args.allow_dirty && args.dry_run && args.quiet && args.json);
    }

    #[test]
    fn relative_or_duplicate_inputs_are_refused() {
        assert!(Args::parse(strings(&[
            "check",
            "--repo-root",
            "repo",
            "--receipt",
            "/receipt",
        ]))
        .is_err());
        assert!(Args::parse(strings(&[
            "check",
            "--repo-root",
            "/one",
            "--repo-root",
            "/two",
            "--receipt",
            "/receipt",
        ]))
        .is_err());
    }
}
