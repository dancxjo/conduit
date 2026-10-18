//! Derived navigation and materially different consumers of the same Face.
use super::*;
use conduit_std_host::terminal_face_mask::{
    TerminalFaceMask, TerminalInput, TerminalInputOutcome, TerminalMaskExecution,
};
use conduitos::native_face_scene::{FaceControl, FaceSceneError, NativeFaceScene};

pub(super) fn navigation(
    face: &Face,
    selected: &str,
) -> (FaceNavigation, FaceProjection, NavigationState) {
    let subjects: Vec<_> = face
        .subjects
        .iter()
        .filter(|s| matches!(&s.role, FaceRole::Semantic(k) if k.as_str() == "commons/place"))
        .map(|s| s.identity.clone())
        .collect();
    assert_eq!(subjects.len(), 3);
    let follows = face.relationships.iter().filter(|r| matches!(&r.kind, FaceRelationshipKind::Semantic(k) if k.as_str() == "commons/reaches-place")).map(|r| NavigationFollow {
        identity: format!("commons/follow/{}/{}", r.source, r.target),
        source_subject: r.source.clone(), target_subject: r.target.clone(), relationship: r.kind.clone(),
        target_place: FacePlace::Program, target_aspect: FaceAspect::Structure,
    }).collect::<Vec<_>>();
    assert_eq!(follows.len(), 2);
    let navigation = FaceNavigation::new(
        face,
        vec![NavigationPlace {
            place: FacePlace::Program,
            root_subject: "commons/hub".into(),
            name: face
                .subjects
                .iter()
                .find(|s| s.identity == "commons/hub")
                .unwrap()
                .name
                .clone(),
            aspects: vec![NavigationAspect {
                aspect: FaceAspect::Structure,
                focusable_subjects: subjects,
            }],
        }],
        follows,
    )
    .unwrap();
    let mut members = Vec::new();
    let mut add = |item, depth| {
        members.push(ProjectionMembership {
            place: FacePlace::Program,
            aspect: FaceAspect::Structure,
            item,
            depth,
        })
    };
    for s in &face.subjects {
        add(
            ProjectionItem::Subject(s.identity.clone()),
            FaceDepth::Primary,
        );
    }
    for i in 0..face.relationships.len() {
        add(
            ProjectionItem::Relationship(i.try_into().unwrap()),
            FaceDepth::Primary,
        );
    }
    for i in 0..face.composition.len() {
        add(
            ProjectionItem::Composition(i.try_into().unwrap()),
            FaceDepth::Context,
        );
    }
    for i in 0..face.properties.len() {
        add(
            ProjectionItem::Property(i.try_into().unwrap()),
            FaceDepth::Detail,
        );
    }
    for i in 0..face.text.len() {
        add(
            ProjectionItem::Text(i.try_into().unwrap()),
            FaceDepth::Detail,
        );
    }
    for a in &face.actions {
        add(
            ProjectionItem::Action(a.identity.clone()),
            FaceDepth::Detail,
        );
    }
    let projection = FaceProjection::new(face, &navigation, members).unwrap();
    let cursor = FaceCursor {
        face: face.identity.clone(),
        navigation: navigation.identity.clone(),
        revision: face.revision,
        place: FacePlace::Program,
        aspect: FaceAspect::Structure,
        focus: Some(selected.into()),
        depth: FaceDepth::Exact,
    };
    let state = NavigationState::new(&navigation, cursor, 2).unwrap();
    (navigation, projection, state)
}

pub(super) fn observe(face: &Face, selected: &str, host: &HostAdvertisement) -> serde_json::Value {
    let before = face.clone();
    let (navigation, projection, state) = navigation(face, selected);
    let linear = render_linear_navigation(face, &navigation, &projection, state.cursor()).unwrap();
    assert_eq!(
        linear,
        render_linear_navigation(face, &navigation, &projection, state.cursor()).unwrap()
    );
    assert_eq!(linear.face_id, face.identity);
    assert!(linear.encoded_bytes <= MAX_LINEAR_FACE_BYTES);
    for action in &face.actions {
        let prefix = format!("ACTION id={:?} ", action.identity);
        let record = linear
            .lines
            .iter()
            .find(|s| s.starts_with(&prefix))
            .unwrap();
        assert!(record.contains(&format!(
            "intent={:?} target={:?}",
            action.intent, action.target
        )));
        assert!(record.contains(&format!("arguments={:?}", action.arguments)));
        if let FaceActionAvailability::Unavailable { reason_code, .. } = &action.availability {
            assert!(record.contains(reason_code));
        }
    }
    let inspection = face
        .actions
        .iter()
        .find(|a| a.intent == "conduit.intent/open-inspection@1")
        .unwrap();
    assert!(!inspection.availability.is_available());
    assert!(linear
        .lines
        .iter()
        .any(|s| s.contains("commons/inspection")));
    let terminal = terminal(face, host);
    let graphical = graphical(face);
    assert_eq!(face, &before);
    serde_json::json!({"selected":selected,"face":face,"navigation":navigation,"cursor":state.cursor(),
        "linear":linear.lines,"linear_bytes":linear.encoded_bytes,"terminal":terminal,"graphical":graphical})
}

