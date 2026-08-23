use std::{
    fs,
    io::{self, BufRead, Write},
    path::{Path, PathBuf},
};

use conduit_host_fabrication::{
    canonical_host_configuration_toml, check_host_configuration, compatible_base_implementations,
    parse_host_configuration, target_descriptors, CheckedHostConfiguration, ConfigurationBase,
    ConfigurationTarget, FabricationCatalog, HostConfiguration, HOST_CONFIGURATION_SCHEMA,
};

use crate::cli::GlobalOpts;

pub fn run(path: Option<&Path>, opts: &GlobalOpts) -> Result<(), Box<dyn std::error::Error>> {
    if opts.json || opts.quiet || opts.dry_run {
        return Err(
            "interactive configuration does not accept --json, --quiet, or --dry-run".into(),
        );
    }
    let stdin = io::stdin();
    let mut stdout = io::stdout();
    configure(&mut stdin.lock(), &mut stdout, path)
}

fn configure(
    input: &mut impl BufRead,
    output: &mut impl Write,
    path: Option<&Path>,
) -> Result<(), Box<dyn std::error::Error>> {
    writeln!(output, "Conduit Host Configurator")?;
    writeln!(
        output,
        "Saving this recipe creates no Host, Boot, Plan, Play, presence, or authority."
    )?;
    let mut destination = path.map(Path::to_path_buf);
    let mut configuration = if path.is_some_and(Path::exists) {
        let source = fs::read_to_string(path.unwrap())?;
        parse_host_configuration(&source)
            .map_err(|item| format!("configuration decode refused: {item:?}"))?
    } else {
        create_configuration(input, output)?
    };
    loop {
        let checked =
            check_host_configuration(configuration.clone(), &FabricationCatalog::canonical())
                .map_err(|items| format!("configuration refused: {items:?}"))?;
        write_summary(output, &checked)?;
        writeln!(
            output,
            "[s] Save  [e] Edit  [v] Validate without saving  [q] Quit"
        )?;
        match read_line(input)?.to_ascii_lowercase().as_str() {
            "s" | "save" => {
                if destination.is_none() {
                    writeln!(output, "Destination path:")?;
                    destination = Some(PathBuf::from(read_line(input)?));
                }
                let destination = destination.as_ref().unwrap();
                if let Some(parent) = destination.parent() {
                    fs::create_dir_all(parent)?;
                }
                let canonical = canonical_host_configuration_toml(&configuration)
                    .map_err(|item| format!("configuration encode refused: {item:?}"))?;
                fs::write(destination, canonical)?;
                writeln!(output, "SAVED {}", destination.display())?;
                return Ok(());
            }
            "e" | "edit" => configuration = edit_configuration(configuration, input, output)?,
            "v" | "validate" => writeln!(output, "VALID {}", checked.configuration_id())?,
            "q" | "quit" => return Ok(()),
            _ => writeln!(output, "Choose save, edit, validate, or quit.")?,
        }
    }
}

