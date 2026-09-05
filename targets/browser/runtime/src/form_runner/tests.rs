use super::*;

const HELLO_LIGHT: &str = r#"form hello-light {
    message: text/literal("SOS")
    morse: text/morse(120)
    light: presentation/indicator

    message > morse > light
}
"#;

fn manifestation(effect: TourHostEffect) -> TourEffect {
    match effect {
        TourHostEffect::Manifestation(effect) => *effect,
        TourHostEffect::Timer(_) => panic!("the fixture must manifest before requesting a timer"),
        TourHostEffect::KeyEvent(_) => panic!("the fixture must manifest before requesting input"),
    }
}

#[test]
fn chapter_one_runs_inside_the_generic_browser_envelope() {
    let (session, effect) = TourSession::prepare(
        "browser/book-test",
        "browser-boot/book-test",
        HELLO_LIGHT,
        1,
    )
    .unwrap();
    let effect = manifestation(effect);
    assert_eq!(effect.unit_millis, 120);
    assert_eq!(effect.segments.len(), 17);
    assert_eq!(effect.host_id, "browser/book-test");
    assert_eq!(session.cancel().unwrap().disposition, "cancelled");
}

#[test]
fn four_gear_text_form_runs_without_a_topology_special_case() {
    let source = r#"form text-chain {
    source: text/literal("hello")
    prefix: text/join("say: ")
    upper: text/upper
    result: presentation/text

    source > prefix > upper > result
}
"#;
    let (session, effect) =
        TourSession::prepare("browser/book-test", "browser-boot/book-test", source, 2).unwrap();
    let effect = manifestation(effect);
    assert_eq!(effect.text.as_deref(), Some("SAY: HELLO"));
    assert_eq!(session.complete().unwrap().disposition, "completed");
}

#[test]
fn linguistic_structured_info_runs_through_the_same_browser_envelope() {
    let source = r#"form language-lab {
    tokens: language/tokenize-four("Bright stars shine.")
    annotate: language/annotate-four
    result: presentation/structured-info

    tokens > annotate > result
}
"#;
    let (session, effect) =
        TourSession::prepare("browser/book-test", "browser-boot/book-test", source, 4).unwrap();
    let effect = manifestation(effect);
    assert!(effect
        .text
        .as_deref()
        .is_some_and(|text| text.starts_with("4 linguistic annotations")));
    assert_eq!(
        effect.presentation_kind,
        conduit_semantic_catalog::STRUCTURED_PRESENTATION_KIND
    );
    assert_eq!(session.complete().unwrap().disposition, "completed");
}

#[test]
fn math_and_logic_families_use_the_same_generic_host_path() {
    let math = r#"form math-lab {
    source: scalar/literal(1.5)
    scale: math/scale(2.0)
    result: presentation/scalar
    source > scale > result
}
"#;
    let (math_session, math_effect) =
        TourSession::prepare("browser/book-test", "browser-boot/book-test", math, 5).unwrap();
    let math_effect = manifestation(math_effect);
    assert_eq!(math_effect.text.as_deref(), Some("3.000000"));
    assert_eq!(math_session.complete().unwrap().disposition, "completed");

    let logic = r#"form logic-lab {
    source: boolean/literal(true)
    invert: logic/not
    result: presentation/bool-value
    source > invert > result
}
"#;
    let (logic_session, logic_effect) =
        TourSession::prepare("browser/book-test", "browser-boot/book-test", logic, 6).unwrap();
    let logic_effect = manifestation(logic_effect);
    assert_eq!(logic_effect.text.as_deref(), Some("false"));
    assert_eq!(logic_session.complete().unwrap().disposition, "completed");
}