fn terminal(face: &Face, host: &HostAdvertisement) -> serde_json::Value {
    let mut mask = TerminalFaceMask::prepare(face.clone(), 80, 24).unwrap();
    let mut execution = HostedTerminalMaskExecution::new(host).unwrap();
    let mut output = Vec::new();
    mask.present(&mut execution, &mut output).unwrap();
    let primary = mask.show().unwrap().clone();
    assert_eq!(
        mask.input(TerminalInput::Inspect, &primary, 1).unwrap(),
        TerminalInputOutcome::Redraw
    );
    execution.close_without_input().unwrap();
    assert!(mask.inspecting());
    assert!(mask.show().is_none());
    assert!(mask.input(TerminalInput::PageDown, &primary, 2).is_err());
    let mut shows = Vec::new();
    let mut readback = String::new();
    let mut last_row = 0;
    let mut complete = false;
    // Read the real renderer's finite page status; do not substitute the
    // nonvisual serializer for an acknowledged terminal inspection journey.
    for page in 0..64 {
        let mut frame = Vec::new();
        mask.present(&mut execution, &mut frame).unwrap();
        let frame_text = std::str::from_utf8(&frame).unwrap();
        let lines: Vec<_> = frame_text.split("\r\n").collect();
        let status_index = lines
            .iter()
            .position(|s| s.starts_with("Inspect · lines "))
            .unwrap();
        let (range, total) = lines[status_index]
            .strip_prefix("Inspect · lines ")
            .unwrap()
            .split_once(" / ")
            .unwrap();
        let (start, end) = range.split_once('–').unwrap();
        let (start, end, total): (usize, usize, usize) = (
            start.parse().unwrap(),
            end.parse().unwrap(),
            total.parse().unwrap(),
        );
        assert_eq!(start, last_row + 1);
        assert!(end <= total && total <= conduit_std_host::terminal_face_mask::MAX_TERMINAL_ROWS);
        for row in lines.iter().take(status_index).skip(2) {
            if let Some(text) = row.strip_prefix("  ").or_else(|| row.strip_prefix("· ")) {
                readback.push_str(text);
            }
        }
        last_row = end;
        output.extend_from_slice(&frame);
        let show = mask.show().unwrap().clone();
        assert_eq!(show.face_id, face.identity);
        assert_eq!(show.show.lifecycle, ManifestationLifecycle::Available);
        assert_eq!(show.show.host_id, host.host_id);
        assert_eq!(show.show.boot_id, host.boot_id);
        assert!(!shows.iter().any(|s: &MaskShow| s.show_id == show.show_id));
        shows.push(show.clone());
        if end == total {
            complete = true;
            break;
        }
        assert_eq!(
            mask.input(TerminalInput::PageDown, &show, page + 3)
                .unwrap(),
            TerminalInputOutcome::Redraw
        );
        execution.close_without_input().unwrap();
    }
    assert!(
        complete,
        "bounded terminal inspection must reach its actual last row"
    );
    assert!(output.len() <= 512 * 1024);
    for record in render_linear_face(face).unwrap().lines {
        assert!(
            readback.contains(&record),
            "terminal inspection lost an exact linear record"
        );
    }
    assert!(readback.contains("commons/inspection"));
    assert!(readback.contains("not-currently-offered") || readback.contains("context-unavailable"));
    for relationship in face.relationships.iter().filter(|r| matches!(&r.kind, FaceRelationshipKind::Semantic(k) if k.as_str().starts_with("commons/"))) {
        assert!(readback.contains(&format!("RELATIONSHIP kind={:?} source={:?} target={:?}", relationship.kind, relationship.source, relationship.target)));
    }
    let inspection = face
        .actions
        .iter()
        .find(|a| a.intent == "conduit.intent/open-inspection@1")
        .unwrap();
    assert_eq!(
        FaceInteraction::new(
            face,
            shows.last().unwrap(),
            &inspection.identity,
            &inspection.target,
            vec![],
            99
        )
        .unwrap_err(),
        FaceInteractionRefusal::UnavailableAction
    );
    assert_eq!(mask.face(), face);
    mask.input(TerminalInput::Escape, shows.last().unwrap(), 100)
        .unwrap();
    execution.cancel().unwrap();
    assert!(mask.show().is_none());
    let receipts: Vec<_> = shows.iter().map(|s| serde_json::json!({
        "show_id":s.show_id,"face_id":s.face_id,"revision":s.face_revision,"mask_plot":s.mask_plot,
        "plan":s.show.plan_id,"play":s.show.active_play_id,"sequence":s.show.play_sequence,
        "host":s.show.host_id,"boot":s.show.boot_id,"lifecycle":s.show.lifecycle,"signs":s.show.signs,
    })).collect();
    serde_json::json!({"proof_class":"software terminal effect and actual planned Mask Shows; no human or native input", "columns":80,"rows":24,"reading_pages":shows.len(),"read_rows":last_row,"primary_show":primary,"inspection_shows":receipts,"transcript":String::from_utf8(output).unwrap(),"readback":readback,"cancelled":true})
}

