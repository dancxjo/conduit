//! Explicit temporal profiles of unchanged owned DSP equations.
pub fn source() -> String {
    let units = [
        include_str!("../../../../semantics/audio/dsp/seeded-noise.conduit"),
        include_str!("../../../../semantics/audio/dsp/envelope.conduit"),
        include_str!("../../../../semantics/audio/dsp/gain.conduit"),
        include_str!("../fixtures/shared-excitation-frame.conduit"),
    ];
    let mut imports = Vec::new();
    let mut declarations = Vec::new();
    let mut plots = Vec::new();
    for unit in units {
        let (prefix, body) = unit.split_once("plot ").unwrap();
        for line in prefix.lines() {
            if line.starts_with("with ") {
                imports.push(line);
            } else {
                declarations.push(line);
            }
        }
        plots.push(format!("plot {body}"));
    }
    for (unit, entry, value_entry) in components() {
        plots.push(value_profile(unit, entry, value_entry));
    }
    format!(
        "{}\n{}\n{}\n",
        imports.join("\n"),
        declarations.join("\n"),
        plots.join("\n")
    )
}

pub fn components() -> [(&'static str, &'static str, &'static str); 3] {
    [
        (
            include_str!("../../../../semantics/audio/dsp/seeded-noise.conduit"),
            "dsp/noise-step-candidate",
            "dsp/noise-step-value",
        ),
        (
            include_str!("../../../../semantics/audio/dsp/envelope.conduit"),
            "dsp/envelope",
            "dsp/envelope-value",
        ),
        (
            include_str!("../../../../semantics/audio/dsp/gain.conduit"),
            "dsp/gain",
            "dsp/gain-value",
        ),
    ]
}

// This fixture derives an explicit Value profile from one owned pure equation.
// The source equation and Type declarations are never reimplemented in Rust.
pub fn value_profile(unit: &str, entry: &str, value_entry: &str) -> String {
    let marker = format!("plot {entry} (");
    let start = unit.rfind(&marker).unwrap();
    let definition = &unit[start..];
    assert!(!definition.contains("\nplot "));
    assert_eq!(definition.matches("...|").count(), 2);
    definition
        .replacen(&marker, &format!("plot {value_entry} ("), 1)
        .replace("...|", "")
}
