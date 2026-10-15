//! Runtime predicates reuse the exact authored snapshot laws. Candidate data
//! never masquerades as a refined Native value during pure construction.
//! Portable Boolean operands are eager. Conditional branches keep arithmetic
//! laws behind prior domain checks, matching Native admission order.
use conduit_plot::parse_syntax_document;
pub fn validation_source(source: &str) -> String {
    let syntax = parse_syntax_document(source);
    assert!(syntax.diagnostics.is_empty());
    let laws = |name: &str, prefix: &str| {
        let ty = syntax
            .types
            .iter()
            .find(|t| t.name.text == name)
            .expect("periodic law owner");
        assert!(!ty.invariants.is_empty());
        conjunction(
            &ty.invariants
                .iter()
                .map(|law| format!("({})", prefixed(&law.text, prefix)))
                .collect::<Vec<_>>(),
        )
    };
    let request = laws("AudioPeriodicRequest", ".");
    let projection = format!(
        "({}) ? ({}) : false",
        laws("AudioPeriodicRequest", ".request"),
        laws("AudioPeriodicProjection", ".")
    );
    let mut step = String::from("plot dsp/oscillator-step-init (\n >> value: AudioPeriodicStepInput\n result: AudioPeriodicCheckedStep >>\n) = ({value: ., valid: true})\n");
    let mut stages = Vec::new();
    for (name, prefix) in [
        (
            "AudioPeriodicRequest",
            ".value.cursor.profile.projection.request",
        ),
        (
            "AudioPeriodicProjection",
            ".value.cursor.profile.projection",
        ),
        ("AudioPeriodicProfile", ".value.cursor.profile"),
        ("AudioPeriodicCursor", ".value.cursor"),
    ] {
        let ty = syntax.types.iter().find(|t| t.name.text == name).unwrap();
        for law in &ty.invariants {
            let n = stages.len();
            let predicate = prefixed(&law.text, prefix);
            step.push_str(&format!("plot dsp/oscillator-step-law-{n} (\n >> value: AudioPeriodicCheckedStep\n result: AudioPeriodicCheckedStep >>\n) = ({{value: .value, valid: .valid ? ({predicate}) : false}})\n"));
            stages.push(format!("law{n}"));
        }
    }
    step.push_str("plot dsp/oscillator-check-step (\n >> value: AudioPeriodicStepInput\n result: AudioPeriodicCheckedStep >>\n) {\n init: dsp/oscillator-step-init()\n value >> init.value\n");
    let mut previous = "init".to_string();
    for (n, stage) in stages.iter().enumerate() {
        step.push_str(&format!(
            " {stage}: dsp/oscillator-step-law-{n}()\n {previous}.result >> {stage}.value\n"
        ));
        previous = stage.clone();
    }
    step.push_str(&format!(" {previous}.result >> result\n}}.\n"));
    let request_in_projection = laws("AudioPeriodicRequest", ".request");
    format!("# Derived from authoritative Native where laws; no independent math copy.\nplot dsp/oscillator-check-request (\n >> value: AudioPeriodicRequest\n result: AudioPeriodicCheckedRequest >>\n) = ({{request: ., valid: {request}}})\nplot dsp/oscillator-check-projection (\n >> value: AudioPeriodicProjectionCandidate\n result: AudioPeriodicCheckedProjection >>\n) = ({{projection: ., valid_request: {request_in_projection}, valid: {projection}}})\n{step}")
}
// These checked laws contain only root/member projections, not member access
// on computed records. Preserve member dots and numeric literal dots verbatim.
fn prefixed(text: &str, prefix: &str) -> String {
    if prefix == "." {
        return text.into();
    }
    let chars: Vec<_> = text.chars().collect();
    let mut output = String::new();
    for (index, ch) in chars.iter().copied().enumerate() {
        let root = ch == '.'
            && chars
                .get(index + 1)
                .is_some_and(|c| c.is_ascii_alphabetic() || *c == '_')
            && index
                .checked_sub(1)
                .and_then(|i| chars.get(i))
                .is_none_or(|c| !c.is_ascii_alphanumeric() && !matches!(c, '_' | ')' | ']' | '.'));
        if root {
            output.push_str(prefix);
        }
        output.push(ch);
    }
    output
}

fn conjunction(parts: &[String]) -> String {
    match parts.len() {
        0 => "true".into(),
        1 => parts[0].clone(),
        n => {
            let mid = n / 2;
            format!(
                "({}) && ({})",
                conjunction(&parts[..mid]),
                conjunction(&parts[mid..])
            )
        }
    }
}
