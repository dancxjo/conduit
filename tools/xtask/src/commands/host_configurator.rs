use std::{
    fs,
    io::{self, IsTerminal as _},
    path::{Path, PathBuf},
};

use conduit_host_fabrication::{
    parse_host_configuration_conduit, CheckedHostConfiguration, HostConfiguration,
};
use console::style;

use crate::{cli::GlobalOpts, commands::host_configuration_prompt};

pub fn run(path: Option<&Path>, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json || opts.quiet || opts.dry_run {
        return Err(
            "interactive configuration does not accept --json, --quiet, or --dry-run".into(),
        );
    }
    if !io::stdin().is_terminal() || !io::stdout().is_terminal() {
        return Err("host configure requires an interactive terminal".into());
    }

    let editing = path.is_some_and(Path::exists);
    let existing = path.filter(|path| path.exists()).map(load).transpose()?;
    cliclack::intro(style(" Conduit host configure ").on_cyan().black())?;
    cliclack::note(
        "Scope",
        "Saving this recipe creates no Host, Boot, Plan, Play, presence, or authority.",
    )?;
    let prompted = host_configuration_prompt::prompt(
        existing
            .as_ref()
            .map(|configuration| configuration.name.as_str()),
        existing.as_ref(),
    )?;
    let destination = match path {
        Some(path) => path.to_path_buf(),
        None => {
            let suggested = format!(
                "targets/{}/profiles/{}.host.conduit",
                prompted.checked.profile().target.family,
                prompted.checked.configuration().name
            );
            let value: String = cliclack::input("Save Host recipe as")
                .default_input(&suggested)
                .validate(|value: &String| validate_destination(value))
                .interact()?;
            PathBuf::from(value)
        }
    };
    if !is_canonical_host_path(&destination) {
        return Err("Host construction source must use the '.host.conduit' suffix".into());
    }
    if destination.exists() && !editing {
        return Err(format!(
            "refusing to replace existing Host configuration {}",
            destination.display()
        )
        .into());
    }
    if !cliclack::confirm(if editing {
        "Save these Host recipe changes?"
    } else {
        "Create this Host recipe?"
    })
    .initial_value(true)
    .interact()?
    {
        cliclack::outro_cancel("Nothing was changed")?;
        return Ok(());
    }
    if let Some(parent) = destination.parent() {
        fs::create_dir_all(parent)?;
    }
    fs::write(&destination, prompted.source)?;
    cliclack::outro_note(
        if editing {
            "Updated Host recipe"
        } else {
            "Created Host recipe"
        },
        destination.display(),
    )?;
    Ok(())
}

fn load(path: &Path) -> Result<HostConfiguration, Box<dyn std::error::Error>> {
    parse_host_configuration_conduit(&fs::read_to_string(path)?)
        .map_err(|item| format!("configuration decode refused: {item:?}").into())
}

fn validate_destination(value: &str) -> Result<(), &'static str> {
    if is_canonical_host_path(Path::new(value)) {
        Ok(())
    } else {
        Err("Use a path ending in '.host.conduit'.")
    }
}

fn is_canonical_host_path(path: &Path) -> bool {
    path.file_name()
        .and_then(|name| name.to_str())
        .is_some_and(|name| name.ends_with(".host.conduit"))
}

pub fn print_summary(
    checked: &CheckedHostConfiguration,
    opts: &GlobalOpts,
) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json {
        println!("{}", serde_json::to_string(checked.profile())?);
    } else if !opts.quiet {
        write_summary(&mut io::stdout(), checked)?;
    }
    Ok(())
}

fn write_summary(
    output: &mut impl io::Write,
    checked: &CheckedHostConfiguration,
) -> io::Result<()> {
    let configuration = checked.configuration();
    writeln!(output, "\nHost configuration: {}", configuration.name)?;
    writeln!(output, "Target: {}", checked.profile().target.key())?;
    writeln!(output, "PROFILE source: {}", checked.configuration_id())?;
    writeln!(output, "Bases")?;
    for (kind, implementation) in checked.resolved_bases() {
        writeln!(output, "  {kind:<28} {implementation}")?;
    }
    writeln!(output, "Limits")?;
    writeln!(
        output,
        "  queue items                  {}",
        configuration.limits.queue_items
    )?;
    writeln!(
        output,
        "  buffered bytes               {}",
        configuration.limits.buffered_bytes
    )?;
    writeln!(
        output,
        "  heap arena bytes             {}",
        configuration.limits.heap_arena_bytes
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn configurator_accepts_only_the_canonical_authoring_suffix() {
        assert!(is_canonical_host_path(Path::new("pico-w.host.conduit")));
        assert!(!is_canonical_host_path(Path::new("pico-w.conduit")));
        assert!(!is_canonical_host_path(Path::new("pico-w.json")));
    }
}
