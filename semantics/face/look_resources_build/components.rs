//! Make composition of reviewed components with exact authored Look expressions.
use conduit_plot::parse_syntax_document;
use std::{fs, path::Path};

pub(super) fn generate(output: &Path) {
    for file in ["todo-mask", "todo-required-item", "patchbay-mask"] {
        println!("cargo:rerun-if-changed=components/{file}.conduit");
    }
    let authored = parse_syntax_document(include_str!("../field-instrument-looks.conduit"));
    assert!(authored.diagnostics.is_empty());
    for (name, base, components) in [
        (
            "todo",
            concat!(
                include_str!("../components/todo-mask.conduit"),
                "\n",
                include_str!("../components/todo-required-item.conduit")
            ),
            &["flat", "grouped", "required-item"][..],
        ),
        (
            "patchbay",
            include_str!("../components/patchbay-mask.conduit"),
            &["nodes"][..],
        ),
    ] {
        let mut source = format!("{base}\n");
        for theme in ["day", "night"] {
            let plot = authored
                .plots
                .iter()
                .find(|plot| plot.name.text == format!("look/field-instrument-{theme}"))
                .expect("checked authored Look exists");
            let [parameter] = plot.front.startup_parameters.as_slice() else {
                panic!("one checked authored Look parameter");
            };
            assert_eq!(parameter.name.text, "look");
            let literal = &parameter
                .default
                .as_ref()
                .expect("checked Look literal")
                .text;
            for component in components {
                let target = if name == "todo" && *component == "required-item" {
                    "todo-required-item".into()
                } else {
                    format!("{name}-component-{component}")
                };
                let root = if name == "patchbay" {
                    format!("patchbay-mask-field-instrument-{theme}")
                } else {
                    format!("todo-mask-{component}-field-instrument-{theme}")
                };
                source.push_str(&format!(
                    "plot {root} (\n >> face: Face\n show: Show >>\n \
                     interaction: FaceInteraction...| >>\n) {{\n \
                     component: {target}(look = {literal})\n \
                     face >> component.face\n component.show >> show\n \
                     component.interaction >> interaction\n}}\n"
                ));
            }
        }
        fs::write(
            output.join(format!("field-instrument-{name}-mask.conduit")),
            source,
        )
        .expect("retain reviewed component Source composition");
    }
}
