use patchbay_html::{
    body_workbench_fixture_snapshot, body_workbench_snapshot, cross_host_demonstration_snapshot,
    demonstration_snapshot, llm_documentary_snapshot, llm_embodiment_snapshot, load_seed_sources,
    text_lab_split_snapshot, BrowserBodyWorkbenchEntrance, PatchbayHtmlServer, SeedSource,
};

#[derive(Debug, Default, PartialEq, Eq)]
struct Arguments {
    documentary_fixture: bool,
    debugger_watch_fixture: bool,
    llm_documentary_fixture: bool,
    llm_embodiment_fixture: Option<usize>,
    text_lab_split: Option<String>,
    seeds: Vec<SeedSource>,
    body_evidence: Option<std::path::PathBuf>,
    body_entrance: Option<BrowserBodyWorkbenchEntrance>,
    body_workbench_fixture: Option<bool>,
}

fn parse_arguments(arguments: impl Iterator<Item = String>) -> Result<Arguments, String> {
    let mut arguments = arguments.peekable();
    let mut parsed = Arguments::default();
    while let Some(argument) = arguments.next() {
        match argument.as_str() {
            "--debugger-watch-fixture" if parsed == Arguments::default() => {
                parsed.debugger_watch_fixture = true;
            }
            "--documentary-fixture"
                if !parsed.debugger_watch_fixture
                    && !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.llm_embodiment_fixture.is_none()
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.documentary_fixture = true;
            }
            "--llm-documentary-fixture"
                if !parsed.debugger_watch_fixture
                    && !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.llm_embodiment_fixture.is_none()
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.llm_documentary_fixture = true;
            }
            "--llm-embodiment-fixture"
                if !parsed.debugger_watch_fixture
                    && !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.llm_embodiment_fixture.is_none()
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.llm_embodiment_fixture = Some(
                    arguments
                        .next()
                        .ok_or("--llm-embodiment-fixture requires stage 0, 1, or 2")?
                        .parse()
                        .map_err(|_| "--llm-embodiment-fixture requires stage 0, 1, or 2")?,
                );
            }
            "--text-lab-split"
                if !parsed.debugger_watch_fixture
                    && !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.llm_embodiment_fixture.is_none()
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
                if !parsed.debugger_watch_fixture
                    && !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.llm_embodiment_fixture.is_none()
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
            "--body-evidence"
                if parsed.body_evidence.is_none()
                    && parsed.body_workbench_fixture.is_none()
                    && !parsed.debugger_watch_fixture
                    && !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.llm_embodiment_fixture.is_none()
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.body_evidence = Some(
                    arguments
                        .next()
                        .ok_or("--body-evidence requires a serialized evidence path")?
                        .into(),
                );
            }
            "--body-workbench-fixture"
                if parsed.body_workbench_fixture.is_none()
                    && parsed.body_evidence.is_none()
                    && !parsed.debugger_watch_fixture
                    && !parsed.documentary_fixture
                    && !parsed.llm_documentary_fixture
                    && parsed.llm_embodiment_fixture.is_none()
                    && parsed.text_lab_split.is_none()
                    && parsed.seeds.is_empty() =>
            {
                parsed.body_workbench_fixture = Some(match arguments.next().as_deref() {
                    Some("hosted") => true,
                    Some("external") => false,
                    _ => return Err("--body-workbench-fixture requires hosted or external".into()),
                });
            }
            "--external-reader"
                if parsed.body_evidence.is_some() && parsed.body_entrance.is_none() =>
            {
                parsed.body_entrance = Some(BrowserBodyWorkbenchEntrance::ExternalReader);
            }
            "--hosted-reader"
                if parsed.body_evidence.is_some() && parsed.body_entrance.is_none() =>
            {
                parsed.body_entrance = Some(BrowserBodyWorkbenchEntrance::Hosted {
                    plan_id: arguments
                        .next()
                        .ok_or("--hosted-reader requires exact Plan and implementation IDs")?,
                    implementation_id: arguments
                        .next()
                        .ok_or("--hosted-reader requires exact Plan and implementation IDs")?,
                });
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
    let server = if arguments.debugger_watch_fixture {
        let snapshot = demonstration_snapshot()?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else if let Some(hosted) = arguments.body_workbench_fixture {
        let snapshot =
            body_workbench_fixture_snapshot(hosted).map_err(|error| error.to_string())?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else if let Some(path) = arguments.body_evidence {
        let entrance = arguments.body_entrance.ok_or(
            "--body-evidence requires exactly one --external-reader or --hosted-reader entrance",
        )?;
        let evidence = std::fs::read(path).map_err(|error| error.to_string())?;
        let snapshot =
            body_workbench_snapshot(1, &evidence, entrance).map_err(|error| error.to_string())?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else if arguments.body_entrance.is_some() {
        return Err("a Body reader entrance requires --body-evidence first".into());
    } else if arguments.documentary_fixture {
        let snapshot = cross_host_demonstration_snapshot().map_err(|error| error.to_string())?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else if arguments.llm_documentary_fixture {
        let snapshot = llm_documentary_snapshot()?;
        PatchbayHtmlServer::bind_ephemeral(&snapshot).map_err(|error| error.to_string())?
    } else if let Some(stage) = arguments.llm_embodiment_fixture {
        let snapshot = llm_embodiment_snapshot(stage)?;
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
        let external = parse_arguments(
            ["--body-evidence", "roseau.json", "--external-reader"]
                .into_iter()
                .map(str::to_owned),
        )
        .unwrap();
        assert_eq!(
            external.body_evidence.as_deref(),
            Some(std::path::Path::new("roseau.json"))
        );
        assert!(matches!(
            external.body_entrance,
            Some(BrowserBodyWorkbenchEntrance::ExternalReader)
        ));
        let hosted = parse_arguments(
            [
                "--body-evidence",
                "roseau.json",
                "--hosted-reader",
                "plan/roseau-patchbay",
                "browser/patchbay-surface@1",
            ]
            .into_iter()
            .map(str::to_owned),
        )
        .unwrap();
        assert!(matches!(
            hosted.body_entrance,
            Some(BrowserBodyWorkbenchEntrance::Hosted { .. })
        ));
    }
}
