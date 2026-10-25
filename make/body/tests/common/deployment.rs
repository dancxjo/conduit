use conduit_birth_plot::{BirthDraft, BirthPlotChoice};
use conduit_body::ResidentPlot;
use conduit_body_make::*;
use conduit_face::ApplicationEventKind;
use conduit_host_make::{parse_host_configuration_conduit, SporeOutputKind};
use conduit_workspace_make::{catalog, package_set};
use std::collections::BTreeMap;

pub fn encounter(name: &str) -> DeploymentDraft {
    encounter_with_memory(name, 64 * 1024 * 1024)
}

pub fn encounter_with_memory(name: &str, heap_bytes: u64) -> DeploymentDraft {
    let mut config = parse_host_configuration_conduit(include_str!(
        "../../../../targets/std/profiles/linux-computer.host.conduit"
    ))
    .unwrap();
    config.limits.heap_arena_bytes = heap_bytes;
    let description = BodyDescription {
        schema: BODY_DESCRIPTION_SCHEMA,
        name: "new-body".into(),
        body: BodyBindingTarget {
            id: "body:construction-intent".into(),
        },
        hosts: ["first", "second"]
            .into_iter()
            .map(|name| BodyHostDescription {
                name: name.into(),
                part: Some(format!("part:{name}")),
                configuration: "linux.host.conduit".into(),
                spore: SporeDescription {
                    join_mode: SporeJoinMode::Prejoined,
                    output: SporeOutputKind::NativeBundle,
                    invitation: None,
                },
                deployment: None,
            })
            .collect(),
        mask_imports: vec![],
        wardrobe: BodyWardrobeDescription::default(),
    };
    let checked = check_body_description(
        description,
        &BTreeMap::from([("linux.host.conduit".into(), config)]),
        &catalog(),
        &package_set(),
    )
    .unwrap();
    let birth = BirthDraft::new(
        "01234567-89ab-4cde-8fab-0123456789ab".into(),
        vec![BirthPlotChoice {
            title: "Notes".into(),
            search_text: "notes".into(),
            plot: ResidentPlot::new("source/notes".into(), "checked/notes".into()),
            refusal: None,
            selected: true,
        }],
    )
    .unwrap();
    let mut draft = DeploymentDraft::new(birth, checked, "first".into()).unwrap();
    draft
        .apply_birth_event(
            draft.revision(),
            "creche.name",
            ApplicationEventKind::Input,
            name,
        )
        .unwrap();
    draft
}
