//! Test-only mechanical receipt admission. JSON metadata is never graph authority.
use conduit_language::{lexical::*, *};
use conduit_plot::rust_binding::NativeRustBinding;
use serde_json::Value;

pub struct Receipt {
    pub lexical: PreparedLexicalTape,
    pub basis: LanguageParserBasis,
    pub arcs: Vec<LanguageDependencyArc>,
    pub choices: Vec<usize>,
    pub vocative: usize,
}
fn bytes(row: &Value, key: &str) -> Result<Vec<u8>, String> {
    let input = row[key].as_array().ok_or(key)?;
    if input.is_empty() || input.len() > 262144 {
        return Err(format!("{key}: byte bound"));
    }
    input
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| u8::try_from(n).ok())
                .ok_or_else(|| key.into())
        })
        .collect()
}
fn ordinal(row: &Value, key: &str) -> Result<usize, String> {
    row[key]
        .as_u64()
        .and_then(|n| usize::try_from(n).ok())
        .ok_or_else(|| key.into())
}
pub fn admit(row: &Value) -> Result<Receipt, String> {
    let source = LanguageTextRevision::decode(&bytes(row, "source_material_bytes")?)
        .map_err(|e| format!("source: {e:?}"))?;
    let native = LanguageLexicalTape::decode(&bytes(row, "lexical_tape_bytes")?)
        .map_err(|e| format!("lexical: {e:?}"))?;
    let basis =
        LanguageParserBasis::decode(&bytes(row, "basis")?).map_err(|e| format!("basis: {e:?}"))?;
    if native.source() != &source
        || basis.text() != source.material().identity()
        || basis.source_revision() != source.material().revision()
        || row["text"].as_str() != Some(source.material().text().as_str())
        || row["text_identity"].as_str() != Some(basis.text().get())
        || row["source_revision"].as_str() != Some(basis.source_revision().get())
        || row["analysis_revision"].as_str() != Some(basis.analysis_revision().get())
    {
        return Err("foreign source/basis metadata".into());
    }
    let acquisition = row.get("acquisition_history").unwrap_or(row);
    let lexical = if acquisition.get("asr_envelope_bytes").is_some() {
        super::asr_sources::admit_history(acquisition, &native)?
    } else if let Some(history) = row["source_revision_history_bytes"].as_array() {
        if history.is_empty() || history.len() > 8 {
            return Err("revision history bound".into());
        }
        let mut previous = None;
        for revision_bytes in history {
            let wrapper = serde_json::json!({"revision":revision_bytes});
            let revision = LanguageTextRevision::decode(&bytes(&wrapper, "revision")?)
                .map_err(|e| format!("history: {e:?}"))?;
            previous = Some(
                prepare_lexical_tape(&revision, native.profile(), previous.as_ref())
                    .map_err(|e| format!("history reconstruction: {e:?}"))?,
            );
        }
        let admitted = previous.ok_or("empty history")?;
        if admitted.tape().source() != &source {
            return Err("foreign history final source".into());
        }
        admitted
    } else {
        prepare_lexical_tape(&source, native.profile(), None)
            .map_err(|e| format!("reconstruction: {e:?}"))?
    };
    if lexical.tape() != &native {
        return Err("changed lexical tape".into());
    }
    let count = native.tokens().len();
    if !(2..=4).contains(&count) || row["complete"] != true {
        return Err("incomplete/bounded graph".into());
    }
    let choices = row["choices"].as_array().ok_or("choices")?;
    if choices.len() != 4 {
        return Err("choice slots".into());
    }
    let choices = choices
        .iter()
        .map(|v| {
            v.as_u64()
                .and_then(|n| usize::try_from(n).ok())
                .ok_or_else(|| "choice".into())
        })
        .collect::<Result<Vec<_>, String>>()?;
    if choices[count..].iter().any(|choice| *choice != 0) {
        return Err("inactive choice slots".into());
    }
    for (token, choice) in native.tokens().iter().zip(&choices) {
        if *choice >= token.candidates().len() {
            return Err("foreign lexical choice".into());
        }
    }
    let predicted = row["predicted"].as_array().ok_or("predicted")?;
    if predicted.len() != count {
        return Err("arc coverage".into());
    }
    let mut arcs = vec![None; count];
    let mut roots = 0;
    let mut vocative = None;
    for item in predicted {
        let dependent = ordinal(item, "dependent")?;
        let head = ordinal(item, "head")?;
        let arc = LanguageDependencyArc::decode(&bytes(item, "canonical_arc_bytes")?)
            .map_err(|e| format!("arc: {e:?}"))?;
        if let Some(identity) = item.get("canonical_arc_identity") {
            let digest = arc
                .clone()
                .into_structured()
                .map_err(|e| format!("arc identity: {e:?}"))?
                .semantic_digest()
                .map_err(|e| format!("arc digest: {e:?}"))?;
            let expected = digest
                .iter()
                .map(|byte| format!("{byte:02x}"))
                .collect::<String>();
            if identity.as_str() != Some(expected.as_str()) {
                return Err("foreign arc identity".into());
            }
        }
        if dependent >= count
            || arcs[dependent].is_some()
            || arc.dependent().revision() != basis.analysis_revision()
            || arc.dependent().token() != native.tokens()[dependent].identity()
        {
            return Err("foreign/duplicate dependent".into());
        }
        match arc.governor() {
            LanguageDependencyHead::Root
                if head == 4
                    && arc.relation().base() == &LanguageUniversalDependencyRelation::Root =>
            {
                roots += 1
            }
            LanguageDependencyHead::Token(reference)
                if head < count
                    && head != dependent
                    && reference.revision() == basis.analysis_revision()
                    && reference.token() == native.tokens()[head].identity() => {}
            _ => return Err("foreign governor".into()),
        }
        if arc.relation().base() == &LanguageUniversalDependencyRelation::Vocative
            && vocative.replace(dependent).is_some()
        {
            return Err("multiple vocatives".into());
        }
        arcs[dependent] = Some(arc);
    }
    if roots != 1 {
        return Err("root coverage".into());
    }
    let arcs = arcs
        .into_iter()
        .collect::<Option<Vec<_>>>()
        .ok_or("arc coverage")?;
    // A single root plus no cycles establishes finite connectivity.
    for start in 0..count {
        let mut cursor = start;
        let mut visited = vec![false; count];
        loop {
            if visited[cursor] {
                return Err("graph cycle".into());
            }
            visited[cursor] = true;
            match arcs[cursor].governor() {
                LanguageDependencyHead::Root => break,
                LanguageDependencyHead::Token(reference) => {
                    cursor = native
                        .tokens()
                        .iter()
                        .position(|t| t.identity() == reference.token())
                        .ok_or("head occurrence")?
                }
            }
        }
    }
    Ok(Receipt {
        lexical,
        basis,
        arcs,
        choices,
        vocative: vocative.ok_or("missing vocative")?,
    })
}