fn graphical(face: &Face) -> serde_json::Value {
    let utterances = plan_face_utterances(face).unwrap();
    let required: Vec<_> = utterances.clauses.iter().filter(|clause| match &clause.provenance {
        FaceUtteranceProvenance::Subject(p) => p.identity().starts_with("commons/"),
        FaceUtteranceProvenance::Relationship(p) => matches!(&face.relationships[*p.index() as usize].kind, FaceRelationshipKind::Semantic(k) if k.as_str().starts_with("commons/")),
        FaceUtteranceProvenance::Action(p) => face.actions.iter().any(|a| a.identity == *p.identity() && a.intent == "conduit.intent/open-inspection@1"),
        _ => false,
    }).map(|clause| clause.text.clone()).collect();
    assert_eq!(required.len(), 8);
    let mut views = Vec::new();
    for (width, height) in [(1280, 800), (640, 480)] {
        let mut scene = NativeFaceScene::prepare(face.clone(), width, height).unwrap();
        assert!(
            !scene.has_diagram(),
            "Commons relations must not invent Gear/Port/Cord topology"
        );
        let mut frames = Vec::new();
        let mut visible_details = String::new();
        for detail in [false, true] {
            scene.show_details(detail);
            for _ in 0..scene.pages() {
                let frame = scene.frame().unwrap();
                assert_eq!(frame.face_id, &face.identity);
                assert_eq!(frame.revision, face.revision);
                assert!(frame.scene.commands().len() <= MAX_GRAPHICS_COMMANDS);
                if detail {
                    for command in frame.scene.commands() {
                        visible_details.push_str(command.payload());
                    }
                }
                for hit in &frame.hits {
                    assert_eq!(hit.action, &face.actions[hit.control.action]);
                }
                frames.push(serde_json::json!({"detail":detail,"page":frame.page,"commands":frame.scene.commands().iter().map(|c| serde_json::json!({"text":c.payload(),"bounds":[c.bounds.x,c.bounds.y,c.bounds.width as i16,c.bounds.height as i16]})).collect::<Vec<_>>() }));
                scene.turn_page(true);
            }
        }
        for clause in &required {
            assert!(
                visible_details.contains(clause),
                "graphical Details lost a source-bound clause: {clause}"
            );
        }
        for (action, contract) in face.actions.iter().enumerate() {
            for (index, argument) in contract.arguments.iter().enumerate() {
                if contract.availability.is_available() {
                    let (resolved, actual) = scene
                        .resolve(
                            &face.identity,
                            face.revision,
                            FaceControl {
                                action,
                                argument: Some(index),
                            },
                        )
                        .unwrap();
                    assert_eq!(resolved, contract);
                    assert_eq!(actual, Some(argument));
                }
            }
            let control = FaceControl {
                action,
                argument: None,
            };
            match scene.resolve(&face.identity, face.revision, control) {
                Ok((resolved, _)) => {
                    assert!(contract.availability.is_available());
                    assert_eq!(resolved, contract);
                }
                Err(FaceSceneError::UnavailableAction) => {
                    assert!(!contract.availability.is_available())
                }
                other => panic!("unexpected control resolution {other:?}"),
            }
        }
        assert_eq!(scene.face(), face);
        views.push(serde_json::json!({"width":width,"height":height,"frames":frames}));
    }
    assert_ne!(
        views[0]["frames"], views[1]["frames"],
        "geometry should vary while the retained Face stays exact"
    );
    serde_json::json!({"proof_class":"CPU native graphical preparation only; no native Show or QEMU walkability", "required_clauses":required,"views":views})
}
