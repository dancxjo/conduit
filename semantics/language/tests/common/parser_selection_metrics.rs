//! Blind held-out evaluation: only source forms enter lexical preparation and
//! decoding. Reference attachments are inspected after selection has finished.
#![allow(dead_code)]
use super::parser_selection_driver::Decoder;
use super::parser_selection_values::{profile, revision_with_text};
use conduit_language::{lexical::prepare_lexical_tape, *};
use serde_json::{json, Value};
use std::time::Instant;

fn ratio(numerator: u64, denominator: u64) -> Value {
    if denominator == 0 {
        Value::Null
    } else {
        json!(numerator as f64 / denominator as f64)
    }
}
pub fn heldout(decoder: &mut Decoder) -> Value {
    let lexical_profile = profile();
    let mut rows = Vec::new();
    let mut tokens = 0u64;
    let mut uas_correct = 0u64;
    let mut las_correct = 0u64;
    let (mut tp, mut fp, mut fn_) = (0u64, 0u64, 0u64);
    let mut total_calls = 0u64;
    let mut total_ns = 0u128;
    for (index, line) in include_str!("../../training/ewt_joint_v2/test_annotations.jsonl")
        .lines()
        .enumerate()
    {
        let row: Value = serde_json::from_str(line).unwrap();
        // Only these forms become model/grammar inputs. Whitespace is explicit
        // reconstruction from retained corpus tokens, not original typography.
        let forms: Vec<&str> = row["forms"]
            .as_array()
            .unwrap()
            .iter()
            .map(|value| value.as_str().unwrap())
            .collect();
        let text = forms.join(" ");
        let source = revision_with_text(
            &text,
            &format!("heldout/{index}"),
            &format!("source/heldout/{index}"),
            0,
            None,
            Some(text.chars().count() as u32),
            LanguageTextFinality::Final,
        );
        eprintln!("decoding held-out source {index}: {text:?}");
        let before = decoder.model.invocations();
        let start = Instant::now();
        let result = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            let tape = prepare_lexical_tape(&source, &lexical_profile, None)
                .map_err(|error| format!("lexical preparation: {error:?}"))?;
            if tape.tape().tokens().len() != forms.len()
                || tape
                    .tape()
                    .tokens()
                    .iter()
                    .zip(&forms)
                    .any(|(token, form)| token.surface() != *form)
            {
                return Err(
                    "unsupported actual tokenization; no gold segmentation is injected".into(),
                );
            }
            if tape
                .tape()
                .tokens()
                .iter()
                .any(|token| token.candidates().is_empty())
            {
                return Err("unsupported lexical occurrence: no admitted candidates".into());
            }
            let lexical =
                LanguageParserAvailableLexical::new(tape.tape().clone(), forms.len() as u64)
                    .map_err(|error| format!("available lexical admission: {error:?}"))?;
            let initial = decoder.initial(&lexical, &format!("analysis/heldout/{index}"));
            Ok::<_, String>(decoder.decode(&lexical, initial, true))
        }));
        let wall_ns = start.elapsed().as_nanos();
        let calls = decoder.model.invocations() - before;
        total_calls += calls;
        total_ns += wall_ns;
        let (decoded, error) = match result {
            Ok(Ok(decoded)) => (Some(decoded), None),
            Ok(Err(error)) => (None, Some(error)),
            Err(payload) => {
                let error = payload
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| payload.downcast_ref::<&str>().map(|error| (*error).into()))
                    .unwrap_or_else(|| "non-string decoding failure".into());
                (None, Some(format!("decoding panic: {error}")))
            }
        };
        // Evaluation starts here. No gold POS, head or relation was provided to
        // initial(), decode(), lexical preparation or the numerical owner.
        let gold_heads: Vec<u64> = row["heads"]
            .as_array()
            .unwrap()
            .iter()
            .map(|head| head.as_u64().unwrap())
            .collect();
        let gold_relations: Vec<&str> = row["relations"]
            .as_array()
            .unwrap()
            .iter()
            .map(|relation| relation.as_str().unwrap().split(':').next().unwrap())
            .collect();
        let mut predictions = Vec::new();
        let mut row_uas = 0u64;
        let mut row_las = 0u64;
        for token in 0..forms.len() {
            tokens += 1;
            let prediction = decoded.as_ref().and_then(|decoded| {
                let best = decoded.beam.candidate0().parser();
                if !*best.active() || token as u64 >= *best.state().token_count() {
                    return None;
                }
                let state = best.state();
                let head = state.heads()[token];
                if head >= 5 {
                    return None;
                }
                let relation = match token {
                    0 => state.relation0(),
                    1 => state.relation1(),
                    2 => state.relation2(),
                    3 => state.relation3(),
                    _ => return None,
                };
                Some((head, format!("{:?}", relation.base()).to_ascii_lowercase()))
            });
            let gold_vocative = gold_relations[token] == "vocative";
            let exact_head = prediction
                .as_ref()
                .is_some_and(|(head, _)| *head == gold_heads[token]);
            let exact_relation = prediction
                .as_ref()
                .is_some_and(|(_, relation)| relation == gold_relations[token]);
            row_uas += u64::from(exact_head);
            row_las += u64::from(exact_head && exact_relation);
            let predicted_vocative = prediction
                .as_ref()
                .is_some_and(|(_, relation)| relation == "vocative");
            let correct_vocative = gold_vocative && predicted_vocative && exact_head;
            tp += u64::from(correct_vocative);
            fp += u64::from(predicted_vocative && !correct_vocative);
            fn_ += u64::from(gold_vocative && !correct_vocative);
            predictions.push(json!({
                "token":token,"form":forms[token],
                "head":prediction.as_ref().map(|(head,_)|head),
                "relation":prediction.as_ref().map(|(_,relation)|relation),
                "gold_head":gold_heads[token],"gold_relation":gold_relations[token],
                "uas_correct":exact_head,"las_correct":exact_head && exact_relation,
            }));
        }
        uas_correct += row_uas;
        las_correct += row_las;
        rows.push(json!({
            "id":row["id"],"source":text,"text_identity":format!("heldout/{index}"),"analysis_revision":format!("analysis/heldout/{index}"),"source_revision":format!("source/heldout/{index}"),
            "active":decoded.as_ref().is_some_and(|value|*value.beam.candidate0().parser().active()),
            "complete":decoded.as_ref().is_some_and(|value|value.complete),
            "steps":decoded.as_ref().map(|value|value.steps),
            "model_invocations":calls,"invocation_first":(calls>0).then_some(before+1),"invocation_last":(calls>0).then_some(decoder.model.invocations()),"elapsed_ns":wall_ns,
            "decode_elapsed_ns":decoded.as_ref().map(|value|value.elapsed_ns),
            "error":error,"uas_correct":row_uas,"las_correct":row_las,
            "tokens":forms.len(),"predictions":predictions,
        }));
    }
    json!({
        "scope":"blind finite-profile held-out text; all retained rows counted; punctuation included; relation subtypes projected to base",
        "source_reconstruction":"retained forms joined with single spaces; original token typography unavailable in retained annotations",
        "sentences":rows.len(),"tokens":tokens,
        "uas":{"correct":uas_correct,"total":tokens,"accuracy":ratio(uas_correct,tokens)},
        "las":{"correct":las_correct,"total":tokens,"accuracy":ratio(las_correct,tokens)},
        "vocative":{"true_positive":tp,"false_positive":fp,"false_negative":fn_,
            "precision":ratio(tp,tp+fp),"recall":ratio(tp,tp+fn_)},
        "model_invocations":total_calls,"elapsed_ns":total_ns,"rows":rows,
    })
}