#[test]
fn typed_fanout_reconverges_without_book_topology_code() {
    let source = r#"form fanout-lab {
    source: scalar/literal(0.5)
    scaled: math/scale(2.0)
    quiet: math/deadband(0.6)
    compare: logic/compare("gt")
    result: presentation/bool-value

    source > scaled > compare.left
    source > quiet > compare.right
    compare.out > result
}
"#;
    let (session, effect) =
        TourSession::prepare("browser/book-test", "browser-boot/book-test", source, 7).unwrap();
    let effect = manifestation(effect);
    assert_eq!(effect.text.as_deref(), Some("true"));
    assert_eq!(effect.expanded_gears.len(), 5);
    assert_eq!(session.complete().unwrap().disposition, "completed");
}

#[test]
fn finite_browser_timer_drives_current_count_through_one_kernel_play() {
    let source = r#"form count-over-time {
    count: state/count(start = 0)
    show: presentation/count(maximum-values = 5)
    clock: time/every(freq = 100ms)

    clock.tick > count.bump
    count.value > show.value
}
"#;
    let first = state_time_trace(source);
    let second = state_time_trace(source);
    assert_eq!(first, second);
    assert_eq!(
        first
            .0
            .iter()
            .filter(|event| event.starts_with("manifestation:"))
            .collect::<Vec<_>>(),
        vec![
            "manifestation:0:0",
            "manifestation:1:1",
            "manifestation:2:2",
            "manifestation:3:3",
            "manifestation:4:4",
        ]
    );
    assert_eq!(
        first
            .0
            .iter()
            .filter(|event| event.starts_with("timer:"))
            .count(),
        conduit_time::TIME_EVERY_COUNT as usize
    );
    assert!(first
        .0
        .iter()
        .filter(|event| event.starts_with("timer:"))
        .all(|event| event.split(':').nth(1) == Some("100")));
    assert_eq!(first.1, (4, 5));

    let too_long = source.replace("100ms", "10001ms");
    let refusal = TourSession::prepare(
        "browser/book-state-time",
        "browser-boot/book-state-time",
        &too_long,
        20,
    )
    .err()
    .map(|error| refusal(error).category)
    .expect("an unadmitted browser timer duration refuses before Play");
    assert_eq!(refusal, "browser-bound");
}

fn state_time_trace(source: &str) -> (Vec<String>, (u32, u32)) {
    let (mut session, mut effect) = TourSession::prepare(
        "browser/book-state-time",
        "browser-boot/book-state-time",
        source,
        19,
    )
    .unwrap();
    let mut trace = Vec::new();
    loop {
        match effect {
            TourHostEffect::Manifestation(ref manifestation) => trace.push(format!(
                "manifestation:{}:{}",
                manifestation.text.as_deref().unwrap_or("missing"),
                manifestation.observation_sequence
            )),
            TourHostEffect::Timer(ref timer) => trace.push(format!(
                "timer:{}:{}",
                timer.duration_millis, timer.request_sequence
            )),
            TourHostEffect::KeyEvent(_) => panic!("timer fixture requested keyboard input"),
        }
        match session.advance().unwrap() {
            TourProgress::Effect(next) => effect = *next,
            TourProgress::Receipt(receipt) => {
                assert_eq!(receipt.disposition, "completed");
                return (
                    trace,
                    (receipt.timer_completions, receipt.manifestation_completions),
                );
            }
        }
    }
}

#[test]
fn pending_browser_timer_cancels_without_becoming_a_completed_tick() {
    let source = r#"form count-over-time {
    count: state/count(start = 0)
    show: presentation/count(maximum-values = 5)
    clock: time/every(freq = 100ms)

    clock.tick > count.bump
    count.value > show.value
}
"#;
    let (session, initial) = TourSession::prepare(
        "browser/book-state-time-cancel",
        "browser-boot/book-state-time-cancel",
        source,
        21,
    )
    .unwrap();
    assert!(matches!(initial, TourHostEffect::Timer(_)));
    let receipt = session.cancel().unwrap();
    assert_eq!(receipt.disposition, "cancelled");
    assert_eq!(receipt.timer_completions, 0);
    assert_eq!(receipt.manifestation_completions, 0);
}

