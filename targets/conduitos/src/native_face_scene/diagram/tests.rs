use super::*;

#[test]
fn notice_counts_capped_links_and_missing_endpoints_without_changing_face() {
    let mut face: Face = serde_json::from_str(include_str!(
        "../../../../proof/conformance/face-waist/domain-derived/patchbay-face.json"
    ))
    .unwrap();
    let cord = face
        .subjects
        .iter()
        .find(|s| s.role == FaceRole::Cord)
        .unwrap()
        .clone();
    let properties = face
        .properties
        .iter()
        .filter(|p| p.subject == cord.identity)
        .cloned()
        .collect::<Vec<_>>();
    let relationships = face
        .relationships
        .iter()
        .filter(|r| r.source == cord.identity)
        .cloned()
        .collect::<Vec<_>>();
    for index in 0..13 {
        let identity = alloc::format!("cord/overflow/{index}");
        let mut subject = cord.clone();
        subject.identity = identity.clone();
        face.subjects.push(subject);
        for property in &properties {
            let mut property = property.clone();
            property.subject = identity.clone();
            face.properties.push(property);
        }
        for relationship in &relationships {
            let mut relationship = relationship.clone();
            relationship.source = identity.clone();
            face.relationships.push(relationship);
        }
    }
    let screen = LayoutRect {
        x: 0,
        y: 0,
        width: 640,
        height: 720,
    };
    let retained = face.clone();
    assert!(
        link_notice(&face, screen, 0)
            .ends_with("12/15 cords shown · 3 omitted · F2 all facts · F3 return")
    );
    assert_eq!(face, retained);
    let checked = Face::new_with_semantics(
        face.revision,
        face.basis.clone(),
        face.subjects.clone(),
        face.relationships.clone(),
        face.properties.clone(),
        face.text.clone(),
        face.actions.clone(),
        face.disclosures.clone(),
    )
    .unwrap();
    let mut scene = NativeFaceScene::prepare(checked.clone(), 640, 720).unwrap();
    scene.show_diagram(true);
    let frame = scene.frame().unwrap();
    assert_eq!(
        frame
            .scene
            .commands()
            .iter()
            .filter(|command| command.kind == conduit_face::GraphicsCommandKind::OrthogonalPath)
            .count(),
        12
    );
    assert!(frame.scene.commands().iter().any(|command| {
        command
            .payload()
            .ends_with("12/15 cords shown · 3 omitted · F2 all facts · F3 return")
    }));
    assert!(
        frame.hits.is_empty(),
        "drawing cords must not invent action mappings"
    );
    assert_eq!(scene.face(), &checked);
    scene.show_details(true);
    let details: String = scene.details.iter().map(|row| row.text.as_str()).collect();
    for subject in checked
        .subjects
        .iter()
        .filter(|subject| subject.role == FaceRole::Cord)
    {
        assert!(details.contains(&subject.identity));
    }
    // A one-column page cannot draw a link to a Gear on another page.
    assert!(
        link_notice(
            &face,
            LayoutRect {
                width: 320,
                height: 240,
                ..screen
            },
            0
        )
        .ends_with("0/15 cords shown · 15 omitted · F2/F3")
    );
    // Missing semantic endpoint facts never become geometry-invented links.
    face.properties.retain(|p| p.name != "sink-port");
    assert!(
        link_notice(&face, screen, 0)
            .ends_with("0/15 cords shown · 15 omitted · F2 all facts · F3 return")
    );
}
