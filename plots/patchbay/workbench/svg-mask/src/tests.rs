use super::*;

fn checked(source: &str) -> ExpandedAuthoringPlot {
    let mut startup = conduit_plot::StartupCatalog::new();
    let mut profiles = conduit_plot::ProfileCatalog::new();
    conduit_semantic_catalog::install_text_pipeline_catalogs(&mut startup, &mut profiles).unwrap();
    let checked =
        conduit_plot::check_syntax_document(&conduit_plot::parse_syntax_document(source), &startup)
            .unwrap();
    let entry = checked.plots.last().unwrap().name.clone();
    conduit_plot::expand_canonical_plot_for_authoring(&checked, &entry, &profiles).unwrap()
}

#[test]
fn svg_connects_exact_ports_and_escapes_labels() {
    let plot = checked("plot main (\n  >> text: Text\n  shown: Text >>\n) {\n  pass: text/join(\"&\")\n  text >> pass >> shown\n}\n");
    let svg = render_svg(&plot, &BTreeMap::new());
    assert!(svg.starts_with("<svg"));
    assert!(svg.contains("Plot inputs"));
    assert!(svg.contains("text"));
    assert!(svg.contains("marker-end=\"url(#arrow)\""));
    assert!(svg.contains(">pass</text>"));
    assert!(svg.contains("main/pass — kind text/join"));
    assert!(svg.contains("class=\"port-name\""));
    assert!(!svg.contains("class=\"port-kind\""));
    assert!(svg.contains("Plot.shown · value/text · payload"));
    assert!(svg.contains(">value/text</text>"));
    assert!(svg.contains("class=\"cord-tag\""));
    assert!(!svg.contains(">payload</text>"));
    assert!(svg.contains(".canvas{fill:var(--conduit-background,var(--diagram-background))}"));
    assert!(svg.contains(".boundary{fill:var(--conduit-surface,var(--diagram-surface))"));
    assert!(svg.contains("fill=\"var(--conduit-structure-primary,var(--diagram-structure))\""));
    assert!(!svg.contains(">text : value/text</text>"));
    assert!(svg.contains("q0,-10 10,-10"));
    assert!(svg.contains("width=\"1460\""));
    assert!(svg.contains("href=\"#gear-mark\""));
    assert!(!svg.contains("class=\"direction\""));
    assert!(svg.contains("class=\"port input-port\""));
    assert!(svg.contains("class=\"port output-port\""));
    assert!(svg.contains(" H"));
    assert!(svg.contains(" V"));
    assert!(!svg.contains("text/join(\"&\")"));
}

#[test]
fn mermaid_labels_cords_once_with_information_kind() {
    let plot = checked("plot main (\n  >> text: Text\n  shown: Text >>\n) {\n  pass: text/join(\" \" )\n  text >> pass >> shown\n}\n");
    let mermaid = render_mermaid(&plot, &BTreeMap::new());
    assert_eq!(mermaid.matches("-- \"value/text\" -->").count(), 2);
    assert!(!mermaid.contains("text → text"));
    assert!(mermaid.contains("<small>text/join</small>"));
}

#[test]
fn svg_includes_every_expanded_gear_and_internal_cord() {
    let plot = checked("plot main (\n  >> text: Text\n  shown: Text >>\n) {\n  first: text/join(\" \" )\n  second: text/join(\" \" )\n  text >> first >> second >> shown\n}\n");
    let svg = render_svg(&plot, &BTreeMap::new());
    assert!(svg.contains("first</text>"));
    assert!(svg.contains("second</text>"));
    assert_eq!(svg.matches("marker-end=\"url(#arrow)\"").count(), 3);
}

#[test]
fn junction_dots_distinguish_semantic_branches_from_crossings() {
    let mut branched = String::new();
    draw_cord(
        &mut branched,
        Point { x: 10, y: 20 },
        Point { x: 200, y: 80 },
        ConnectionTrack::Payload,
        "image".into(),
        "source.image → sink.image · structured-info/profile-image",
        0,
        true,
        true,
        660,
        true,
    );
    assert!(branched.contains("class=\"split-junction\""));
    assert!(branched.contains("class=\"join-junction\""));

    let mut crossing = String::new();
    draw_cord(
        &mut crossing,
        Point { x: 10, y: 20 },
        Point { x: 200, y: 80 },
        ConnectionTrack::Payload,
        "image".into(),
        "source.image → sink.image · structured-info/profile-image",
        0,
        false,
        false,
        660,
        true,
    );
    assert!(!crossing.contains("junction"));

    let mut straight = String::new();
    draw_cord(
        &mut straight,
        Point { x: 10, y: 20 },
        Point { x: 200, y: 20 },
        ConnectionTrack::Payload,
        "image".into(),
        "source.image → sink.image · structured-info/profile-image",
        0,
        false,
        false,
        660,
        true,
    );
    assert!(straight.contains("d=\"M10,20 H200\""));
    assert!(!straight.contains(" V"));

    let mut long_span = String::new();
    draw_cord(
        &mut long_span,
        Point { x: 10, y: 240 },
        Point { x: 1000, y: 420 },
        ConnectionTrack::Payload,
        "image".into(),
        "source.image → sink.image · structured-info/profile-image",
        0,
        false,
        false,
        660,
        true,
    );
    assert!(long_span.contains("V100"));
}

#[test]
fn information_labels_use_types_not_port_names() {
    assert_eq!(
        information_label("value/text", &BTreeMap::new()),
        "value/text"
    );
    assert_eq!(
            information_label(
                "structured-info/profile-5499281c7145915c96904daf22ba0da93fcd03c7edb3c39ed767bb31ab8b9fdc@1",
                &BTreeMap::new(),
            ),
            "profile-5499281c…@1"
        );
    let names = BTreeMap::from([(
            "structured-info/profile-5499281c7145915c96904daf22ba0da93fcd03c7edb3c39ed767bb31ab8b9fdc@1"
                .to_string(),
            "ImageResource".to_string(),
        )]);
    assert_eq!(
            information_label(
                "structured-info/profile-5499281c7145915c96904daf22ba0da93fcd03c7edb3c39ed767bb31ab8b9fdc@1",
                &names,
            ),
            "ImageResource"
        );
}

#[test]
fn plot_boundary_face_shows_port_name_without_repeating_its_type() {
    let mut svg = String::new();
    draw_boundary(
        &mut svg,
        "Plot inputs",
        10,
        20,
        &[conduit_core::PortDescriptor {
            port_id: conduit_core::PortId::from("image"),
            value_kind: conduit_core::KindId::from("vision/image"),
            direction: conduit_core::PortDirection::Input,
            temporal: conduit_core::PortTemporal::Value,
            abnormal_kind: None,
        }],
        true,
    );
    assert!(svg.contains(">image</text>"));
    assert!(!svg.contains(">image : vision/image</text>"));
    assert!(svg.contains("<title>image: vision/image</title>"));
}
