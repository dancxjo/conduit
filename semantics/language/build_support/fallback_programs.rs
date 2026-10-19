//! Retain existing linear authored expression topology; no semantic decisions.
use conduit_plot::*;
pub fn retain(checked: &CheckedSyntaxDocument) {
    let names = [
        "language-parser-score-proposal",
        "language-parser-transition",
        "language-parser-decode-complete",
        "language-parser-scorer-branch",
        "language-parser-scorer-model-features",
        "language-parser-scorer-legality",
        "language-parser-scorer-fallback",
        "language-parser-scorer-rank",
        "language-parser-scorer-ranking-next",
        "language-parser-scorer-expand",
        "language-parser-scorer-merge",
        "language/analysis-round-empty",
        "language-parser-admitted-agreement",
        "language-parser-advance-stable",
    ];
    let mut retained = Vec::new();
    for name in names {
        let authored =
            expand_canonical_plot_for_authoring(checked, name, &ProfileCatalog::new()).unwrap();
        assert_eq!(authored.input_bindings.len(), 1);
        assert_eq!(authored.output_bindings.len(), 1);
        let mut id = authored.input_bindings[0].gear_id.clone();
        let mut seen = Vec::new();
        let mut programs = Vec::new();
        loop {
            assert!(!seen.contains(&id));
            seen.push(id.clone());
            let gear = authored
                .expanded
                .gears
                .iter()
                .find(|g| g.gear_id == id)
                .unwrap();
            assert!(gear.resource_ports.is_empty());
            let [entry] = gear.configuration.as_slice() else {
                panic!("pure expression")
            };
            assert_eq!(entry.key, "program");
            let conduit_core::ConfigurationValue::Text(hex) = &entry.value else {
                panic!("program")
            };
            programs.push(hex.clone());
            let next: Vec<_> = authored
                .expanded
                .connections
                .iter()
                .filter(|c| c.source_gear_id == id)
                .collect();
            if next.is_empty() {
                assert_eq!(id, authored.output_bindings[0].gear_id);
                break;
            }
            assert_eq!(next.len(), 1);
            id = next[0].sink_gear_id.clone();
        }
        assert_eq!(seen.len(), authored.expanded.gears.len());
        retained.push((name, programs));
    }
    let rows: Vec<_> = retained
        .iter()
        .map(|(name, programs)| format!("({name:?}, &{programs:?})"))
        .collect();
    let text = format!(
        "pub const FALLBACK_ROUND_PROGRAMS: &[(&str,&[&str])] = &[{}];\n",
        rows.join(",")
    );
    std::fs::write(
        std::path::PathBuf::from(std::env::var_os("OUT_DIR").unwrap())
            .join("fallback_round_programs.rs"),
        text,
    )
    .unwrap();
}
