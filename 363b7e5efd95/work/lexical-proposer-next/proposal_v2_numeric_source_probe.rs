#[path="../../semantics/language/tests/common/parser_model_resource.rs"] mod model_resource;
#[path="../../semantics/language/tests/common/parser_fixture.rs"] mod fixture;
#[path="../../semantics/language/tests/common/window8_proposal_candidate.rs"] mod candidate;
use conduit_plot::*;
fn main(){
 let candidate=candidate::prepare(); let document=candidate::numeric_source(&candidate);
 std::fs::write("work/lexical-proposer-next/proposal-v2-numeric-source.conduit",&document).unwrap();
 let mut startup=StartupCatalog::new();let mut catalogs=ProfileCatalog::new();candidate.scorer.install(&mut startup,&mut catalogs,true).unwrap();
 startup.insert_structured_type("LanguageParserWindow8ProposerV2CategoricalIndices",candidate.scorer.indices_type().clone()).unwrap();startup.insert_structured_type("LanguageParserWindow8ProposerV2CategoricalScores",candidate.scorer.scores_type().clone()).unwrap();
 let checked=check_syntax_document(&parse_syntax_document(&document),&startup).unwrap();let expanded=expand_canonical_plot_for_authoring(&checked,"window8-proposal-v2-learned-model",&catalogs).unwrap();
 println!("PASS exact independently pinned411model Source numeric projection+model expansion {}gears/{}cords; no target execution/corpus claim",expanded.expanded.gears.len(),expanded.expanded.cords.len());
}
