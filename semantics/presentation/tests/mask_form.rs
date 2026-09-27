#![cfg(feature = "form-catalog")]

use conduit_core::{kind_id, port_id, KindIdentity, PortDescriptor, PortDirection, PortTemporal};
use conduit_form::{
    check_syntax_document, expand_canonical_form_for_authoring, parse_syntax_document,
    KindProjection, KindSignature, ProfileCatalog, StartupCatalog,
};
use conduit_presentation::{
    MaskForm, MANIFESTATION_VALUE_KIND, PRESENTATION_INTERACTION_VALUE_KIND,
    PRESENTATION_VALUE_KIND,
};

fn port(
    name: &str,
    kind: &str,
    direction: PortDirection,
    temporal: PortTemporal,
) -> PortDescriptor {
    PortDescriptor {
        port_id: port_id(name),
        value_kind: kind_id(kind),
        direction,
        temporal,
    }
}

fn kind(name: &str, inputs: Vec<PortDescriptor>, outputs: Vec<PortDescriptor>) -> KindProjection {
    KindProjection {
        kind_id: kind_id(name),
        kind_contract_revision: KindIdentity::from(format!("conduit.test/{name}@1")),
        inputs,
        outputs,
        configuration: vec![],
    }
}

fn catalogs() -> (StartupCatalog, ProfileCatalog) {
    let definitions = vec![
        kind(
            "presentation/layout",
            vec![port(
                "presentation",
                PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "scene",
                "graphics/scene@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "presentation/composite",
            vec![port(
                "scene",
                "graphics/scene@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "frame",
                "graphics/frame@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "display/show",
            vec![port(
                "frame",
                "graphics/frame@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "show",
                MANIFESTATION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "display/input",
            vec![],
            vec![port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
        kind(
            "web/dom",
            vec![port(
                "presentation",
                PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "show",
                MANIFESTATION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "web/input",
            vec![],
            vec![port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
        kind(
            "presentation/aural",
            vec![port(
                "presentation",
                PRESENTATION_VALUE_KIND,
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "text",
                "text/text@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "speech/synthesize",
            vec![port(
                "text",
                "text/text@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "audio",
                "audio/pcm@1",
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "audio/play",
            vec![port(
                "audio",
                "audio/pcm@1",
                PortDirection::Input,
                PortTemporal::Value,
            )],
            vec![port(
                "show",
                MANIFESTATION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Value,
            )],
        ),
        kind(
            "audio/listen",
            vec![],
            vec![port(
                "interaction",
                PRESENTATION_INTERACTION_VALUE_KIND,
                PortDirection::Output,
                PortTemporal::Flow { closes: true },
            )],
        ),
    ];
    let mut startup = StartupCatalog::new();
    let mut profiles = ProfileCatalog::new();
    for definition in definitions {
        startup
            .insert(KindSignature {
                kind: definition.kind_id.as_str().into(),
                startup_parameters: vec![],
            })
            .unwrap();
        profiles.insert(definition).unwrap();
    }
    startup
        .insert_value_kind_alias("Presentation", kind_id(PRESENTATION_VALUE_KIND))
        .unwrap();
    startup
        .insert_value_kind_alias(
            "FaceInteraction",
            kind_id(PRESENTATION_INTERACTION_VALUE_KIND),
        )
        .unwrap();
    startup
        .insert_value_kind_alias("Show", kind_id(MANIFESTATION_VALUE_KIND))
        .unwrap();
    (startup, profiles)
}

fn admit(source: &str, name: &str) -> MaskForm {
    let (startup, profiles) = catalogs();
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty(), "{:?}", syntax.diagnostics);
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, name, &profiles).unwrap();
    MaskForm::admit(&expanded).unwrap()
}

#[test]
fn graphical_browser_and_spoken_masks_are_ordinary_forms_with_one_role_boundary() {
    let native = admit(
        "form native-graphical (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n layout: presentation/layout\n compose: presentation/composite\n output: display/show\n input: display/input\n presentation >> layout.presentation\n layout.scene >> compose.scene\n compose.frame >> output.frame\n output.show >> show\n input.interaction >> interaction\n}\n",
        "native-graphical",
    );
    let browser = admit(
        "form browser-graphical (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n output: web/dom\n input: web/input\n presentation >> output.presentation\n output.show >> show\n input.interaction >> interaction\n}\n",
        "browser-graphical",
    );
    let spoken = admit(
        "form spoken (\n >> presentation: Presentation\n interaction: FaceInteraction...| >>\n show: Show >>\n) {\n language: presentation/aural\n voice: speech/synthesize\n output: audio/play\n input: audio/listen\n presentation >> language.presentation\n language.text >> voice.text\n voice.audio >> output.audio\n output.show >> show\n input.interaction >> interaction\n}\n",
        "spoken",
    );

    assert_eq!(native.form_name, "native-graphical");
    assert_eq!(browser.form_name, "browser-graphical");
    assert_eq!(spoken.form_name, "spoken");
    assert_ne!(
        native.form_identity.checked_form_id,
        browser.form_identity.checked_form_id
    );
    assert_ne!(
        browser.form_identity.checked_form_id,
        spoken.form_identity.checked_form_id
    );
    for mask in [native, browser, spoken] {
        assert_eq!(
            mask.presentation_input.front_port_id.as_str(),
            "presentation"
        );
        assert_eq!(
            mask.interaction_output.front_port_id.as_str(),
            "interaction"
        );
        assert_eq!(mask.show_output.front_port_id.as_str(), "show");
    }
}

#[test]
fn an_ordinary_form_without_the_mask_role_boundary_is_not_a_mask() {
    let (startup, profiles) = catalogs();
    let syntax = parse_syntax_document(
        "form tutorial (\n >> presentation: Presentation\n show: Show >>\n) {\n output: web/dom\n presentation >> output.presentation\n output.show >> show\n}\n",
    );
    let checked = check_syntax_document(&syntax, &startup).unwrap();
    let expanded = expand_canonical_form_for_authoring(&checked, "tutorial", &profiles).unwrap();
    assert!(MaskForm::admit(&expanded).is_err());
}