#[test]
fn unchanged_morse_caller_substitutes_direct_and_nested_recursive_realizations() {
    let (direct_session, direct) = TourSession::prepare(
        "browser/book-test",
        "browser-boot/book-test",
        HELLO_LIGHT,
        10,
    )
    .unwrap();
    let direct = manifestation(direct);
    let (recursive_session, recursive) = TourSession::prepare_recursive(
        "browser/book-test",
        "browser-boot/book-test",
        HELLO_LIGHT,
        11,
    )
    .unwrap();
    let recursive = manifestation(recursive);

    assert_eq!(direct.source_document_id, recursive.source_document_id);
    assert_eq!(direct.checked_form_id, recursive.checked_form_id);
    assert_ne!(direct.expanded_form_id, recursive.expanded_form_id);
    assert_ne!(direct.plan_id, recursive.plan_id);
    assert_eq!(direct.segments, recursive.segments);
    assert_eq!(direct.unit_millis, recursive.unit_millis);
    assert_eq!(direct.realization, "direct");
    assert_eq!(recursive.realization, "recursive");
    assert!(direct.realization_backs.is_empty());
    assert_eq!(recursive.realization_backs.len(), 2);
    assert!(direct
        .expanded_gears
        .iter()
        .any(|gear| { gear.implementation_id == "browser/kernel-text-morse-direct@1" }));
    for kind in [
        conduit_text::TEXT_CHARACTERS_KIND,
        conduit_text::MORSE_LOOKUP_KIND,
        conduit_text::MORSE_INTERSPERSE_KIND,
        conduit_text::MORSE_FLATTEN_KIND,
        conduit_text::MORSE_SYMBOLS_TO_PATTERN_KIND,
    ] {
        assert!(recursive
            .expanded_gears
            .iter()
            .any(|gear| gear.kind_id == kind));
    }
    assert_eq!(direct_session.complete().unwrap().disposition, "completed");
    assert_eq!(
        recursive_session.complete().unwrap().disposition,
        "completed"
    );
}

#[test]
fn semantic_kind_without_browser_installation_refuses_before_play() {
    let source = r#"form missing-installation {
    source: text/literal("hello")
    result: presentation/text
    unavailable: layout/inset
    source > result
}
"#;
    let message = TourSession::prepare("browser/book-test", "browser-boot/book-test", source, 3)
        .err()
        .expect("uninstalled semantic Kind refuses");
    let refusal = refusal(message);
    assert_eq!(refusal.disposition, "refused-before-play");
    assert_eq!(refusal.category, "missing-implementation-or-placement");
}

#[test]
fn newly_installed_logic_select_executes_through_the_generic_host() {
    let select = r#"form browser-select {
    selector: boolean/literal(true)
    when_false: scalar/literal(1.0)
    when_true: scalar/literal(2.0)
    choose: logic/select
    show: presentation/scalar
    selector.value > choose.selector
    when_false.value > choose.when-false
    when_true.value > choose.when-true
    choose.out > show.value
}
"#;
    let (session, effect) =
        TourSession::prepare("browser/select", "browser/select-boot", select, 31).unwrap();
    assert_eq!(manifestation(effect).text.as_deref(), Some("2.000000"));
    assert_eq!(session.complete().unwrap().disposition, "completed");
}

#[test]
fn exact_type_mismatch_refuses_before_play_as_source_contract_error() {
    let source = r#"form wrong-type {
    source: scalar/literal(1.0)
    invert: logic/not
    result: presentation/bool-value
    source > invert > result
}
"#;
    let message = TourSession::prepare("browser/book-test", "browser-boot/book-test", source, 12)
        .err()
        .expect("mismatched exact ports refuse");
    assert_eq!(refusal(message).category, "type-or-source");
}

#[test]
fn browser_gear_bound_refuses_before_play() {
    let mut source = String::from("form too-large {\n source: text/literal(\"x\")\n");
    for index in 0..15 {
        source.push_str(&format!(" step{index}: text/upper\n"));
    }
    source.push_str(" result: presentation/text\n source");
    for index in 0..15 {
        source.push_str(&format!(" > step{index}"));
    }
    source.push_str(" > result\n}\n");
    let message = TourSession::prepare("browser/book-test", "browser-boot/book-test", &source, 13)
        .err()
        .expect("seventeen Gears exceed the exact sixteen-Gear profile");
    let refusal = refusal(message);
    assert_eq!(refusal.disposition, "refused-before-play");
    assert_eq!(refusal.category, "browser-bound");
}

