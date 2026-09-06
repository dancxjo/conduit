use std::{
    fs,
    path::PathBuf,
    sync::atomic::{AtomicUsize, Ordering},
};

use conduit_body_fabrication::{parse_body_description_conduit, SporeJoinMode};

use super::*;

static NEXT_TEMP: AtomicUsize = AtomicUsize::new(0);

#[test]
fn robot_template_and_explicit_host_generate_checked_canonical_source() {
    let root = workspace_root().unwrap();
    let output = temp_path("robot").join("pete.body.conduit");
    let eyes = "eyes=browser-page".parse().unwrap();
    let prepared = prepare(
        &root,
        "pete",
        Some(BodyTemplate::Robot),
        &[eyes],
        &[],
        &output,
    )
    .unwrap();
    let parsed = parse_body_description_conduit(&prepared.source).unwrap();
    assert_eq!(parsed.body.id, "body:pete");
    assert_eq!(parsed.hosts.len(), 3);
    assert!(parsed.hosts.iter().all(|host| host.part.is_some()));
    assert!(prepared.source.find("brainstem").unwrap() < prepared.source.find("eyes").unwrap());
    assert!(prepared.source.find("eyes").unwrap() < prepared.source.find("forebrain").unwrap());
}

#[test]
fn explicit_hosts_replace_the_implicit_minimal_template() {
    let composition = compose(
        None,
        &[
            "forebrain=linux-computer".parse().unwrap(),
            "brainstem=pico-w".parse().unwrap(),
        ],
    )
    .unwrap();
    assert_eq!(composition.seeds.len(), 2);
    assert!(!composition.seeds.iter().any(|seed| seed.name == "main"));
}

#[test]
fn explicit_hosts_override_template_entries_and_duplicate_flags_are_refused() {
    let composition = compose(
        Some(BodyTemplate::Robot),
        &["brainstem=browser-page".parse().unwrap()],
    )
    .unwrap();
    assert_eq!(
        composition
            .seeds
            .iter()
            .find(|seed| seed.name == "brainstem")
            .unwrap()
            .configuration,
        "browser-page"
    );
    assert!(compose(
        None,
        &[
            "same=pico-w".parse().unwrap(),
            "same=browser-page".parse().unwrap()
        ]
    )
    .is_err());
}

#[test]
fn distributed_template_preserves_self_joining_truth() {
    let composition = compose(Some(BodyTemplate::Distributed), &[]).unwrap();
    let peer = composition
        .seeds
        .iter()
        .find(|seed| seed.name == "peer")
        .unwrap();
    assert!(matches!(peer.join_mode, TemplateJoinMode::SelfJoining));

    let root = workspace_root().unwrap();
    let output = temp_path("distributed").join("mesh.body.conduit");
    let prepared = prepare(
        &root,
        "mesh",
        Some(BodyTemplate::Distributed),
        &[],
        &[],
        &output,
    )
    .unwrap();
    let parsed = parse_body_description_conduit(&prepared.source).unwrap();
    let peer = parsed
        .hosts
        .iter()
        .find(|host| host.name == "peer")
        .unwrap();
    assert_eq!(peer.spore.join_mode, SporeJoinMode::SelfJoining);
    assert_eq!(
        peer.spore.invitation.as_deref(),
        Some("invitation:mesh-peer:single-use")
    );
    assert!(peer.part.is_none());
}

#[test]
fn dry_run_does_not_create_the_destination_and_collision_is_refused() {
    let directory = temp_path("writes");
    let output = directory.join("new.body.conduit");
    create(
        Some("new"),
        None,
        &[],
        Some(&output),
        false,
        &GlobalOpts {
            dry_run: true,
            quiet: true,
            ..GlobalOpts::default()
        },
    )
    .unwrap();
    assert!(!output.exists());

    write_new(&output, b"first", "Body description").unwrap();
    let error = write_new(&output, b"second", "Body description")
        .unwrap_err()
        .to_string();
    assert!(error.contains("refusing to replace existing Body description"));
    fs::remove_dir_all(&directory).unwrap();
}

#[test]
fn automation_without_a_name_is_refused_before_any_write() {
    let error = create(None, None, &[], None, true, &GlobalOpts::default())
        .unwrap_err()
        .to_string();
    assert!(error.contains("requires NAME outside an interactive terminal"));
}

#[test]
fn every_template_references_current_checked_host_recipes() {
    let recipes = load_host_recipes(&workspace_root().unwrap()).unwrap();
    let report = template_catalog(&recipes).unwrap();
    assert_eq!(report.templates.len(), 4);
    assert!(report.host_configurations.len() >= 3);
}

fn temp_path(label: &str) -> PathBuf {
    std::env::temp_dir().join(format!(
        "conduit-body-new-{}-{label}-{}",
        std::process::id(),
        NEXT_TEMP.fetch_add(1, Ordering::Relaxed)
    ))
}
