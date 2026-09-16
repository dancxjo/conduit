use super::*;

const FORMS: [&str; 4] = ["Keyboard canvas", "Memory Lantern", "Tour", "Patchbay"];

#[test]
fn launcher_navigation_is_finite() {
    let mut home = HomeModel::new();
    assert_eq!(home.selected_destination(), HomeDestination::Tour);
    home.accept(HomeEvent::Previous, &FORMS);
    assert_eq!(home.selected_destination(), HomeDestination::Prompt);
    home.accept(HomeEvent::Next, &FORMS);
    assert_eq!(home.selected_destination(), HomeDestination::Tour);
}

#[test]
fn text_and_voice_share_the_exact_command_path() {
    let mut home = HomeModel::new();
    assert_eq!(home.submit_text("help", &FORMS), HomeAction::Changed);
    assert_eq!(home.view(), HomeView::Prompt);
    assert!(home.output().contains("open <place>"));
    assert_eq!(
        home.submit_text("run memory lantern", &FORMS),
        HomeAction::RunForm(1)
    );
    assert_eq!(
        home.submit_text("run definitely absent", &FORMS),
        HomeAction::Changed
    );
    assert_eq!(home.output(), "No installed Form named definitely absent.");
}

#[test]
fn creche_is_an_explicit_host_request() {
    let mut home = HomeModel::new();
    assert_eq!(
        home.submit_text("open creche", &FORMS),
        HomeAction::OpenCreche
    );
}

#[test]
fn command_storage_is_bounded_without_splitting_unicode() {
    let mut home = HomeModel::new();
    home.accept(HomeEvent::Text("é"), &FORMS);
    for _ in 0..MAX_COMMAND_BYTES {
        home.accept(HomeEvent::Text("é"), &FORMS);
    }
    assert!(home.command().len() <= MAX_COMMAND_BYTES);
    assert!(core::str::from_utf8(home.command().as_bytes()).is_ok());
}

#[test]
fn journey_identity_is_complete_and_unique() {
    assert_eq!(JOURNEY_STEP_IDS.len(), 8);
    for (index, step) in JOURNEY_STEP_IDS.iter().enumerate() {
        assert!(step.contains('.'));
        assert!(!JOURNEY_STEP_IDS[..index].contains(step));
    }
}

#[test]
fn every_home_view_lowers_to_the_shared_application_contract() {
    let mut home = HomeModel::new();
    let launcher = home.presentation(1, &FORMS).lower().unwrap();
    assert_eq!(launcher.actions.len(), HOME_ITEM_COUNT);
    assert_eq!(launcher.actions[0].id, OPEN_TOUR_ACTION_ID);

    home.submit_text("forms", &FORMS);
    let forms = home.presentation(2, &FORMS).lower().unwrap();
    assert_eq!(forms.actions.len(), FORMS.len());

    home.submit_text("open prompt", &FORMS);
    let prompt = home.presentation(3, &FORMS).lower().unwrap();
    assert!(prompt.nodes.iter().any(|node| node.key == "command-result"));
}

#[test]
fn inspect_resolves_real_inventory_or_refuses_without_success_action() {
    let mut home = HomeModel::new();
    assert_eq!(
        home.submit_text("inspect memory lantern", &FORMS),
        HomeAction::OpenForm(1)
    );
    assert_eq!(
        home.submit_text("inspect missing thing", &FORMS),
        HomeAction::Changed
    );
    assert_eq!(
        home.output(),
        "Cannot inspect unresolved subject missing thing."
    );
}

#[test]
fn category_and_lifecycle_commands_refuse_honestly_on_an_unprivileged_face() {
    let mut home = HomeModel::new();
    home.submit_text("hosts", &FORMS);
    let hosts = String::from(home.output());
    home.submit_text("lines", &FORMS);
    let lines = String::from(home.output());
    assert_ne!(hosts, lines);
    assert!(hosts.contains("unavailable"));
    assert!(lines.contains("unavailable"));

    assert_eq!(home.submit_text("wake", &FORMS), HomeAction::Changed);
    assert_eq!(
        home.output(),
        "Wake is unavailable on this Home face; no lifecycle authority is attached."
    );
}

#[test]
fn help_documents_effects_and_unavailable_commands() {
    let mut home = HomeModel::new();
    home.submit_text("help", &FORMS);
    assert!(home.output().contains("inspect <installed form>|body"));
    assert!(home.output().contains("hosts/lines/wake (unavailable"));
}