fn create_configuration(
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<HostConfiguration, Box<dyn std::error::Error>> {
    writeln!(output, "Configuration name:")?;
    let name = read_line(input)?;
    let descriptors = target_descriptors();
    writeln!(output, "Target:")?;
    for (index, descriptor) in descriptors.iter().enumerate() {
        writeln!(
            output,
            "  [{}] {} ({}/{})",
            index + 1,
            descriptor.label,
            descriptor.architecture,
            descriptor.machine
        )?;
    }
    let selected = choose_index(input, descriptors.len())?;
    let descriptor = &descriptors[selected];
    let bases = choose_bases(input, output, descriptor)?;
    Ok(HostConfiguration {
        schema: HOST_CONFIGURATION_SCHEMA,
        name,
        target: ConfigurationTarget {
            architecture: descriptor.architecture.into(),
            machine: descriptor.machine.into(),
            board: descriptor.board.map(str::to_owned),
            os: descriptor.os.map(str::to_owned),
        },
        bases,
        resources: Vec::new(),
        limits: descriptor.maxima.clone(),
    })
}

fn edit_configuration(
    mut configuration: HostConfiguration,
    input: &mut impl BufRead,
    output: &mut impl Write,
) -> Result<HostConfiguration, Box<dyn std::error::Error>> {
    writeln!(output, "New name (blank keeps {}):", configuration.name)?;
    let name = read_line(input)?;
    if !name.is_empty() {
        configuration.name = name;
    }
    let descriptor = target_descriptors()
        .into_iter()
        .find(|item| {
            item.architecture == configuration.target.architecture
                && item.machine == configuration.target.machine
                && item.board.map(str::to_owned) == configuration.target.board
                && item.os.map(str::to_owned) == configuration.target.os
        })
        .ok_or("existing target has no descriptor")?;
    writeln!(output, "Replace Base selections? [y/N]")?;
    if matches!(read_line(input)?.to_ascii_lowercase().as_str(), "y" | "yes") {
        configuration.bases = choose_bases(input, output, &descriptor)?;
    }
    writeln!(output, "Replace finite limits? [y/N]")?;
    if matches!(read_line(input)?.to_ascii_lowercase().as_str(), "y" | "yes") {
        writeln!(output, "Enter nine comma-separated positive values for static bytes, heap bytes, queue items, buffered bytes, active instances, operation slots, timer slots, line sessions, evidence items:")?;
        let values = read_line(input)?
            .split(',')
            .map(str::trim)
            .map(str::parse::<u64>)
            .collect::<Result<Vec<_>, _>>()?;
        if values.len() != 9 {
            return Err("exactly nine finite limit values are required".into());
        }
        configuration.limits = conduit_host_fabrication::HostBounds {
            static_memory_bytes: values[0],
            heap_arena_bytes: values[1],
            queue_items: u32::try_from(values[2])?,
            buffered_bytes: values[3],
            active_instances: u32::try_from(values[4])?,
            operation_slots: u32::try_from(values[5])?,
            timer_slots: u32::try_from(values[6])?,
            line_sessions: u32::try_from(values[7])?,
            evidence_items: u32::try_from(values[8])?,
        };
    }
    Ok(configuration)
}

fn choose_bases(
    input: &mut impl BufRead,
    output: &mut impl Write,
    descriptor: &conduit_host_fabrication::HostTargetDescriptor,
) -> Result<Vec<ConfigurationBase>, Box<dyn std::error::Error>> {
    let choices = compatible_base_implementations(descriptor, &FabricationCatalog::canonical());
    writeln!(
        output,
        "Compatible Bases (comma-separated numbers, blank for none):"
    )?;
    for (index, (kind, implementations)) in choices.iter().enumerate() {
        writeln!(
            output,
            "  [{}] {} -> {}",
            index + 1,
            kind,
            implementations.join(" | ")
        )?;
    }
    writeln!(output, "Unavailable variants are omitted because the shared BUILD metadata rejects them for this target.")?;
    let selection = read_line(input)?;
    let mut bases = Vec::new();
    for token in selection
        .split(',')
        .map(str::trim)
        .filter(|item| !item.is_empty())
    {
        let index = token
            .parse::<usize>()
            .map_err(|_| "Base selection must be a number")?;
        let (kind, implementations) = choices
            .get(index.checked_sub(1).ok_or("Base selection starts at 1")?)
            .ok_or("Base selection is outside the offered range")?;
        bases.push(ConfigurationBase {
            kind: kind.clone(),
            implementation: Some(implementations[0].clone()),
            implementations: Vec::new(),
        });
    }
    Ok(bases)
}

fn choose_index(
    input: &mut impl BufRead,
    count: usize,
) -> Result<usize, Box<dyn std::error::Error>> {
    let choice = read_line(input)?.parse::<usize>()?;
    choice
        .checked_sub(1)
        .filter(|index| *index < count)
        .ok_or_else(|| "selection is outside the offered range".into())
}

fn read_line(input: &mut impl BufRead) -> Result<String, io::Error> {
    let mut value = String::new();
    input.read_line(&mut value)?;
    Ok(value.trim().to_owned())
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

fn write_summary(output: &mut impl Write, checked: &CheckedHostConfiguration) -> io::Result<()> {
    let configuration = checked.configuration();
    writeln!(output, "\nHost configuration: {}", configuration.name)?;
    writeln!(
        output,
        "Target: {} / {}",
        configuration.target.machine, configuration.target.architecture
    )?;
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
    use super::configure;
    use std::io::Cursor;

    #[test]
    fn recorded_create_and_edit_transcript() {
        let directory =
            std::env::temp_dir().join(format!("conduit-host-config-{}", std::process::id()));
        let path = directory.join("created.toml");
        let mut create_input = Cursor::new("created\n1\n1,2\nv\ns\n");
        let mut create_output = Vec::new();
        configure(&mut create_input, &mut create_output, Some(&path)).unwrap();
        assert!(String::from_utf8(create_output)
            .unwrap()
            .contains("VALID sha256:"));
        let mut edit_input = Cursor::new("e\nrenamed\nn\nn\ns\n");
        let mut edit_output = Vec::new();
        configure(&mut edit_input, &mut edit_output, Some(&path)).unwrap();
        let saved = std::fs::read_to_string(&path).unwrap();
        assert!(saved.contains("name = \"renamed\""));
        std::fs::remove_dir_all(directory).unwrap();
    }
}