#[test]
fn browser_cord_bound_refuses_before_play() {
    let mut source = String::from("form too-connected {\n source: scalar/literal(1.0)\n");
    for index in 0..13 {
        source.push_str(&format!(" compare{index}: logic/compare(\"gt\")\n"));
    }
    for index in 0..13 {
        source.push_str(&format!(
            " source > compare{index}.left\n source > compare{index}.right\n"
        ));
    }
    source.push_str("}\n");
    let message = TourSession::prepare("browser/book-test", "browser-boot/book-test", &source, 14)
        .err()
        .expect("twenty-six Cords exceed the exact twenty-four-Cord profile");
    assert_eq!(refusal(message).category, "browser-bound");
}

#[test]
fn semantic_value_bound_refuses_before_play() {
    let oversized = "x".repeat(conduit_text::MAX_TEXT_BYTES as usize + 1);
    let source = format!(
        "form too-much-text {{\n source: text/literal(\"{oversized}\")\n result: presentation/text\n source > result\n}}\n"
    );
    let message = TourSession::prepare("browser/book-test", "browser-boot/book-test", &source, 15)
        .err()
        .expect("text beyond its exact semantic value bound refuses");
    assert!(message.contains("CND-FRM-040"));
    assert_eq!(refusal(message).category, "type-or-source");
}

#[test]
fn missing_resource_and_authority_are_distinct_pre_play_refusals() {
    let (startup, catalog) = catalogs().unwrap();
    let checked = conduit_form::check_syntax_document(
        &conduit_form::parse_syntax_document(HELLO_LIGHT),
        &startup,
    )
    .unwrap();
    let expanded = conduit_form::expand_canonical_form(&checked, "hello-light", &catalog).unwrap();

    let mut without_resource = advertisement(
        conduit_core::HostId::from("browser/book-test"),
        conduit_core::BootId::from("browser-boot/book-test"),
    );
    without_resource.resources.clear();
    let resource_error = planning_error(&expanded, &without_resource);
    assert_eq!(refusal(resource_error).category, "resource");

    let mut without_authority = advertisement(
        conduit_core::HostId::from("browser/book-test"),
        conduit_core::BootId::from("browser-boot/book-test"),
    );
    let indicator = without_authority
        .capabilities
        .iter_mut()
        .find(|offer| {
            offer.kind_id.as_str() == conduit_semantic_catalog::INDICATOR_PRESENTATION_KIND
        })
        .unwrap();
    indicator.authority_requirements = vec![conduit_core::present_authority_requirement(
        indicator.kind_id.clone(),
    )];
    let authority_error = planning_error(&expanded, &without_authority);
    assert_eq!(refusal(authority_error).category, "authority");
}

fn planning_error(
    expanded: &conduit_form::ExpandedCanonicalForm,
    host: &conduit_core::HostAdvertisement,
) -> String {
    let hosts = core::slice::from_ref(host);
    let placements = match default_expanded_placements(expanded, hosts) {
        Ok(placements) => placements,
        Err(error) => return format!("place negative-proof Form: {error:?}"),
    };
    plan_expanded_canonical_with_options(
        expanded,
        hosts,
        &placements,
        &local_bases(),
        PlanningOptions {
            connection_bases: &BTreeMap::new(),
            line_candidates: &BTreeMap::new(),
            connection_item_capacity: 1,
            connection_byte_capacity: crate::installed_browser::MAXIMUM_BROWSER_VALUE_BYTES as u32,
            authority_grants: &[],
            protected_resource_grants: &[],
            line_offers: &[],
        },
    )
    .err()
    .map(|error| format!("plan negative-proof Form: {error:?}"))
    .expect("negative proof must refuse")
}
