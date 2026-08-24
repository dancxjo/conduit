use patchbay_html::{
    cross_host_demonstration_snapshot, llm_documentary_snapshot, load_seed_sources,
    text_lab_split_snapshot, PatchbayHtmlServer, SeedSource,
};

#[derive(Debug, Default, PartialEq, Eq)]
struct Arguments {
    documentary_fixture: bool,
    llm_documentary_fixture: bool,
    text_lab_split: Option<String>,
    seeds: Vec<SeedSource>,
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut arguments = arguments.peekable();
    let mut parsed = Arguments::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--documentary-fixture"
                if !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.documentary_fixture = true;
            }
            "--llm-documentary-fixture"
                if !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.llm_documentary_fixture = true;
            }
            "--text-lab-split"
                if !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.text_lab_split = Some(
                    arguments
                        .next()
                        .ok_or("--text-lab-split requires one loopback WebSocket base")?,
                );
            }
            "--seed"
                if !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.text_lab_split.is_none() =>
            {
                let label = arguments
                    .next()
                    .ok_or("--seed requires a label and canonical .conduit path")?;
                let path = arguments
                    .next()
                    .ok_or("--seed requires a label and canonical .conduit path")?;
                parsed.seeds.push(SeedSource::new(label, path));
            }
            _ => {
                return Err(format!(
                    "unknown or incompatible Patchbay HTML argument {argument}; expected repeated --seed <LABEL> <PATH>"
                ));
            }
        }
    }
    Ok(parsed)
}

fn main() -> Result<(), String> {
    let arguments = parse_arguments(std::env::args().skip(1))?;
    let server = if arguments.documentary_fixture {
        let snapshot = cross_host_demonstration_snapshot().map_err(|error| error.to_string())?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else if arguments.llm_documentary_fixture {
        let snapshot = llm_documentary_snapshot()?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else if let Some(base) = arguments.text_lab_split {
        let snapshot = text_lab_split_snapshot(&base)?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot)
            .map_err(|error| error.to_string())?
            .with_text_lab_loss_updates(base)
            .map_err(|error| error.to_string())?
    } else {
        let seeds = load_seed_sources(&arguments.seeds).map_err(|error| error.to_string())?;
        PatchbayHtmlServer::bind_browser_front_door_with_seeds_ephemeral(seeds)
            .map_err(|error| error.to_string())?
    };
    println!(
        "PATCHBAY_HTML_URL=http://{}",
        server.local_addr().map_err(|error| error.to_string())?
    );
    server.serve().map_err(|error| error.to_string())?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn arguments_admit_only_explicit_ordered_canonical_seed_bindings() {
        let parsed = parse_arguments(
            [
                "--seed",
                "Text Lab",
                "examples/text-lab.conduit",
                "--seed",
                "Hello",
                "examples/hello.conduit",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(parsed.seeds.len(), 2);
        assert_eq!(parsed.seeds[0].label, "Text Lab");
        assert_eq!(
            parsed.seeds[1].path,
            std::path::Path::new("examples/hello.conduit")
        );
        assert_eq!(
            parse_arguments(["--seed", "Text Lab"].into_iter().map(str::to_owned)),
            Err("--seed requires a label and canonical .conduit path".into())
        );
        assert!(parse_arguments(
            [
                "--documentary-fixture",
                "--seed",
                "Hello",
                "examples/hello.conduit"
            ]
            .into_iter()
            .map(str::to_owned)
        )
        .is_err());
        assert!(parse_arguments(
            ["--llm-documentary-fixture", "--documentary-fixture"]
                .into_iter()
                .map(str::to_owned)
        )
        .is_err());
        assert!(parse_arguments(
            [
                "--llm-documentary-fixture",
                "--seed",
                "Hello",
                "examples/hello.conduit"
            ]
            .into_iter()
            .map(str::to_owned)
        )
        .is_err());
        assert_eq!(
            parse_arguments(
                ["--text-lab-split", "ws://127.0.0.1:1/conduit"]
                    .into_iter()
                    .map(str::to_owned)
            )
            .unwrap()
            .text_lab_split
            .as_deref(),
            Some("ws://127.0.0.1:1/conduit")
        );
    }
}
