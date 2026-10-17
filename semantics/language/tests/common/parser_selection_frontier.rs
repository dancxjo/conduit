//! Observe actual checked beam consensus, frontier policy and projected facts.
//! Commitment results here are observations; the caller's decoder beam is unchanged.
#![allow(dead_code)]
use super::parser_selection_io::SourceGraph;
use conduit_language::{
    discourse::prepare_vocative_fact, parser_model::english_parser_scorer_descriptor, *,
};
use serde_json::{json, Value};

pub fn observe(
    beam: &LanguageParserScorerBeam,
    lexical: &LanguageParserAvailableLexical,
    agreement: &mut SourceGraph,
    advance: &mut SourceGraph,
    commit: &mut SourceGraph,
    explicit_commit: bool,
) -> Value {
    let best = beam.candidate0().parser();
    let state = best.state();
    let basis = state.basis().clone();
    if basis.text() != lexical.tape().source().material().identity()
        || basis.source_revision() != lexical.tape().source().material().revision()
    {
        return json!({"error":"beam and lexical source basis differ","decoder_beam_mutated":false});
    }
    let admitted = LanguageParserAdmittedBeam::new(
        basis.clone(),
        best.clone(),
        beam.candidate1().parser().clone(),
        beam.candidate2().parser().clone(),
        beam.candidate3().parser().clone(),
    )
    .unwrap();
    let initial_committed = *state.committed();
    let mut frontier =
        LanguageParserRawFrontier::new(basis.clone(), initial_committed, initial_committed)
            .unwrap();
    let mut agreements = Vec::new();
    let mut halted = false;
    for dependent in 0..*lexical.token_count() {
        let consensus: LanguageParserAgreement = agreement.run_native(
            LanguageParserAdmittedAgreementQuery::new(admitted.clone(), dependent).unwrap(),
        );
        let mut advanced = Value::Null;
        if !halted && dependent == *frontier.stable() {
            let result: LanguageParserFrontierResult = advance.run_native(
                LanguageParserAdvanceRequest::new(
                    consensus.clone(),
                    false,
                    frontier.clone(),
                    false,
                )
                .unwrap(),
            );
            advanced =
                json!({"accepted":result.accepted(),"refusal":format!("{:?}",result.refusal())});
            halted = !*result.accepted();
            frontier = result.frontier().clone();
        }
        agreements.push(json!({"dependent":dependent,"agrees":consensus.agrees(),
            "survivors":consensus.survivors(),"reference_head":consensus.reference().head(),
            "reference_relation":format!("{:?}",consensus.reference().relation().base()).to_ascii_lowercase(),
            "advance":advanced}));
    }
    let commit_observation = if explicit_commit {
        let result: LanguageParserBeamCommitResult = commit.run_native(
            LanguageParserBeamCommitRequest::new(
                admitted,
                false,
                frontier.clone(),
                false,
                *frontier.stable(),
            )
            .unwrap(),
        );
        frontier = result.frontier().clone();
        json!({"requested":true,"accepted":result.accepted(),"refusal":format!("{:?}",result.refusal()),
            "candidate_committed":[result.candidate0().state().committed(),result.candidate1().state().committed(),result.candidate2().state().committed(),result.candidate3().state().committed()]})
    } else {
        json!({"requested":false,"accepted":false,"refusal":null})
    };
    let (artifact, _) = english_parser_scorer_descriptor();
    let model: String = artifact
        .content_identity()
        .iter()
        .map(|byte| format!("{byte:02x}"))
        .collect();
    let mut arcs = Vec::new();
    let mut discourse = Vec::new();
    if *best.active() {
        for dependent in 0..*lexical.token_count() {
            let head = state.heads()[dependent as usize];
            if head >= 5 {
                continue;
            }
            let relation = match dependent {
                0 => state.relation0(),
                1 => state.relation1(),
                2 => state.relation2(),
                _ => state.relation3(),
            };
            let projected = (|| {
                let dependent_token = lexical.tape().tokens()[dependent as usize]
                    .identity()
                    .clone();
                let dependent_ref = LanguageAnalysisTokenRef::new(
                    basis.analysis_revision().clone(),
                    dependent_token,
                )?;
                let governor = if head == 4 {
                    LanguageDependencyHead::Root
                } else {
                    LanguageDependencyHead::token(
                        basis.analysis_revision().clone(),
                        lexical.tape().tokens()[head as usize].identity().clone(),
                    )?
                };
                let subtype = if relation.subtype().get().is_empty() {
                    None
                } else {
                    Some(LanguageDependencySubtype::new(
                        relation.subtype().get().clone(),
                    )?)
                };
                LanguageDependencyArc::new(
                    dependent_ref,
                    governor,
                    LanguageDependencyRelation::new(*relation.base(), subtype)?,
                )
            })();
            match projected {
                Ok(arc) => {
                    arcs.push(json!({"dependent":dependent,"head":head,
                        "relation":format!("{:?}",relation.base()).to_ascii_lowercase(),
                        "subtype":relation.subtype().get(),"native_admitted":true}));
                    if matches!(relation.base(), LanguageUniversalDependencyRelation::Vocative) {
                        let provenance = LinguisticDerivationProvenance::model("language/parser-categorical-i16@1".into(),model.clone()).unwrap();
                        let fact = prepare_vocative_fact(format!("parser-vocative/{dependent}"),lexical.tape().source(),basis.analysis_revision(),&arc,provenance,*lexical.token_count());
                        discourse.push(match fact {
                            Ok(fact) => json!({"dependent":dependent,"head":head,"role":format!("{:?}",fact.fact().role()),"refusal":null}),
                            Err(error) => json!({"dependent":dependent,"head":head,"role":null,"refusal":format!("{error:?}")}),
                        });
                    }
                }
                Err(error) => arcs.push(json!({"dependent":dependent,"head":head,"native_admitted":false,"refusal":format!("{error:?}")})),
            }
        }
    }
    json!({"source_revision":basis.source_revision().get(),"analysis_revision":basis.analysis_revision().get(),
        "model_content_identity":model,"stable":frontier.stable(),"committed":frontier.committed(),
        "decoder_beam_committed":initial_committed,"decoder_beam_mutated":false,
        "commit_observation":commit_observation,"agreements":agreements,"actual_best_arcs":arcs,
        "discourse":discourse,"discourse_abstained":discourse.is_empty()})
}
